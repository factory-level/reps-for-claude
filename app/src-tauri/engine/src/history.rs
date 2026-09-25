use serde::{Serialize,Deserialize};
#[derive(Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct HistoryRecord {
    #[serde(skip)] pub sequence:i64,
    pub id:String, pub date:String, pub exercise:String, pub kind:crate::types::ExerciseKind,
    pub reps:u32, pub seconds:f64, pub weight:f64, pub verified:bool,
    pub recorded_at:Option<String>,pub weight_unit:Option<String>,
}
#[cfg(test)] mod tests {
 use crate::{store::Store,types::{SetRecord,ExerciseKind}};
 #[test] fn migration_pending_and_readonly() {
  let dir=tempfile::tempdir().unwrap();let path=dir.path().join("reps.sqlite");
  let s=Store::open(&path).unwrap();
  s.record_set(&SetRecord{date:"2026-09-24".into(),exercise:"squat".into(),kind:ExerciseKind::Rep,reps:10,seconds:0.,weight:0.,verified:true}).unwrap();
  let ro=Store::open_readonly(&path).unwrap();let r=ro.records(None,None,0,10,false).unwrap();
  assert_eq!(r.len(),1);assert!(r[0].recorded_at.is_some());
  assert_eq!(ro.records(Some("2026-09-25"),None,0,10,false).unwrap().len(),0);
  s.acknowledge(&[r[0].id.clone()]).unwrap();assert!(s.records(None,None,0,10,true).unwrap().is_empty());
  assert!(ro.record_set(&SetRecord{date:"today".into(),exercise:"x".into(),kind:ExerciseKind::Rep,reps:1,seconds:0.,weight:0.,verified:false}).is_err());
 }
 #[test] fn readonly_does_not_create() {let d=tempfile::tempdir().unwrap();let p=d.path().join("missing");assert!(Store::open_readonly(&p).is_err());assert!(!p.exists());}
}
