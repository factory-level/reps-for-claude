//! SQLite persistence. Only this module touches the database.

use std::path::Path;

use rusqlite::{params, Connection};

use crate::types::{ExerciseDef, ExerciseKind, SetRecord};

pub struct Store {
    conn: Connection,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS rotation (
    position INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    default_reps INTEGER NOT NULL,
    default_weight REAL NOT NULL,
    target_seconds REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS pointer_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    pointer INTEGER NOT NULL,
    capacity_used INTEGER NOT NULL,
    capacity_date TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS exercise_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL,
    exercise TEXT NOT NULL,
    kind TEXT NOT NULL,
    reps INTEGER NOT NULL,
    seconds REAL NOT NULL,
    weight REAL NOT NULL,
    verified INTEGER NOT NULL
);
";

fn kind_to_str(k: ExerciseKind) -> &'static str {
    match k {
        ExerciseKind::Rep => "rep",
        ExerciseKind::Continuous => "continuous",
    }
}

fn kind_from_str(s: &str) -> ExerciseKind {
    if s == "continuous" { ExerciseKind::Continuous } else { ExerciseKind::Rep }
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        let version: Option<i64> = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .ok();
        if version.is_none() {
            conn.execute("INSERT INTO schema_version (version) VALUES (1)", [])?;
        }
        let store = Self { conn };
        if store.load_rotation()?.is_empty() {
            store.save_rotation(&default_rotation())?;
        }
        Ok(store)
    }

    pub fn load_rotation(&self) -> rusqlite::Result<Vec<ExerciseDef>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, kind, default_reps, default_weight, target_seconds
             FROM rotation ORDER BY position",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ExerciseDef {
                name: r.get(0)?,
                kind: kind_from_str(&r.get::<_, String>(1)?),
                default_reps: r.get(2)?,
                default_weight: r.get(3)?,
                target_seconds: r.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn save_rotation(&self, defs: &[ExerciseDef]) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM rotation", [])?;
        for (i, d) in defs.iter().enumerate() {
            self.conn.execute(
                "INSERT INTO rotation (position, name, kind, default_reps, default_weight, target_seconds)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![i as i64, d.name, kind_to_str(d.kind), d.default_reps, d.default_weight, d.target_seconds],
            )?;
        }
        Ok(())
    }

    pub fn load_pointer_state(&self) -> rusqlite::Result<(usize, u32, String)> {
        let row = self
            .conn
            .query_row(
                "SELECT pointer, capacity_used, capacity_date FROM pointer_state WHERE id = 1",
                [],
                |r| Ok((r.get::<_, i64>(0)? as usize, r.get(1)?, r.get(2)?)),
            );
        match row {
            Ok(v) => Ok(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok((0, 0, String::new())),
            Err(e) => Err(e),
        }
    }

    pub fn save_pointer_state(
        &self,
        pointer: usize,
        capacity_used: u32,
        capacity_date: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO pointer_state (id, pointer, capacity_used, capacity_date)
             VALUES (1, ?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET
               pointer = ?1, capacity_used = ?2, capacity_date = ?3",
            params![pointer as i64, capacity_used, capacity_date],
        )?;
        Ok(())
    }

    pub fn record_set(&self, rec: &SetRecord) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO exercise_history (date, exercise, kind, reps, seconds, weight, verified)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                rec.date,
                rec.exercise,
                kind_to_str(rec.kind),
                rec.reps,
                rec.seconds,
                rec.weight,
                rec.verified as i64
            ],
        )?;
        Ok(())
    }

    pub fn history(&self, limit: u32) -> rusqlite::Result<Vec<SetRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT date, exercise, kind, reps, seconds, weight, verified
             FROM exercise_history ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |r| {
            Ok(SetRecord {
                date: r.get(0)?,
                exercise: r.get(1)?,
                kind: kind_from_str(&r.get::<_, String>(2)?),
                reps: r.get(3)?,
                seconds: r.get(4)?,
                weight: r.get(5)?,
                verified: r.get::<_, i64>(6)? != 0,
            })
        })?;
        rows.collect()
    }

    pub fn setting(&self, key: &str, default: &str) -> String {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .unwrap_or_else(|_| default.to_string())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }
}

pub fn default_rotation() -> Vec<ExerciseDef> {
    let lift = |name: &str, weight: f64| ExerciseDef {
        name: name.into(),
        kind: ExerciseKind::Rep,
        default_reps: 10,
        default_weight: weight,
        target_seconds: 0.0,
    };
    vec![
        lift("bench", 95.0),
        lift("row", 65.0),
        lift("squat", 115.0),
        lift("overhead", 55.0),
        lift("curl", 25.0),
    ]
}

pub fn default_continuous_pool() -> Vec<ExerciseDef> {
    vec![
        ExerciseDef {
            name: "jumprope".into(),
            kind: ExerciseKind::Continuous,
            default_reps: 0,
            default_weight: 0.0,
            target_seconds: 60.0,
        },
        ExerciseDef {
            name: "stretch".into(),
            kind: ExerciseKind::Continuous,
            default_reps: 0,
            default_weight: 0.0,
            target_seconds: 30.0,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ExerciseKind, SetRecord};

    fn open_tmp() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("reps.sqlite")).unwrap();
        (dir, store)
    }

    #[test]
    fn seeds_default_rotation_once() {
        let (dir, store) = open_tmp();
        let rot = store.load_rotation().unwrap();
        assert!(!rot.is_empty());
        // reopen: still there, not duplicated
        drop(store);
        let store = Store::open(&dir.path().join("reps.sqlite")).unwrap();
        assert_eq!(store.load_rotation().unwrap().len(), rot.len());
    }

    #[test]
    fn pointer_state_roundtrips() {
        let (_dir, store) = open_tmp();
        assert_eq!(store.load_pointer_state().unwrap(), (0, 0, String::new()));
        store.save_pointer_state(3, 5, "2026-07-19").unwrap();
        assert_eq!(
            store.load_pointer_state().unwrap(),
            (3, 5, "2026-07-19".to_string())
        );
    }

    #[test]
    fn history_newest_first() {
        let (_dir, store) = open_tmp();
        for (i, name) in ["bench", "row"].iter().enumerate() {
            store
                .record_set(&SetRecord {
                    date: format!("2026-07-1{i}"),
                    exercise: name.to_string(),
                    kind: ExerciseKind::Rep,
                    reps: 10,
                    seconds: 0.0,
                    weight: 100.0,
                    verified: true,
                })
                .unwrap();
        }
        let h = store.history(10).unwrap();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].exercise, "row");
    }

    #[test]
    fn settings_roundtrip_with_default() {
        let (_dir, store) = open_tmp();
        assert_eq!(store.setting("work_minutes", "6"), "6");
        store.set_setting("work_minutes", "25").unwrap();
        assert_eq!(store.setting("work_minutes", "6"), "25");
    }

    // Source of truth: vision/src/reps_vision/exercises.py SPECS (rep
    // exercises) and the jumprope/stretch activities under
    // vision/src/reps_vision/activities/. Seeded rotation/pool names must
    // stay in sync with what the vision sidecar actually knows how to
    // detect, or the app will prescribe an exercise the detector rejects.
    const KNOWN_VISION_REP_EXERCISES: &[&str] =
        &["bench", "curl", "overhead", "pullup", "pushup", "row", "squat"];
    const KNOWN_VISION_CONTINUOUS_EXERCISES: &[&str] = &["jumprope", "stretch"];

    #[test]
    fn default_rotation_names_match_known_vision_exercises() {
        for def in default_rotation() {
            assert!(
                KNOWN_VISION_REP_EXERCISES.contains(&def.name.as_str()),
                "default_rotation() seeds {:?}, which is not in the known-vision set {:?}",
                def.name,
                KNOWN_VISION_REP_EXERCISES
            );
        }
    }

    #[test]
    fn default_continuous_pool_names_match_known_vision_exercises() {
        for def in default_continuous_pool() {
            assert!(
                KNOWN_VISION_CONTINUOUS_EXERCISES.contains(&def.name.as_str()),
                "default_continuous_pool() seeds {:?}, which is not in the known-vision set {:?}",
                def.name,
                KNOWN_VISION_CONTINUOUS_EXERCISES
            );
        }
    }
}
