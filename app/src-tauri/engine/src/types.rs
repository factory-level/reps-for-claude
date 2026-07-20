//! Shared plain-data types. Snapshot is the single UI contract (camelCase).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExerciseKind {
    Rep,
    Continuous,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExerciseDef {
    pub name: String,
    pub kind: ExerciseKind,
    pub default_reps: u32,
    pub default_weight: f64,
    pub target_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prescription {
    pub exercise: String,
    pub kind: ExerciseKind,
    pub target_reps: u32,
    pub target_seconds: f64,
    pub default_weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Phase {
    Coding,
    ExerciseRequired,
    WorkoutActive,
    WeightConfirmation,
    Unlocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub value: f64,
    pub unit: String,
    pub satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRecord {
    pub date: String,
    pub exercise: String,
    pub kind: ExerciseKind,
    pub reps: u32,
    pub seconds: f64,
    pub weight: f64,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub phase: Phase,
    pub remaining_seconds: f64,
    pub prescription: Option<Prescription>,
    pub progress: Option<Progress>,
    pub capacity_used: u32,
    pub capacity_limit: u32,
    pub rotation: Vec<String>,
    pub pointer: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_serializes_screaming_snake() {
        let s = serde_json::to_string(&Phase::ExerciseRequired).unwrap();
        assert_eq!(s, "\"EXERCISE_REQUIRED\"");
    }

    #[test]
    fn snapshot_serializes_camel_case() {
        let snap = Snapshot {
            phase: Phase::Coding,
            remaining_seconds: 12.0,
            prescription: None,
            progress: None,
            capacity_used: 0,
            capacity_limit: 20,
            rotation: vec!["bench".into()],
            pointer: 0,
        };
        let v: serde_json::Value = serde_json::to_value(&snap).unwrap();
        assert!(v.get("remainingSeconds").is_some());
        assert!(v.get("capacityLimit").is_some());
    }
}
