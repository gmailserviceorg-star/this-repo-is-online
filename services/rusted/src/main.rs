//! Rusted: boot verifier, integrity watch + rollback, sandbox launcher, audit, permission IPC.
use serde::Deserialize;
use std::{collections::HashMap,fs,io::{BufRead,BufReader,Write},os::unix::{fs::PermissionsExt,net::{UnixListener,UnixStream},process::CommandExt},path::Path,process::Command,sync::Arc,thread::{sleep,spawn},time::Duration};
use stw_pkg::sig;
#[derive(Deserialize)] struct Cfg{boot:Boot,watch:Watch,audit:Audit,ipc:Ipc,#[serde(default)]profile:HashMap<String,Prof>}
#[derive(Deserialize)] struct Boot{manifest:String,sig:String,pubkeys:String,safe_flag:String}
#[derive(Deserialize)] struct Watch{interval_s:u64,upper:String,lock:Vec<String>}
#[derive(Deserialize)] struct Audit{log:String}
#[derive(Deserialize)] struct Ipc{socket:String,prompt_cmd:String}
#[derive(Deserialize,Default)] struct Prof{#[serde(default)]ro:Vec<String>,#[serde(default)]rw:Vec<String>,#[serde(default)]net:bool,uid:Option<u32>,gid:Option<u32>}
fn es<E:ToString>(x:E)->String{x.to_string()}
fn audit(c:&Cfg,m:&str){
 let t=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs()).unwrap_or(0);
 eprintln!("rusted: {m}");
 if let Ok(mut f)=fs::OpenOptions::new().create(true).append(true).open(&c.audit.log){let _=writeln!(f,"{t} {m}");}}
/// verifies manifest signature, returns (sha256,path) entries
fn load(c:&Cfg)->Result<Vec<(String,String)>,String>{
 let m=fs::read(&c.boot.manifest).map_err(es)?;
 let s=fs::read_to_string(&c.boot.sig).map_err(es)?;
 if !sig::verify(&sig::trusted(&c.boot.pubkeys),&m,&s){return Err("manifest signature invalid".into())}
 let mut v=vec![];
 for l in String::from_utf8_lossy(&m).lines(){let l=l.trim();if l.is_empty()||l.starts_with('#'){continue}
  let(h,p)=l.split_once("  ").ok_or("bad manifest line")?;v.push((h.to_string(),p.to_string()))}
 Ok(v)}
fn bad_paths(v:&[(String,String)])->Vec<String>{
 v.iter().filter(|(h,p)|{
  if !Path::new(p).exists()&&p.starts_with("/mnt/iso/"){return false}
  fs::read(p).map(|d|sig::sha256(&d)!=*h).unwrap_or(true)}).map(|(_,p)|p.clone()).collect()}
fn rollback(c:&Cfg,p:&str)->bool{let u=format!("{}{}",c.watch.upper,p);Path::new(&u).exists()&&fs::remove_file(&u).is_ok()}
fn safe(c:&Cfg,why:&str){
 audit(c,&format!("SAFE MODE: {why}"));let _=fs::write(&c.boot.safe_flag,why);
 let _=Command::new("sv").args(["restart","/etc/service/greetd"]).status();}
fn mnt(a:&[&str]){let _=Command::new("mount").args(a).status();}
fn remount(c:&Cfg,m:&str){for p in &c.watch.lock{mnt(&["-o",format!("remount,{m},bind").as_str(),p.as_str()])}}
fn check(c:&Cfg)->bool{
 if Path::new(&c.boot.safe_flag).exists(){return false}
 match load(c){
  Err(x)=>{safe(c,&x);false}
  Ok(v)=>{
   for p in bad_paths(&v){if rollback(c,&p){audit(c,&format!("ROLLBACK {p}"))}else{audit(c,&format!("TAMPER {p}"))}}
   let rest=bad_paths(&v);
   if rest.is_empty(){true}else{safe(c,&format!("integrity failure: {}",rest.join(",")));false}}}}
fn ipc(c:Arc<Cfg>){
 let _=fs::remove_file(&c.ipc.socket);
 let l=match UnixListener::bind(&c.ipc.socket){Ok(l)=>l,Err(x)=>{audit(&c,&format!("ipc bind: {x}"));return}};
 let _=fs::set_permissions(&c.ipc.socket,fs::Permissions::from_mode(0o666));
 for s in l.incoming().flatten(){handle(&c,s)}}
fn handle(c:&Arc<Cfg>,mut s:UnixStream){
 let mut line=String::new();
 if BufReader::new(&s).read_line(&mut line).is_err(){return}
 let w:Vec<&str>=line.trim().splitn(2,' ').collect();
 let r:String=match w.as_slice(){
  ["status"]=>if Path::new(&c.boot.safe_flag).exists(){"safe".into()}else{"ok".into()},
  ["unlock",p]=>match sig::verify_stw(p,&sig::trusted(&c.boot.pubkeys)){
   Ok(_)=>{audit(c,&format!("UNLOCK by signed pkg {p}"));remount(c,"rw");let c2=c.clone();spawn(move||{sleep(Duration::from_secs(120));remount(&c2,"ro");audit(&c2,"relocked")});"unlocked 120s".into()}
   Err(x)=>{audit(c,&format!("UNLOCK denied {p}: {x}"));format!("denied: {x}")}},
  ["ask",t]=>{let ok=Command::new(&c.ipc.prompt_cmd).arg(t).status().map(|s|s.success()).unwrap_or(false);audit(c,&format!("ASK '{t}' -> {ok}"));if ok{"allow".into()}else{"deny".into()}}
  _=>"error".into()};
 let _=writeln!(s,"{r}");}
fn daemon(c:Cfg){
 audit(&c,"start");
 if check(&c){audit(&c,"boot verify ok")}
 for p in &c.watch.lock{mnt(&["--bind",p.as_str(),p.as_str()])}remount(&c,"ro");
 let c=Arc::new(c);{let c2=c.clone();spawn(move||ipc(c2));}
 loop{sleep(Duration::from_secs(c.watch.interval_s));check(&c);}}
fn run(c:&Cfg,name:&str,cmd:&[String])->Result<(),String>{
 use landlock::{Access,AccessFs,PathBeneath,PathFd,Ruleset,RulesetAttr,RulesetCreatedAttr,ABI};
 use nix::sched::{unshare,CloneFlags};
 let pr=c.profile.get(name).ok_or("no such profile")?;if cmd.is_empty(){return Err("no command".into())}
 let abi=ABI::V2;
 let mut rs=Ruleset::default().handle_access(AccessFs::from_all(abi)).map_err(es)?.create().map_err(es)?;
 for p in pr.ro.iter().filter(|p|Path::new(p).exists()){rs=rs.add_rule(PathBeneath::new(PathFd::new(p).map_err(es)?,AccessFs::from_read(abi))).map_err(es)?}
 for p in pr.rw.iter().filter(|p|Path::new(p).exists()){rs=rs.add_rule(PathBeneath::new(PathFd::new(p).map_err(es)?,AccessFs::from_all(abi))).map_err(es)?}
 rs.restrict_self().map_err(es)?;
 let mut fl=CloneFlags::CLONE_NEWNS|CloneFlags::CLONE_NEWIPC|CloneFlags::CLONE_NEWUTS;if !pr.net{fl|=CloneFlags::CLONE_NEWNET}
 unshare(fl).map_err(es)?;
 let mut k=Command::new(&cmd[0]);k.args(&cmd[1..]);if let Some(u)=pr.uid{k.uid(u);}if let Some(g)=pr.gid{k.gid(g);}
 Err(k.exec().to_string())}
fn main(){
 let mut a:Vec<String>=std::env::args().skip(1).collect();
 let mut cf="/System/etc/rusted/rusted.toml".to_string();
 if a.first().map(|s|s=="-c").unwrap_or(false)&&a.len()>2{cf=a[1].clone();a.drain(..2);}
 let c:Cfg=match fs::read_to_string(&cf).map_err(es).and_then(|s|toml::from_str(&s).map_err(es)){Ok(c)=>c,Err(x)=>{eprintln!("rusted: config {cf}: {x}");std::process::exit(2)}};
 let r=match a.first().map(|s|s.as_str()){
  Some("daemon")=>{daemon(c);Ok(())}
  Some("verify")=>load(&c).and_then(|v|{let b=bad_paths(&v);if b.is_empty(){println!("ok");Ok(())}else{Err(format!("bad: {}",b.join(", ")))}}),
  Some("run")=>{let d=a.iter().position(|x|x=="--").unwrap_or(a.len());run(&c,a.get(1).map(|s|s.as_str()).unwrap_or(""),&a[(d+1).min(a.len())..])}
  Some("ctl")=>UnixStream::connect(&c.ipc.socket).map_err(es).and_then(|mut s|{writeln!(s,"{}",a[1..].join(" ")).map_err(es)?;let mut r=String::new();BufReader::new(s).read_line(&mut r).map_err(es)?;print!("{r}");Ok(())}),
  _=>Err("usage: rusted [-c cfg] daemon|verify|run PROFILE -- CMD..|ctl status|unlock PKG|ask TEXT".into())};
 if let Err(x)=r{eprintln!("rusted: {x}");std::process::exit(1)}}
