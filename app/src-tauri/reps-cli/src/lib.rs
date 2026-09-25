pub mod site;
pub mod workday;
pub mod inspect;
pub mod commands;
pub mod control;
use std::path::Path;
use serde::{Serialize,Deserialize};
#[derive(Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Config {pub url:String,pub token:String}
pub fn private_file(path:&Path)->Result<(),String>{
 #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;if std::fs::metadata(path).map_err(|_|"Credential file not found")?.permissions().mode() & 0o077 != 0{return Err("Credential files must be private (chmod 600)".into());}}
 Ok(())
}
pub fn legacy_config(home:&Path,name:&str)->Result<Config,String>{
 let path=home.join(name);private_file(&path)?;
 let c:Config=serde_json::from_slice(&std::fs::read(path).map_err(|_|"Cannot read credential file")?).map_err(|_|"Invalid credential file")?;
 site::normalize(&c.url)?;Ok(c)
}
pub fn config(home:&Path,name:&str)->Result<Config,String>{site::active_config(home,name=="upload.json")}
pub fn request(c:&Config,method:&str,path:&str,body:Option<&serde_json::Value>)->Result<serde_json::Value,String>{
 let client=reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(20)).redirect(reqwest::redirect::Policy::none()).build().map_err(|_|"HTTP client unavailable")?;
 let url=format!("{}/api/v1/{path}",c.url.trim_end_matches('/'));
 let mut r=client.request(if method=="POST"{reqwest::Method::POST}else{reqwest::Method::GET},url).bearer_auth(&c.token);
 if let Some(b)=body{r=r.json(b);}
 let response=r.send().map_err(|_|"Could not reach Reps server; queued workouts remain local")?;
 if !response.status().is_success(){return Err(format!("Server returned {}",response.status()));}
 response.json().map_err(|_|"Invalid server response".into())
}
pub fn sync(home:&Path)->Result<usize,String>{
 let c=config(home,"upload.json")?;
 let s=engine::store::Store::open(&home.join("reps.sqlite")).map_err(|e|e.to_string())?;
 // Capture today's real routine without restarting an older running desktop.
 if let Ok(runtime)=control::send(home,&control::Command::Snapshot){
  let state:serde_json::Value=serde_json::from_str(&s.setting("plan_state","{}" )).unwrap_or_default();
  let today=chrono::Local::now().format("%Y-%m-%d").to_string();
  if runtime["mode"]=="workout" && runtime["sessionHome"].as_str()==home.to_str() && state["date"]==today{
   let day=&runtime["snapshot"]["day"];
   if let (Some(done),Some(target))=(day["setsDone"].as_u64(),day["setsTotal"].as_u64()){
    if done<=target && target<=100000{s.record_routine_day(&today,done as u32,target as u32).map_err(|e|e.to_string())?;}
   }
  }
 }
 use sha2::{Digest,Sha256};
 let destination=format!("{:x}",Sha256::digest(format!("{}\n{}",c.url,c.token)));
 let records=s.pending_for_destination(&destination,100).map_err(|e|e.to_string())?;
 let result=request(&c,"POST","sync",Some(&serde_json::json!({"records":records,"routineDays":s.routine_days().map_err(|e|e.to_string())?})))?;
 let accepted:Vec<String>=serde_json::from_value(result["accepted"].clone()).map_err(|_|"Invalid sync acknowledgement")?;
 let ids:Vec<String>=accepted.into_iter().filter(|id|records.iter().any(|r|&r.id==id)).collect();
 s.acknowledge_destination(&destination,&ids).map_err(|e|e.to_string())?;
 s.set_setting("sync_status",&format!("Synced to {} at {}",c.url,chrono::Utc::now().to_rfc3339())).map_err(|e|e.to_string())?;
 Ok(ids.len())
}
