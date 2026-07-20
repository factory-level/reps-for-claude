//! Rotation + daily-capacity workout programming (V1 spec §6–7).
//! The pointer never resets; capacity resets on a new date.

use crate::types::{ExerciseDef, ExerciseKind, Prescription};

pub struct WorkoutEngine {
    rotation: Vec<ExerciseDef>,
    continuous_pool: Vec<ExerciseDef>,
    capacity_limit: u32,
    pointer: usize,
    continuous_pointer: usize,
    capacity_used: u32,
    capacity_date: String,
    last_kind: Option<ExerciseKind>,
}

impl WorkoutEngine {
    pub fn new(
        rotation: Vec<ExerciseDef>,
        continuous_pool: Vec<ExerciseDef>,
        capacity_limit: u32,
    ) -> Self {
        Self {
            rotation,
            continuous_pool,
            capacity_limit,
            pointer: 0,
            continuous_pointer: 0,
            capacity_used: 0,
            capacity_date: String::new(),
            last_kind: None,
        }
    }

    pub fn restore(&mut self, pointer: usize, capacity_used: u32, capacity_date: &str) {
        self.pointer = if self.rotation.is_empty() { 0 } else { pointer % self.rotation.len() };
        self.capacity_used = capacity_used;
        self.capacity_date = capacity_date.to_string();
    }

    fn roll_date(&mut self, today: &str) {
        if self.capacity_date != today {
            self.capacity_date = today.to_string();
            self.capacity_used = 0;
        }
    }

    pub fn prescribe(&mut self, today: &str) -> Option<Prescription> {
        self.roll_date(today);
        let lifting_open = self.capacity_used < self.capacity_limit;
        let def = if lifting_open && !self.rotation.is_empty() {
            &self.rotation[self.pointer]
        } else if !self.continuous_pool.is_empty() {
            &self.continuous_pool[self.continuous_pointer % self.continuous_pool.len()]
        } else if !self.rotation.is_empty() {
            &self.rotation[self.pointer]
        } else {
            return None;
        };
        self.last_kind = Some(def.kind);
        Some(Prescription {
            exercise: def.name.clone(),
            kind: def.kind,
            target_reps: def.default_reps,
            target_seconds: def.target_seconds,
            default_weight: def.default_weight,
        })
    }

    pub fn complete(&mut self, today: &str) {
        self.roll_date(today);
        match self.last_kind {
            Some(ExerciseKind::Rep) => {
                self.capacity_used += 1;
                if !self.rotation.is_empty() {
                    self.pointer = (self.pointer + 1) % self.rotation.len();
                }
            }
            Some(ExerciseKind::Continuous) => {
                if !self.continuous_pool.is_empty() {
                    self.continuous_pointer =
                        (self.continuous_pointer + 1) % self.continuous_pool.len();
                }
            }
            None => {}
        }
        self.last_kind = None;
    }

    pub fn pointer(&self) -> usize {
        self.pointer
    }

    pub fn capacity_used(&self) -> u32 {
        self.capacity_used
    }

    pub fn capacity_limit(&self) -> u32 {
        self.capacity_limit
    }

    pub fn capacity_date(&self) -> &str {
        &self.capacity_date
    }

    pub fn rotation_names(&self) -> Vec<String> {
        self.rotation.iter().map(|d| d.name.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ExerciseDef, ExerciseKind};

    fn lift(name: &str) -> ExerciseDef {
        ExerciseDef {
            name: name.into(),
            kind: ExerciseKind::Rep,
            default_reps: 10,
            default_weight: 45.0,
            target_seconds: 0.0,
        }
    }

    fn cardio(name: &str, secs: f64) -> ExerciseDef {
        ExerciseDef {
            name: name.into(),
            kind: ExerciseKind::Continuous,
            default_reps: 0,
            default_weight: 0.0,
            target_seconds: secs,
        }
    }

    fn engine() -> WorkoutEngine {
        WorkoutEngine::new(
            vec![lift("bench"), lift("row"), lift("squat")],
            vec![cardio("jumprope", 60.0), cardio("stretch", 30.0)],
            2,
        )
    }

    #[test]
    fn rotation_steps_and_wraps() {
        let mut w = engine();
        assert_eq!(w.prescribe("2026-07-19").unwrap().exercise, "bench");
        w.complete("2026-07-19");
        assert_eq!(w.prescribe("2026-07-19").unwrap().exercise, "row");
        // pointer survives without completing: same prescription again
        assert_eq!(w.prescribe("2026-07-19").unwrap().exercise, "row");
    }

    #[test]
    fn capacity_switches_to_continuous_pool() {
        let mut w = engine();
        w.prescribe("2026-07-19");
        w.complete("2026-07-19"); // capacity 1/2
        w.prescribe("2026-07-19");
        w.complete("2026-07-19"); // capacity 2/2 spent
        let p = w.prescribe("2026-07-19").unwrap();
        assert_eq!(p.kind, ExerciseKind::Continuous);
        assert_eq!(p.exercise, "jumprope");
        w.complete("2026-07-19"); // continuous set does NOT add capacity
        assert_eq!(w.capacity_used(), 2);
        assert_eq!(w.prescribe("2026-07-19").unwrap().exercise, "stretch");
    }

    #[test]
    fn new_day_resets_capacity_but_not_pointer() {
        let mut w = engine();
        w.prescribe("2026-07-19");
        w.complete("2026-07-19");
        w.prescribe("2026-07-19");
        w.complete("2026-07-19");
        assert_eq!(w.capacity_used(), 2);
        let p = w.prescribe("2026-07-20").unwrap(); // new day
        assert_eq!(w.capacity_used(), 0);
        assert_eq!(p.kind, ExerciseKind::Rep);
        assert_eq!(p.exercise, "squat"); // pointer kept from yesterday
    }

    #[test]
    fn restore_resumes_state() {
        let mut w = engine();
        w.restore(2, 1, "2026-07-19");
        assert_eq!(w.prescribe("2026-07-19").unwrap().exercise, "squat");
        assert_eq!(w.capacity_used(), 1);
    }

    #[test]
    fn empty_rotation_prescribes_continuous() {
        let mut w = WorkoutEngine::new(vec![], vec![cardio("jumprope", 60.0)], 2);
        assert_eq!(w.prescribe("2026-07-19").unwrap().exercise, "jumprope");
    }
}
