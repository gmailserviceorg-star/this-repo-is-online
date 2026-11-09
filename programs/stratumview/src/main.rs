//! StratumView: engine selection + health check for the WebView fallback chain, and `open URL`.
use serde::Deserialize;
use std::{collections::HashMap,fs,os::unix::process::CommandExt,path::Path,process::{Command,Stdio},thread::sleep,time::Duration};
#[derive(Deserialize)] struct Cfg{order:Vec<String>,engine:HashMap<String,Eng>}
#[derive(Deserialize)] struct Eng{bin:String,open:Vec<String>,#[serde(default)]health:Vec<String>}
fn healthy(e:&Eng)->bool{
 if !Path::new(&e.bin).exists(){return false}
 if e.health.is_empty(){return true}
 let mut ch=match Command::new(&e.health[0]).args(&e.health[1..]).stdout(Stdio::null()).stderr(Stdio::null()).spawn(){Ok(c)=>c,Err(_)=>return false};
 for _ in 0..30{if let Ok(Some(s))=ch.try_wait(){return s.success()}sleep(Duration::from_millis(100))}
 let _=ch.kill();false}
fn main(){
 let a:Vec<String>=std::env::args().skip(1).collect();
 let p=std::env::var("STRATUMVIEW_CONF").unwrap_or("/System/etc/webview.toml".into());
 let c:Cfg=match fs::read_to_string(&p).map_err(|e|e.to_string()).and_then(|s|toml::from_str(&s).map_err(|e|e.to_string())){Ok(c)=>c,Err(e)=>{eprintln!("stratumview: {p}: {e}");std::process::exit(2)}};
 let pick=||c.order.iter().find(|n|c.engine.get(*n).map(healthy).unwrap_or(false)).cloned();
 match a.first().map(|s|s.as_str()){
  Some("status")=>for n in &c.order{println!("{n}: {}",if c.engine.get(n).map(healthy).unwrap_or(false){"OK"}else{"unavailable"})},
  Some("select")=>match pick(){Some(n)=>{let _=fs::write("/run/stratumview.engine",&n);println!("{n}")}None=>{eprintln!("no web engine available");std::process::exit(1)}},
  Some("open")=>{let url=a.get(1).cloned().unwrap_or("about:blank".into());
   let n=pick().unwrap_or_else(||{eprintln!("no web engine available");std::process::exit(1)});
   let e=&c.engine[&n];let args:Vec<String>=e.open.iter().map(|x|x.replace("{url}",&url)).collect();
   eprintln!("stratumview: using {n}");let err=Command::new(&args[0]).args(&args[1..]).exec();eprintln!("{err}");std::process::exit(1)}
  _=>{eprintln!("usage: stratumview status|select|open URL");std::process::exit(2)}}}
