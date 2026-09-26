//! Local process presence, never prompt contents or transcripts.
use serde::Serialize;
use std::path::Path;
#[derive(Debug, Default, Clone, Serialize)]
pub struct AgentPresence { pub codex: bool, pub claude: bool }
impl AgentPresence { pub fn active(&self) -> bool { self.codex || self.claude } }

pub fn identify(executable: &str, script: Option<&str>) -> AgentPresence {
    let name = Path::new(executable).file_name().and_then(|s|s.to_str()).unwrap_or(executable).to_ascii_lowercase();
    let native_claude = executable.contains("/.local/share/claude/versions/") && name.chars().all(|c|c.is_ascii_digit() || c == '.');
    let script = script.unwrap_or("").replace('\\', "/");
    let node = matches!(name.as_str(), "node" | "nodejs" | "bun");
    AgentPresence {
        codex: matches!(name.as_str(), "codex" | "codex-desktop") || (node && script.ends_with("/@openai/codex/bin/codex.js")),
        claude: native_claude || matches!(name.as_str(), "claude" | "claude-desktop") || (node && script.ends_with("/@anthropic-ai/claude-code/cli.js")),
    }
}

pub fn detect() -> AgentPresence {
    let mut found = AgentPresence::default();
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let uid = std::fs::metadata("/proc/self").map(|m| m.uid()).unwrap_or(u32::MAX);
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                if !entry.file_name().to_string_lossy().chars().all(|c| c.is_ascii_digit()) { continue; }
                let path = entry.path();
                if path.metadata().map(|m| m.uid() != uid).unwrap_or(true) { continue; }
                let exe = std::fs::read_link(path.join("exe")).unwrap_or_default();
                let name = exe.file_name().and_then(|s| s.to_str()).unwrap_or("");
                // Only inspect a known runtime's script argument, never arbitrary arguments.
                let script = if matches!(name, "node" | "nodejs" | "bun") {
                    std::fs::read(path.join("cmdline")).ok().and_then(|b| b.split(|v| *v == 0).nth(1).map(|s| String::from_utf8_lossy(s).into_owned()))
                } else { None };
                let seen = identify(&exe.to_string_lossy(), script.as_deref());
                found.codex |= seen.codex; found.claude |= seen.claude;
            }
        }
    }
    found
}

/// Seconds excluded from the timer. Large scheduling gaps are treated as sleep.
pub fn excluded_elapsed(elapsed: f64, active: bool, snoozed: bool) -> f64 {
    if !active || snoozed || elapsed > 5.0 { elapsed.max(0.0) } else { 0.0 }
}

/// The one definition of where our state lives: `~/.local/share/rfp`.
/// Migrates a pre-rebrand `reps-for-claude/` directory on first use. The rename
/// is atomic and carries history, mode and the nested `vision-env/` across
/// together; doing it here rather than at startup means no caller can race it.
pub fn app_home() -> std::path::PathBuf {
    if let Some(home) = std::env::var_os("REPS_APP_HOME") { return home.into(); }
    home_in(&Path::new(&std::env::var_os("HOME").unwrap_or_default()).join(".local/share"))
}

fn home_in(share: &Path) -> std::path::PathBuf {
    let (dir, legacy) = (share.join("rfp"), share.join("reps-for-claude"));
    if !dir.exists() && legacy.is_dir() { let _ = std::fs::rename(&legacy, &dir); }
    dir
}

#[cfg(test)] mod tests {
    use super::*;

    #[test] fn migrates_pre_rebrand_home_once() {
        let tmp = tempfile::tempdir().unwrap(); let share = tmp.path();
        let legacy = share.join("reps-for-claude");
        std::fs::create_dir_all(legacy.join("vision-env")).unwrap();
        std::fs::write(legacy.join("reps.sqlite"), b"history").unwrap();

        let home = home_in(share);
        assert_eq!(home, share.join("rfp"));
        assert_eq!(std::fs::read(home.join("reps.sqlite")).unwrap(), b"history");
        assert!(home.join("vision-env").is_dir(), "nested env must come across");
        assert!(!legacy.exists(), "legacy directory must not linger");

        // Idempotent, and a later legacy directory never overwrites live history.
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("reps.sqlite"), b"stale").unwrap();
        assert_eq!(home_in(share), share.join("rfp"));
        assert_eq!(std::fs::read(home.join("reps.sqlite")).unwrap(), b"history");
    }

    #[test] fn exact_names_only() {
        assert!(identify("codex", None).codex);
        assert!(identify("claude", None).claude);
        assert!(identify("/home/me/.local/share/claude/versions/2.1.276", None).claude);
        assert!(identify("node", Some("/usr/lib/node_modules/@anthropic-ai/claude-code/cli.js")).claude);
        assert!(!identify("bash", Some("claude")).active());
        assert!(!identify("my-codex-backup", None).active());
        assert!(!identify("node", Some("/tmp/codex.js")).active());
    }
    #[test] fn excludes_pause_snooze_and_sleep() {
        assert_eq!(excluded_elapsed(1., true, false), 0.);
        assert_eq!(excluded_elapsed(1., false, false), 1.);
        assert_eq!(excluded_elapsed(1., true, true), 1.);
        assert_eq!(excluded_elapsed(3600., true, false), 3600.);
    }
}
