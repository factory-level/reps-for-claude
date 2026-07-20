//! The application state machine (V1 spec §20):
//! CODING → EXERCISE_REQUIRED → WORKOUT_ACTIVE → WEIGHT_CONFIRMATION → UNLOCKED → CODING.
//! Pure: driven by tick()/commands with explicit time, no threads, no IO.

use crate::timer::CodingTimer;
use crate::types::{ExerciseKind, Phase, Prescription, Progress, SetRecord, Snapshot};
use crate::workout::WorkoutEngine;

pub struct Session {
    phase: Phase,
    timer: CodingTimer,
    workout: WorkoutEngine,
    prescription: Option<Prescription>,
    progress: Option<Progress>,
    pending_record: Option<SetRecord>,
}

impl Session {
    pub fn new(timer: CodingTimer, workout: WorkoutEngine) -> Self {
        Self {
            phase: Phase::Coding,
            timer,
            workout,
            prescription: None,
            progress: None,
            pending_record: None,
        }
    }

    pub fn start(&mut self, now: f64, _today: &str) {
        self.phase = Phase::Coding;
        self.timer.start(now);
    }

    pub fn tick(&mut self, now: f64, today: &str) -> bool {
        if self.phase == Phase::Coding && self.timer.expired(now) {
            self.timer.stop();
            self.prescription = self.workout.prescribe(today);
            self.progress = None;
            self.phase = Phase::ExerciseRequired;
            return true;
        }
        false
    }

    pub fn begin_workout(&mut self) {
        if self.phase == Phase::ExerciseRequired && self.prescription.is_some() {
            self.phase = Phase::WorkoutActive;
        }
    }

    pub fn report_progress(&mut self, p: Progress) {
        if self.phase != Phase::WorkoutActive {
            return;
        }
        let satisfied = p.satisfied;
        self.progress = Some(p);
        if !satisfied {
            return;
        }
        match self.prescription.as_ref().map(|rx| rx.kind) {
            Some(ExerciseKind::Rep) => self.phase = Phase::WeightConfirmation,
            Some(ExerciseKind::Continuous) => {
                let rx = self.prescription.as_ref().unwrap();
                self.pending_record = Some(SetRecord {
                    date: self.workout.capacity_date().to_string(),
                    exercise: rx.exercise.clone(),
                    kind: rx.kind,
                    reps: 0,
                    seconds: rx.target_seconds,
                    weight: 0.0,
                    verified: true,
                });
                let date = self.workout.capacity_date().to_string();
                self.workout.complete(&date);
                self.phase = Phase::Unlocked;
            }
            None => {}
        }
    }

    pub fn confirm_weight(&mut self, weight: f64, today: &str) -> Option<SetRecord> {
        if self.phase != Phase::WeightConfirmation {
            return None;
        }
        let rx = self.prescription.as_ref()?;
        let record = SetRecord {
            date: today.to_string(),
            exercise: rx.exercise.clone(),
            kind: rx.kind,
            reps: rx.target_reps,
            seconds: 0.0,
            weight,
            verified: true,
        };
        self.workout.complete(today);
        self.pending_record = Some(record.clone());
        self.phase = Phase::Unlocked;
        Some(record)
    }

    pub fn resume_coding(&mut self, now: f64) {
        if self.phase == Phase::Unlocked {
            self.prescription = None;
            self.progress = None;
            self.start(now, "");
        }
    }

    pub fn take_pending_record(&mut self) -> Option<SetRecord> {
        self.pending_record.take()
    }

    pub fn snapshot(&self, now: f64) -> Snapshot {
        Snapshot {
            phase: self.phase,
            remaining_seconds: self.timer.remaining(now),
            prescription: self.prescription.clone(),
            progress: self.progress.clone(),
            capacity_used: self.workout.capacity_used(),
            capacity_limit: self.workout.capacity_limit(),
            rotation: self.workout.rotation_names(),
            pointer: self.workout.pointer(),
        }
    }

    pub fn workout(&self) -> &WorkoutEngine {
        &self.workout
    }

    #[cfg(test)]
    pub fn workout_mut_for_test(&mut self) -> &mut WorkoutEngine {
        &mut self.workout
    }
}

#[cfg(test)]
mod tests {
    use crate::timer::CodingTimer;
    use crate::types::{ExerciseDef, ExerciseKind, Phase, Progress};
    use crate::workout::WorkoutEngine;

    fn session() -> super::Session {
        let rotation = vec![ExerciseDef {
            name: "bench".into(),
            kind: ExerciseKind::Rep,
            default_reps: 10,
            default_weight: 45.0,
            target_seconds: 0.0,
        }];
        let pool = vec![ExerciseDef {
            name: "jumprope".into(),
            kind: ExerciseKind::Continuous,
            default_reps: 0,
            default_weight: 0.0,
            target_seconds: 60.0,
        }];
        super::Session::new(CodingTimer::new(360.0), WorkoutEngine::new(rotation, pool, 20))
    }

    fn done(unit: &str, v: f64) -> Progress {
        Progress { value: v, unit: unit.into(), satisfied: true }
    }

    #[test]
    fn full_lift_cycle() {
        let mut s = session();
        s.start(0.0, "2026-07-19");
        assert_eq!(s.snapshot(0.0).phase, Phase::Coding);

        assert!(!s.tick(359.0, "2026-07-19"));
        assert!(s.tick(360.0, "2026-07-19"));
        let snap = s.snapshot(360.0);
        assert_eq!(snap.phase, Phase::ExerciseRequired);
        assert_eq!(snap.prescription.as_ref().unwrap().exercise, "bench");

        s.begin_workout();
        assert_eq!(s.snapshot(361.0).phase, Phase::WorkoutActive);

        s.report_progress(Progress { value: 3.0, unit: "reps".into(), satisfied: false });
        assert_eq!(s.snapshot(400.0).phase, Phase::WorkoutActive);

        s.report_progress(done("reps", 10.0));
        assert_eq!(s.snapshot(401.0).phase, Phase::WeightConfirmation);

        let rec = s.confirm_weight(145.0, "2026-07-19").unwrap();
        assert_eq!(rec.weight, 145.0);
        assert_eq!(rec.reps, 10);
        assert!(rec.verified);
        assert_eq!(s.snapshot(402.0).phase, Phase::Unlocked);

        s.resume_coding(500.0);
        let snap = s.snapshot(500.0);
        assert_eq!(snap.phase, Phase::Coding);
        assert_eq!(snap.remaining_seconds, 360.0);
        assert_eq!(snap.capacity_used, 1);
    }

    #[test]
    fn continuous_skips_weight_confirmation() {
        let mut s = session();
        // force capacity spent so prescription is continuous
        s.workout_mut_for_test().restore(0, 20, "2026-07-19");
        s.start(0.0, "2026-07-19");
        s.tick(360.0, "2026-07-19");
        assert_eq!(
            s.snapshot(360.0).prescription.as_ref().unwrap().kind,
            ExerciseKind::Continuous
        );
        s.begin_workout();
        s.report_progress(done("seconds", 60.0));
        assert_eq!(s.snapshot(361.0).phase, Phase::Unlocked);
        let rec = s.take_pending_record().unwrap();
        assert_eq!(rec.exercise, "jumprope");
        assert_eq!(rec.weight, 0.0);
    }

    #[test]
    fn guards_ignore_wrong_phase_calls() {
        let mut s = session();
        s.start(0.0, "2026-07-19");
        s.begin_workout(); // Coding: no-op
        assert_eq!(s.snapshot(1.0).phase, Phase::Coding);
        assert!(s.confirm_weight(100.0, "2026-07-19").is_none());
        s.report_progress(done("reps", 10.0)); // no active workout: no-op
        assert_eq!(s.snapshot(2.0).phase, Phase::Coding);
    }

    #[test]
    fn timer_stops_while_locked() {
        let mut s = session();
        s.start(0.0, "2026-07-19");
        s.tick(360.0, "2026-07-19");
        // long workout: coding timer must not be running
        assert_eq!(s.snapshot(10_000.0).remaining_seconds, 360.0);
    }
}
