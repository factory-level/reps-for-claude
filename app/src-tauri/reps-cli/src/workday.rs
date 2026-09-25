use chrono::Timelike;
pub fn parse_time(s:&str)->Result<u32,String>{
 let parts:Vec<_>=s.split(':').collect();
 if parts.len()!=2||parts.iter().any(|s|s.len()!=2||!s.bytes().all(|b|b.is_ascii_digit())){return Err("End time must be HH:MM (24-hour local time)".into());}
 let h=parts[0].parse::<u32>().unwrap();let m=parts[1].parse::<u32>().unwrap();
 if h>23||m>59{return Err("Invalid workday end time".into());}Ok(h*60+m)
}
pub fn local_minute()->u32{let n=chrono::Local::now();n.hour()*60+n.minute()}
/// One warning per local date, only while the user is working and a set remains.
pub fn due(minute:u32,end:u32,lead:u32,eligible:bool,remaining:u32,already_sent:bool)->bool{
 eligible&&remaining>0&&!already_sent&&minute>=end.saturating_sub(lead)
}
#[cfg(test)] mod tests{
 use super::*;
 #[test] fn validates_clock(){assert_eq!(parse_time("18:00").unwrap(),1080);for s in ["24:00","18:60","6:00","-1:00"]{assert!(parse_time(s).is_err());}}
 #[test] fn warning_is_bounded_and_catches_late_login(){assert!(!due(1019,1080,60,true,2,false));assert!(due(1020,1080,60,true,2,false));assert!(due(1200,1080,60,true,2,false));assert!(!due(1020,1080,60,false,2,false));assert!(!due(1020,1080,60,true,0,false));assert!(!due(1020,1080,60,true,2,true));}
}
