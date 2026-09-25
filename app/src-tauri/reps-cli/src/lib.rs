pub mod workday;
pub mod inspect;
pub mod commands;
pub mod control;
use std::path::Path;
use serde::{Serialize,Deserialize};
#[derive(Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Config {pub url:String,pub token:String}
pub fn config(home:&Path,name:&str)->Result<Config,String>{
 let path=home.join(name);
 #[cfg(unix)] {
  use std::os::unix::fs::PermissionsExt;
  if let Ok(meta)=std::fs::metadata(&path){if meta.permissions().mode() & 0o077 != 0{return Err(format!("Protect {name} with chmod 600 before use"));}}
 }
 let content=std::fs::read_to_string(path).map_err(|_|format!("Configure {name} in the Reps data directory first"))?;
 let c:Config=serde_json::from_str(&content).map_err(|_|format!("Invalid {name}"))?;
 let u=reqwest::Url::parse(&c.url).map_err(|_|"Invalid API URL")?;
 if u.scheme()!="https" && !(u.scheme()=="http"&&matches!(u.host_str(),Some("localhost"|"127.0.0.1"))){return Err("API must use HTTPS".into());}
 if !u.username().is_empty()||u.password().is_some(){return Err("Do not put credentials in URLs".into());}Ok(c)
}
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
 let records=s.records(None,None,0,100,true).map_err(|e|e.to_string())?;
 let result=request(&c,"POST","sync",Some(&serde_json::json!({"records":records})))?;
 let accepted:Vec<String>=serde_json::from_value(result["accepted"].clone()).map_err(|_|"Invalid sync acknowledgement")?;
 let ids:Vec<String>=accepted.into_iter().filter(|id|records.iter().any(|r|&r.id==id)).collect();
 s.acknowledge(&ids).map_err(|e|e.to_string())?;
 s.set_setting("sync_status",&format!("Synced {}",chrono::Utc::now().to_rfc3339())).map_err(|e|e.to_string())?;
 Ok(ids.len())
}
