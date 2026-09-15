//! Desktop publications awaiting hub acknowledgement. Persist before returning
//! to the caller; remove only after acknowledgement. Retries preserve wire IDs.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Publication {
    Event(serde_json::Value),
    Command(serde_json::Value),
    ActionResult(serde_json::Value),
}

#[derive(Debug)]
pub struct Pending {
    pub sequence: i64,
    pub publication: Publication,
    pub attempts: u32,
}

pub struct Outbox {
    db: Connection,
    capacity: usize,
}

impl Outbox {
    pub fn open(path: &Path) -> Result<Self, String> {
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        db.busy_timeout(std::time::Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS publications (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                body TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                retry_at INTEGER NOT NULL DEFAULT 0,
                last_error TEXT
            );",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            db,
            capacity: 10_000,
        })
    }

    pub fn enqueue(&mut self, publication: &Publication) -> Result<(), String> {
        let body = serde_json::to_string(publication).map_err(|e| e.to_string())?;
        let payload = match publication {
            Publication::Event(p) | Publication::Command(p) | Publication::ActionResult(p) => p,
        };
        if payload
            .get("id")
            .and_then(|v| v.as_str())
            .is_none_or(str::is_empty)
        {
            return Err("publication requires a stable id".into());
        }
        if body.len() > 256 * 1024 {
            return Err("publication exceeds 256 KiB".into());
        }
        let tx = self
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let count: usize = tx
            .query_row("SELECT COUNT(*) FROM publications", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if count >= self.capacity {
            return Err("publication outbox is full; restore hub connectivity".into());
        }
        tx.execute("INSERT INTO publications(body) VALUES (?)", [body])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn next(&self, now_ms: i64) -> Result<Option<Pending>, String> {
        // Keep FIFO order even when the head is backing off: a result must not
        // overtake its command, or a workout completion its prescription.
        let row = self.db.query_row("SELECT sequence, body, attempts, retry_at FROM publications ORDER BY sequence LIMIT 1", [],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, u32>(2)?, r.get::<_, i64>(3)?)))
            .optional().map_err(|e| e.to_string())?;
        match row {
            Some((sequence, body, attempts, retry_at)) if retry_at <= now_ms => Ok(Some(Pending {
                sequence,
                attempts,
                publication: serde_json::from_str(&body).map_err(|e| e.to_string())?,
            })),
            _ => Ok(None),
        }
    }

    pub fn acknowledge(&self, sequence: i64) -> Result<(), String> {
        self.db
            .execute("DELETE FROM publications WHERE sequence = ?", [sequence])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn retry(&self, pending: &Pending, now_ms: i64, error: &str) -> Result<(), String> {
        let delay_ms = (1000_i64 * (1_i64 << pending.attempts.min(5))).min(30_000);
        let message: String = error.chars().take(1024).collect();
        self.db.execute("UPDATE publications SET attempts = attempts + 1, retry_at = ?, last_error = ? WHERE sequence = ?",
            params![now_ms.saturating_add(delay_ms), message, pending.sequence]).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn restart_preserves_all_kinds_order_and_retry_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("outbox.sqlite");
        let messages = [
            Publication::Event(json!({"id":"e1", "sessionId":"s1"})),
            Publication::Command(json!({"id":"c1"})),
            Publication::ActionResult(json!({"id":"r1", "commandId":"c1"})),
        ];
        {
            let mut outbox = Outbox::open(&path).unwrap();
            for message in &messages {
                outbox.enqueue(message).unwrap();
            }
            let head = outbox.next(0).unwrap().unwrap();
            outbox.retry(&head, 100, "hub down").unwrap();
        }
        let outbox = Outbox::open(&path).unwrap();
        assert!(outbox.next(1099).unwrap().is_none());
        for (index, message) in messages.iter().enumerate() {
            let head = outbox.next(1100).unwrap().unwrap();
            assert_eq!(&head.publication, message);
            assert_eq!(head.attempts, if index == 0 { 1 } else { 0 });
            outbox.acknowledge(head.sequence).unwrap();
        }
        assert!(outbox.next(i64::MAX).unwrap().is_none());
    }

    #[test]
    fn sqlite_full_preserves_committed_head_and_recovers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("outbox.sqlite");
        let mut outbox = Outbox::open(&path).unwrap();
        let first = Publication::Event(json!({"id":"committed"}));
        outbox.enqueue(&first).unwrap();
        // SQLite's own page limit produces SQLITE_FULL without filling the host disk.
        let pages: i64 = outbox.db.query_row("PRAGMA page_count", [], |r| r.get(0)).unwrap();
        outbox.db.execute_batch(&format!("PRAGMA max_page_count={pages}")).unwrap();
        let error = outbox.enqueue(&Publication::Event(json!({"id":"too-large", "data":"x".repeat(128 * 1024)}))).unwrap_err();
        assert!(error.contains("full"), "{error}");
        assert_eq!(outbox.next(0).unwrap().unwrap().publication, first);
        outbox.db.execute_batch("PRAGMA max_page_count=1073741823").unwrap();
        drop(outbox);
        let mut recovered = Outbox::open(&path).unwrap();
        let head = recovered.next(0).unwrap().unwrap();
        assert_eq!(head.publication, first);
        recovered.acknowledge(head.sequence).unwrap();
        recovered.enqueue(&Publication::Event(json!({"id":"after-recovery"}))).unwrap();
        assert_eq!(recovered.next(0).unwrap().unwrap().publication, Publication::Event(json!({"id":"after-recovery"})));
    }

    #[test]
    fn lost_ack_is_replayed_instead_of_discarded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("outbox.sqlite");
        let message = Publication::Event(json!({"id":"stable"}));
        {
            let mut outbox = Outbox::open(&path).unwrap();
            outbox.enqueue(&message).unwrap();
            assert_eq!(outbox.next(0).unwrap().unwrap().publication, message);
            // Hub accepted; desktop crashed before local acknowledgement.
        }
        let outbox = Outbox::open(&path).unwrap();
        let head = outbox.next(0).unwrap().unwrap();
        assert_eq!(head.publication, message);
        outbox.acknowledge(head.sequence).unwrap();
        drop(outbox);
        assert!(Outbox::open(&path).unwrap().next(0).unwrap().is_none());
    }

    #[test]
    fn capacity_and_invalid_identity_fail_without_losing_pending_records() {
        let dir = tempfile::tempdir().unwrap();
        let mut outbox = Outbox::open(&dir.path().join("outbox.sqlite")).unwrap();
        outbox.capacity = 1;
        assert!(outbox.enqueue(&Publication::Event(json!({}))).is_err());
        let first = Publication::Event(json!({"id":"first"}));
        outbox.enqueue(&first).unwrap();
        assert!(outbox
            .enqueue(&Publication::Event(json!({"id":"second"})))
            .is_err());
        assert_eq!(outbox.next(0).unwrap().unwrap().publication, first);
    }

    #[test]
    fn committed_publication_survives_process_exit_without_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("outbox.sqlite");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "outbox::tests::crash_writer"])
            .env("REPS_TEST_OUTBOX_CRASH_PATH", &path)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(17));
        let outbox = Outbox::open(&path).unwrap();
        assert_eq!(
            outbox.next(0).unwrap().unwrap().publication,
            Publication::Event(json!({"id":"before-crash", "sessionId":"s1"}))
        );
    }

    #[test]
    fn crash_writer() {
        let Some(path) = std::env::var_os("REPS_TEST_OUTBOX_CRASH_PATH") else {
            return;
        };
        let mut outbox = Outbox::open(Path::new(&path)).unwrap();
        outbox
            .enqueue(&Publication::Event(
                json!({"id":"before-crash", "sessionId":"s1"}),
            ))
            .unwrap();
        // Bypass all destructors: recovery must use the committed SQLite WAL.
        std::process::exit(17);
    }
}
