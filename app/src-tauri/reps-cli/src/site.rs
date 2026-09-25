//! Endpoint-scoped credentials. Output never includes bearer tokens.
use std::{collections::BTreeMap,path::Path};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use crate::{Config,request};
pub const DEFAULT_URL:&str="https://reps-for-prompts.vercel.app";
#[derive(Clone,Default,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
struct Site { upload_token:Option<String>, read_token:Option<String> }
#[derive(Default,Serialize,Deserialize)]
struct Sites { active:String, endpoints:BTreeMap<String,Site> }
pub fn normalize(raw:&str)->Result<String,String>{
 let u=reqwest::Url::parse(raw).map_err(|_|"Invalid site URL")?;
 let local=matches!(u.host_str(),Some("localhost"|"127.0.0.1"|"[::1]"));
 if u.scheme()!="https"&&!(u.scheme()=="http"&&local){return Err("Use HTTPS, or HTTP on localhost for testing".into());}
 if u.host_str().is_none()||!u.username().is_empty()||u.password().is_some()||u.query().is_some()||u.fragment().is_some()||u.path()!="/"{return Err("Use a site origin only, without credentials, path, query or fragment".into());}
 Ok(u.as_str().trim_end_matches('/').to_string())
}
fn load(home:&Path)->Result<Sites,String>{
 let path=home.join("sites.json");
 if path.exists(){crate::private_file(&path)?;return serde_json::from_slice(&std::fs::read(path).map_err(|_|"Cannot read sites.json")?).map_err(|_|"Invalid sites.json".into());}
 let mut sites=Sites{active:DEFAULT_URL.into(),..Default::default()};
 for (file,upload) in [("upload.json",true),("remote.json",false)]{
  if home.join(file).exists(){let c=crate::legacy_config(home,file)?;let url=normalize(&c.url)?;
   if upload{sites.active=url.clone();}let site=sites.endpoints.entry(url).or_default();
   if upload{site.upload_token=Some(c.token);}else{site.read_token=Some(c.token);}
  }
 }
 Ok(sites)
}
fn save(home:&Path,sites:&Sites)->Result<(),String>{
 use std::io::Write;
 std::fs::create_dir_all(home).map_err(|e|e.to_string())?;
 let path=home.join(format!("sites-{}.tmp",std::process::id()));
 let mut options=std::fs::OpenOptions::new();options.write(true).create_new(true);
 #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
 let mut file=options.open(&path).map_err(|e|e.to_string())?;
 let result=(||{file.write_all(&serde_json::to_vec(sites).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;file.sync_all().map_err(|e|e.to_string())?;std::fs::rename(&path,home.join("sites.json")).map_err(|e|e.to_string())})();
 if result.is_err(){let _=std::fs::remove_file(path);}result
}
pub fn active_config(home:&Path,upload:bool)->Result<Config,String>{
 let sites=load(home)?;let url=normalize(&sites.active)?;
 let site=sites.endpoints.get(&url).ok_or("Configure this endpoint with rfp site --url URL --upload-token-file PATH")?;
 let token=if upload{&site.upload_token}else{&site.read_token};
 Ok(Config{url,token:token.clone().ok_or(if upload{"Upload credential missing for this endpoint"}else{"Read credential missing for this endpoint; use rfp site --read-token-file PATH"})?})
}
fn token(path:&str)->Result<String,String>{
 let p=Path::new(path);crate::private_file(p)?;
 let s=std::fs::read_to_string(p).map_err(|_|"Cannot read token file")?.trim().to_string();
 if s.len()<32||!s.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'_'||c==b'-'){return Err("Token file must contain only the issued bearer token".into());}Ok(s)
}
pub fn execute(home:&Path,args:&[String])->Result<Value,String>{
 let mut opts=BTreeMap::new();let profile=args[0]=="profile";let mut i=1;let mut clear_location=false;
 while i<args.len(){if args[i]=="--json"{i+=1;continue;}
 if profile&&args[i]=="--clear-location"{if clear_location{return Err("Duplicate option".into());}clear_location=true;i+=1;continue;}
 let allowed=if profile{vec!["--nickname","--sharing","--location"]}else{vec!["--url","--upload-token-file","--read-token-file"]};
 if !allowed.contains(&args[i].as_str()){return Err(format!("Unknown option {}",args[i]));}
 let value=args.get(i+1).ok_or("Missing option value")?;if opts.insert(args[i].as_str(),value.as_str()).is_some(){return Err("Duplicate option".into());}i+=2;
 }
 if profile{
  if clear_location&&opts.contains_key("--location"){return Err("Choose --location or --clear-location, not both".into());}
  let mut payload=json!({});
  if clear_location{payload["location"]=Value::Null;}
  if let Some(label)=opts.get("--location"){let label=label.trim();if label.is_empty()||label.encode_utf16().count()>80{return Err("Location must be 1–80 characters".into());}payload["location"]=json!(label);}
  let config=active_config(home,true)?;
  if let Some(n)=opts.get("--nickname"){let n=n.trim();if n.is_empty()||n.chars().count()>32{return Err("Nickname must be 1–32 characters".into());}payload["nickname"]=json!(n);}
  if let Some(v)=opts.get("--sharing"){payload["sharing"]=json!(match *v{"on"=>true,"off"=>false,_=>return Err("Sharing must be on or off".into())});}
  let editing=!opts.is_empty()||clear_location;
  let mut result=request(&config,if editing{"POST"}else{"GET"},"profile",if editing{Some(&payload)}else{None})?;
  result["endpoint"]=json!(config.url);return Ok(result);
 }
 let mut sites=load(home)?;
 if !opts.is_empty(){
  if let Ok(runtime)=crate::control::send(home,&crate::control::Command::Snapshot){
   if runtime["capabilities"]["siteProfiles"]!=true{return Err("The running daemon needs the site-profile update. Finish your current set, then restart rfp.service before switching endpoints.".into());}
  }
  let url=normalize(opts.get("--url").copied().unwrap_or(&sites.active))?;
  let is_new=!sites.endpoints.contains_key(&url);
  let mut selected=sites.endpoints.get(&url).cloned().unwrap_or_default();
  if let Some(path)=opts.get("--upload-token-file"){selected.upload_token=Some(token(path)?);}
  if let Some(path)=opts.get("--read-token-file"){selected.read_token=Some(token(path)?);}
  let config=Config{url:url.clone(),token:selected.upload_token.clone().ok_or("A new endpoint needs its own --upload-token-file; production credentials are never copied")?};
  // Authenticate the target before changing the active destination. New sites share future results by default.
  let defaults=json!({"sharing":true});
  request(&config,if is_new{"POST"}else{"GET"},"profile",if is_new{Some(&defaults)}else{None})?;
  sites.endpoints.insert(url.clone(),selected);sites.active=url;save(home,&sites)?;
 }
 Ok(json!({"schemaVersion":1,"endpoint":sites.active,"automaticUpload":true,"endpoints":sites.endpoints.iter().map(|(url,s)|json!({"url":url,"uploadConfigured":s.upload_token.is_some(),"readConfigured":s.read_token.is_some()})).collect::<Vec<_>>() }))
}
#[cfg(test)] mod tests{
 use super::*;
 #[test] fn location_flags_reject_conflicts_and_invalid_labels(){
  let d=tempfile::tempdir().unwrap();
  for flags in [vec!["--location","Oakland","--clear-location"],vec!["--location","   "],vec!["--clear-location","--clear-location"]]{let mut args=vec!["profile".to_string()];args.extend(flags.into_iter().map(str::to_string));let error=execute(d.path(),&args).unwrap_err();assert!(!error.contains("credential"));}
 }
 #[test] fn restricts_endpoint_origins(){assert_eq!(normalize("http://localhost:3000/").unwrap(),"http://localhost:3000");assert!(normalize("http://[::1]:3000").is_ok());for s in ["http://example.com","https://user:secret@example.com","https://example.com/api","https://example.com?token=x"]{assert!(normalize(s).is_err());}}
 #[test] fn separate_sites_do_not_reuse_tokens(){let d=tempfile::tempdir().unwrap();let mut s=Sites{active:"http://localhost:3000".into(),..Default::default()};s.endpoints.insert(DEFAULT_URL.into(),Site{upload_token:Some("secret".into()),read_token:None});save(d.path(),&s).unwrap();assert!(active_config(d.path(),true).is_err());s.active=DEFAULT_URL.into();save(d.path(),&s).unwrap();assert_eq!(active_config(d.path(),true).unwrap().token,"secret");let v=execute(d.path(),&["site".into()]).unwrap();assert!(!v.to_string().contains("secret"));}
}
