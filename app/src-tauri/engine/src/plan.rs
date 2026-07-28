//! Daily routine plan: the app works off a routine.json of LIFTS (sets×reps),
//! CONDITIONING (jump-rope rounds×seconds), and STRETCHES (timed holds). Each
//! lock prescribes ONE incomplete set/round/hold; the day clears when every
//! item is done. This replaces the old infinite rotation with a bounded,
//! completable daily program.

use serde::Deserialize;

use crate::types::{DayItem, DayPlan, ExerciseKind, Prescription};

// ---- routine.json shape ----
#[derive(Debug, Deserialize)]
struct RoutineFile {
    #[serde(default)]
    lifts: Vec<LiftEntry>,
    #[serde(default)]
    conditioning: Vec<CondEntry>,
    #[serde(default)]
    stretches: Vec<StretchEntry>,
}

#[derive(Debug, Deserialize)]
struct LiftEntry {
    exercise: String,
    sets: u32,
    reps: u32,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    weight: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct CondEntry {
    exercise: String,
    #[serde(default = "one")]
    rounds: u32,
    seconds: f64,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StretchEntry {
    #[serde(default)]
    name: Option<String>,
    seconds: f64,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    per_side: bool,
}

fn one() -> u32 {
    1
}

/// One line of the daily plan with how many of its sets/rounds/holds remain.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanItem {
    pub exercise: String, // detection name: a lift, "jumprope", or "stretch"
    pub label: String,
    pub kind: ExerciseKind, // Rep for lifts, Continuous for jumprope/stretch
    pub ui_kind: String,    // "lift" | "jumprope" | "stretch" for the UI icon
    pub target_reps: u32,
    pub target_seconds: f64,
    pub weight: f64,
    pub total: u32,
    pub done: u32,
}

#[derive(Debug, Clone)]
pub struct DailyPlan {
    items: Vec<PlanItem>,
    current: Option<usize>,
    date: String,
}

impl DailyPlan {
    pub fn from_routine_json(json: &str, today: &str) -> Result<Self, String> {
        let r: RoutineFile = serde_json::from_str(json).map_err(|e| format!("routine.json: {e}"))?;
        let mut items = Vec::new();
        for l in r.lifts {
            items.push(PlanItem {
                label: l.label.clone().unwrap_or_else(|| l.exercise.clone()),
                exercise: l.exercise,
                kind: ExerciseKind::Rep,
                ui_kind: "lift".into(),
                target_reps: l.reps,
                target_seconds: 0.0,
                weight: l.weight.unwrap_or(0.0),
                total: l.sets.max(1),
                done: 0,
            });
        }
        for c in r.conditioning {
            items.push(PlanItem {
                label: c.label.clone().unwrap_or_else(|| c.exercise.clone()),
                ui_kind: if c.exercise == "jumprope" { "jumprope".into() } else { "lift".into() },
                exercise: c.exercise,
                kind: ExerciseKind::Continuous,
                target_reps: 0,
                target_seconds: c.seconds,
                weight: 0.0,
                total: c.rounds.max(1),
                done: 0,
            });
        }
        for s in r.stretches {
            // Every stretch is detected by the generic "stretch" timed hold; the
            // routine name is just the label. perSide → two holds.
            items.push(PlanItem {
                exercise: "stretch".into(),
                label: s.label.or(s.name).unwrap_or_else(|| "Stretch".into()),
                kind: ExerciseKind::Continuous,
                ui_kind: "stretch".into(),
                target_reps: 0,
                target_seconds: s.seconds,
                weight: 0.0,
                total: if s.per_side { 2 } else { 1 },
                done: 0,
            });
        }
        Ok(Self { items, current: None, date: today.to_string() })
    }

    /// Reset all completion at the start of a new day.
    pub fn roll_date(&mut self, today: &str) {
        if self.date != today {
            for it in &mut self.items {
                it.done = 0;
            }
            self.current = None;
            self.date = today.to_string();
        }
    }

    /// Prescribe the next incomplete set/round/hold (one per lock). Returns None
    /// when the whole day is done.
    pub fn prescribe(&mut self) -> Option<Prescription> {
        let idx = self.items.iter().position(|it| it.done < it.total)?;
        self.current = Some(idx);
        let it = &self.items[idx];
        Some(Prescription {
            exercise: it.exercise.clone(),
            kind: it.kind,
            target_reps: it.target_reps,
            target_seconds: it.target_seconds,
            default_weight: it.weight,
        })
    }

    /// Mark the currently-prescribed set complete.
    pub fn complete(&mut self) {
        if let Some(i) = self.current.take() {
            if let Some(it) = self.items.get_mut(i) {
                if it.done < it.total {
                    it.done += 1;
                }
            }
        }
    }

    pub fn is_complete(&self) -> bool {
        self.items.iter().all(|it| it.done >= it.total)
    }

    pub fn date(&self) -> &str {
        &self.date
    }

    /// Restore per-item done counts (from persistence), matched by label+exercise.
    pub fn restore(&mut self, date: &str, done: &[(String, u32)]) {
        self.date = date.to_string();
        for (key, d) in done {
            if let Some(it) = self.items.iter_mut().find(|it| plan_key(it) == *key) {
                it.done = (*d).min(it.total);
            }
        }
    }

    pub fn done_state(&self) -> Vec<(String, u32)> {
        self.items.iter().map(|it| (plan_key(it), it.done)).collect()
    }

    /// The UI-facing plan (camelCase Snapshot contract).
    pub fn to_day_plan(&self) -> DayPlan {
        let items: Vec<DayItem> = self
            .items
            .iter()
            .map(|it| DayItem {
                name: it.exercise.clone(),
                label: it.label.clone(),
                kind: it.ui_kind.clone(),
                unit: if it.kind == ExerciseKind::Rep { "reps".into() } else { "seconds".into() },
                target: if it.kind == ExerciseKind::Rep { it.target_reps as f64 } else { it.target_seconds },
                done: it.done,
                total: it.total,
            })
            .collect();
        let sets_done = self.items.iter().map(|it| it.done).sum();
        let sets_total = self.items.iter().map(|it| it.total).sum();
        DayPlan { items, sets_done, sets_total, complete: self.is_complete() }
    }
}

/// Stable key per plan item for persistence (label distinguishes same-exercise
/// stretches).
fn plan_key(it: &PlanItem) -> String {
    format!("{}|{}", it.exercise, it.label)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUTINE: &str = r#"{
      "lifts": [{"exercise":"squat","sets":2,"reps":5,"label":"Back squat"}],
      "conditioning": [{"exercise":"jumprope","rounds":1,"seconds":60,"label":"Jump rope"}],
      "stretches": [{"name":"hamstring","label":"Hamstring","seconds":30,"perSide":true}]
    }"#;

    #[test]
    fn expands_routine_into_sets() {
        let plan = DailyPlan::from_routine_json(ROUTINE, "2026-07-21").unwrap();
        let dp = plan.to_day_plan();
        assert_eq!(dp.items.len(), 3);
        // 2 squat sets + 1 jumprope round + 2 hamstring holds (perSide)
        assert_eq!(dp.sets_total, 5);
        assert_eq!(dp.sets_done, 0);
        assert!(!dp.complete);
    }

    #[test]
    fn prescribes_one_set_per_lock_until_the_day_is_complete() {
        let mut plan = DailyPlan::from_routine_json(ROUTINE, "d").unwrap();
        let mut kinds = Vec::new();
        while let Some(rx) = plan.prescribe() {
            kinds.push(rx.exercise.clone());
            assert!(!plan.is_complete()); // not done until this set is completed
            plan.complete();
        }
        // squat, squat, jumprope, stretch, stretch — in routine order
        assert_eq!(kinds, vec!["squat", "squat", "jumprope", "stretch", "stretch"]);
        assert!(plan.is_complete());
        assert_eq!(plan.to_day_plan().sets_done, 5);
        assert!(plan.prescribe().is_none());
    }

    #[test]
    fn lift_carries_reps_continuous_carries_seconds() {
        let mut plan = DailyPlan::from_routine_json(ROUTINE, "d").unwrap();
        let squat = plan.prescribe().unwrap();
        assert_eq!(squat.kind, ExerciseKind::Rep);
        assert_eq!(squat.target_reps, 5);
        plan.complete();
        plan.complete_noop_guard();
        // advance past 2nd squat + jumprope to a stretch
        plan.prescribe();
        plan.complete(); // squat 2
        let jr = plan.prescribe().unwrap();
        assert_eq!(jr.exercise, "jumprope");
        assert_eq!(jr.kind, ExerciseKind::Continuous);
        assert_eq!(jr.target_seconds, 60.0);
        plan.complete();
        let st = plan.prescribe().unwrap();
        assert_eq!(st.exercise, "stretch");
        assert_eq!(st.kind, ExerciseKind::Continuous);
        assert_eq!(st.target_seconds, 30.0);
    }

    #[test]
    fn roll_date_resets_completion() {
        let mut plan = DailyPlan::from_routine_json(ROUTINE, "day1").unwrap();
        plan.prescribe();
        plan.complete();
        assert_eq!(plan.to_day_plan().sets_done, 1);
        plan.roll_date("day2");
        assert_eq!(plan.to_day_plan().sets_done, 0);
        assert_eq!(plan.date(), "day2");
    }

    #[test]
    fn restore_reapplies_done_counts() {
        let mut plan = DailyPlan::from_routine_json(ROUTINE, "d").unwrap();
        let state = {
            let mut p = DailyPlan::from_routine_json(ROUTINE, "d").unwrap();
            p.prescribe();
            p.complete(); // squat 1 done
            p.done_state()
        };
        plan.restore("d", &state);
        assert_eq!(plan.to_day_plan().sets_done, 1);
    }
}

// test-only helper to keep the borrow checker happy in a chained test
impl DailyPlan {
    #[cfg(test)]
    fn complete_noop_guard(&mut self) {}
}
