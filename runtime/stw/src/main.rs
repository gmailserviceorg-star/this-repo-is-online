//! Multi-call binary: stw, pkg, unpkg, stwc (dispatch on argv[0]).
use std::{collections::HashSet,env,fs,io::{self,Write},os::unix::{fs::{PermissionsExt,symlink},process::CommandExt},path::{Path,PathBuf},process::{exit,Command}};
use stw_lang::*;
use stw_pkg::{sig,Pkg};
type R=Result<(),String>;
fn e<E:ToString>(x:E)->String{x.to_string()}
const SYM:u32=0o120000;
fn main(){
 let a:Vec<String>=env::args().collect();
 let me=Path::new(&a[0]).file_name().and_then(|s|s.to_str()).unwrap_or("stw").to_string();
 let r=match me.as_str(){"unpkg"=>unpkg(&a[1..]),"pkg"=>pkg(&a[1..]),"stwc"=>stwc(&a[1..]),_=>stw(&a[1..])};
 if let Err(x)=r{eprintln!("{me}: {x}");exit(1)}}
fn safe(n:&str)->bool{!n.starts_with('/')&&!n.split('/').any(|s|s==".."||s.is_empty())}
fn extract(p:&mut Pkg,dir:&Path)->R{
 let names:Vec<(String,u32)>=p.idx.iter().map(|(k,v)|(k.clone(),v.mode)).collect();
 for(n,m)in names{
  if !safe(&n){return Err(format!("unsafe path {n}"))}
  let d=p.get(&n).map_err(e)?;let t=dir.join(&n);
  if let Some(x)=t.parent(){fs::create_dir_all(x).map_err(e)?}
  let _=fs::remove_file(&t);
  if m&0o170000==SYM{symlink(String::from_utf8_lossy(&d).to_string(),&t).map_err(e)?}
  else{fs::write(&t,d).map_err(e)?;fs::set_permissions(&t,fs::Permissions::from_mode(m&0o7777)).map_err(e)?}}
 Ok(())}
fn unpkg(a:&[String])->R{
 let(mut list,mut dest,mut f)=(false,None,None);let mut i=0;
 while i<a.len(){match a[i].as_str(){"-l"=>list=true,"-C"=>{i+=1;dest=a.get(i).map(PathBuf::from)}x=>f=Some(x.to_string())}i+=1}
 let f=f.ok_or("usage: unpkg [-l] [-C dir] file.pkg")?;
 let mut p=Pkg::open(Path::new(&f)).map_err(e)?;
 if list{for(n,en)in &p.idx{println!("{:>10} {:o} {n}",en.size,en.mode)}return Ok(())}
 let d=dest.unwrap_or_else(||Path::new(&f).with_extension(""));
 extract(&mut p,&d)}
fn add_path(p:&mut Pkg,base:&Path,rel:&Path)->R{
 let full=base.join(rel);let md=fs::symlink_metadata(&full).map_err(e)?;
 if md.file_type().is_symlink(){
  let t=fs::read_link(&full).map_err(e)?;p.put(&rel.to_string_lossy(),t.to_string_lossy().as_bytes(),SYM|0o777).map_err(e)?}
 else if md.is_dir(){for en in fs::read_dir(&full).map_err(e)?{let en=en.map_err(e)?;add_path(p,base,&rel.join(en.file_name()))?}}
 else if md.is_file(){p.put(&rel.to_string_lossy(),&fs::read(&full).map_err(e)?,md.permissions().mode()&0o7777).map_err(e)?}
 Ok(())}
fn compact(f:&Path)->R{
 let mut s=Pkg::open(f).map_err(e)?;let t=f.with_extension("tmp");let mut d=Pkg::create(&t).map_err(e)?;
 let names:Vec<(String,u32)>=s.idx.iter().map(|(k,v)|(k.clone(),v.mode)).collect();
 for(n,m)in names{let b=s.get(&n).map_err(e)?;d.put(&n,&b,m).map_err(e)?}
 d.commit().map_err(e)?;fs::rename(&t,f).map_err(e)}
fn pkg(a:&[String])->R{
 let u="usage: pkg create|ls|cat|put|patch|rm|compact FILE.pkg ...";
 let(c,f)=(a.first().ok_or(u)?.as_str(),a.get(1).ok_or(u)?);
 let fp=Path::new(f);
 match c{
  "create"=>{let mut p=Pkg::create(fp).map_err(e)?;
   for x in &a[2..]{let x=Path::new(x);let(b,r)=if x.is_dir(){(x.to_path_buf(),PathBuf::new())}else{(x.parent().unwrap_or(Path::new("")).to_path_buf(),PathBuf::from(x.file_name().unwrap()))};add_path(&mut p,&b,&r)?}
   p.commit().map_err(e)}
  "ls"=>{let p=Pkg::open(fp).map_err(e)?;for(n,en)in &p.idx{println!("{:>10} {:o} {n}",en.size,en.mode)}Ok(())}
  "cat"=>{let mut p=Pkg::open(fp).map_err(e)?;let d=p.get(a.get(2).ok_or(u)?).map_err(e)?;io::stdout().write_all(&d).map_err(e)}
  "put"=>{let mut p=Pkg::open(fp).map_err(e)?;let n=a.get(2).ok_or(u)?;let s=a.get(3).ok_or(u)?;
   p.put(n,&fs::read(s).map_err(e)?,0o644).map_err(e)?;p.commit().map_err(e)}
  "patch"=>{let mut p=Pkg::open(fp).map_err(e)?;let n=a.get(2).ok_or(u)?;let off:u64=a.get(3).ok_or(u)?.parse().map_err(e)?;
   p.patch(n,off,&fs::read(a.get(4).ok_or(u)?).map_err(e)?).map_err(e)?;p.commit().map_err(e)}
  "rm"=>{let mut p=Pkg::open(fp).map_err(e)?;if !p.remove(a.get(2).ok_or(u)?){return Err("no such file".into())}p.commit().map_err(e)}
  "compact"=>compact(fp),
  _=>Err(u.into())}}
fn trusted_default()->Vec<Vec<u8>>{sig::trusted(&env::var("STW_TRUST").unwrap_or("/System/etc/rusted/trusted.pub".into()))}
fn txt(d:&roxmltree::Document,t:&str)->Option<String>{d.descendants().find(|n|n.is_element()&&n.tag_name().name()==t).and_then(|n|n.text()).map(|s|s.trim().to_string())}
fn all(d:&roxmltree::Document,t:&str)->Vec<String>{d.descendants().filter(|n|n.is_element()&&n.tag_name().name()==t).filter_map(|n|n.text()).map(|s|s.trim().to_string()).collect()}
fn esc(s:&str)->String{s.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace('"',"&quot;")}
fn ask(q:&str)->bool{print!("{q} [y/N] ");let _=io::stdout().flush();let mut l=String::new();let _=io::stdin().read_line(&mut l);l.trim().eq_ignore_ascii_case("y")}
fn stw(a:&[String])->R{
 let u="usage: stw keygen|pub|sign-file|verify-file|verify|install|run|strata ...";
 let arg=|i:usize|a.get(i).ok_or(u.to_string());
 match arg(0)?.as_str(){
  "keygen"=>{let s=sig::keygen().map_err(e)?;fs::write(arg(1)?,sig::hex(&s)).map_err(e)?;println!("{}",sig::hex(&sig::pubkey(&s)));Ok(())}
  "pub"=>{println!("{}",sig::hex(&sig::pubkey(&sig::seed_from(arg(1)?)?)));Ok(())}
  "sign-file"=>{let s=sig::seed_from(arg(1)?)?;println!("{}",sig::sign(&s,&fs::read(arg(2)?).map_err(e)?));Ok(())}
  "verify-file"=>{let ok=sig::verify(&sig::trusted(arg(1)?),&fs::read(arg(2)?).map_err(e)?,&fs::read_to_string(arg(3)?).map_err(e)?);if ok{println!("ok");Ok(())}else{Err("BAD SIGNATURE".into())}}
  "verify"=>{sig::verify_stw(arg(1)?,&trusted_default())?;println!("ok");Ok(())}
  "install"=>install(arg(1)?),
  "run"=>run(arg(1)?,&a[2..]),
  "strata"=>strata(&a[1..]),
  _=>Err(u.into())}}
fn manifest(p:&mut Pkg)->Result<String,String>{String::from_utf8(p.get("Softwaremanifest.xml").map_err(e)?).map_err(e)}
fn okid(id:&str)->Result<(),String>{if !id.is_empty()&&id.chars().all(|c|c.is_alphanumeric()||"._-".contains(c))&&!id.starts_with('.'){Ok(())}else{Err("bad <id>".into())}}
fn install(f:&str)->R{
 let mut p=sig::verify_stw(f,&trusted_default())?;
 let man=manifest(&mut p)?;let md=roxmltree::Document::parse(&man).map_err(e)?;
 let id=txt(&md,"id").ok_or("manifest: no <id>")?;okid(&id)?;
 let au=String::from_utf8(p.get("Automated.xml").map_err(e)?).map_err(e)?;let ad=roxmltree::Document::parse(&au).map_err(e)?;
 let pre=fs::read_to_string("/var/lib/stw/preapproved").unwrap_or_default();let pre=pre.lines().any(|l|l.trim()==id);
 for(tag,file)in [("tos","Termsofservice.md"),("privacy","PrivacyPolicy.md")]{
  let body=p.get(file).map_err(e)?;let h=sig::sha256(&body);
  let n=ad.descendants().find(|n|n.tag_name().name()==tag).ok_or(format!("Automated.xml: no <{tag}>"))?;
  if n.attribute("sha256")!=Some(h.as_str()){return Err(format!("{file} does not match Automated.xml"))}
  let auto=n.attribute("accept")==Some("preapproved")&&pre;
  if !auto{println!("{}\n",String::from_utf8_lossy(&body));if !ask(&format!("Accept {file}?")){return Err("declined".into())}}
  let _=fs::create_dir_all("/var/lib/stw");
  if let Ok(mut l)=fs::OpenOptions::new().create(true).append(true).open("/var/lib/stw/accepted.log"){
   let t=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs()).unwrap_or(0);
   let _=writeln!(l,"{t} {id} {tag} {h} {}",if auto{"preapproved"}else{"user"});}}
 let app=p.get("App.pkg").map_err(e)?;let tmp=env::temp_dir().join(format!("{id}.app.pkg"));
 fs::write(&tmp,app).map_err(e)?;let mut ap=Pkg::open(&tmp).map_err(e)?;
 extract(&mut ap,&Path::new("/Apps").join(&id))?;let _=fs::remove_file(tmp);println!("installed {id}");Ok(())}
fn run(f:&str,rest:&[String])->R{
 let mut p=sig::verify_stw(f,&trusted_default())?;
 let man=manifest(&mut p)?;let md=roxmltree::Document::parse(&man).map_err(e)?;
 let id=txt(&md,"id").ok_or("no <id>")?;okid(&id)?;let entry=txt(&md,"entry").ok_or("no <entry>")?;
 if !safe(&entry){return Err("unsafe entry".into())}
 let app=p.get("App.pkg").map_err(e)?;let dir=Path::new("/run/stw").join(&id);
 fs::create_dir_all(&dir).map_err(e)?;let tmp=dir.join("App.pkg");fs::write(&tmp,app).map_err(e)?;
 let mut ap=Pkg::open(&tmp).map_err(e)?;extract(&mut ap,&dir)?;
 Err(Command::new(dir.join(entry)).args(rest).exec().to_string())}
fn strata(a:&[String])->R{
 let(m,f)=(a.first().ok_or("usage: stw strata check|dump|labwc FILE")?,a.get(1).ok_or("file?")?);
 let n=parse_strata(&fs::read_to_string(f).map_err(e)?)?;
 match m.as_str(){
  "check"=>println!("ok"),
  "dump"=>dump(&n,0),
  "labwc"=>{println!("<openbox_menu>");for x in &n{menu(x)}println!("</openbox_menu>")}
  _=>return Err("check|dump|labwc".into())}
 Ok(())}
fn dump(v:&[Node],d:usize){for n in v{println!("{d}\t{}\t{}",n.kind,n.args.join("\t"));dump(&n.kids,d+1)}}
fn menu(n:&Node){match n.kind.as_str(){
 "menu"=>{let id=n.args.first().cloned().unwrap_or_default();let lb=n.args.get(1).cloned().unwrap_or(id.clone());
  println!("<menu id=\"{}\" label=\"{}\">",esc(&id),esc(&lb));for k in &n.kids{menu(k)}println!("</menu>")}
 "item"=>{let c=n.act.as_ref().map(|(_,a)|a.iter().map(|x|format!("'{}'",x.replace('\'',""))).collect::<Vec<_>>().join(" ")).unwrap_or_default();
  println!("<item label=\"{}\"><action name=\"Execute\"><command>{}</command></action></item>",esc(n.args.first().map(|s|s.as_str()).unwrap_or("")),esc(&c))}
 "sep"=>println!("<separator/>"),
 _=>{}}}
fn stwc(a:&[String])->R{
 if a.first().map(|s|s.as_str())!=Some("build"){return Err("usage: stwc build [--unsigned] [project-dir] [task]".into())}
 let(mut uns,mut rest)=(false,vec![]);
 for x in &a[1..]{if x=="--unsigned"{uns=true}else{rest.push(x.clone())}}
 let proj=PathBuf::from(rest.first().cloned().unwrap_or_else(||".".into()));
 let mn=fs::read_to_string(proj.join("MN")).map_err(|x|format!("MN: {x}"))?;
 let d=roxmltree::Document::parse(&mn).map_err(e)?;
 let need=|t:&str|txt(&d,t).ok_or(format!("MN: missing <{t}>"));
 let(id,name,ver)=(need("id")?,need("name")?,need("version")?);okid(&id)?;
 let b=parse_brix(&fs::read_to_string(proj.join(need("compiler")?)).map_err(e)?)?;
 let task=rest.get(1).cloned().unwrap_or(b.default.clone());
 run_brix(&proj,&b,&task,&mut HashSet::new())?;
 let rd=|t:&str,def:&str|fs::read(proj.join(txt(&d,t).unwrap_or(def.into()))).map_err(|x|format!("<{t}>: {x}"));
 let(tos,pr)=(rd("terms","Termsofservice.md")?,rd("privacy","PrivacyPolicy.md")?);
 let acc=|a:&str|d.descendants().find(|n|n.tag_name().name()=="accept").and_then(|n|n.attribute(a)).unwrap_or("prompt").to_string();
 let perms:String=all(&d,"permission").iter().map(|x|format!("<permission>{}</permission>",esc(x))).collect();
 let man=format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<software schema=\"1\"><id>{}</id><name>{}</name><version>{}</version><author>{}</author><entry>{}</entry><layer>{}</layer><permissions>{}</permissions></software>\n",
  esc(&id),esc(&name),esc(&ver),esc(&txt(&d,"author").unwrap_or_default()),esc(&need("entry")?),esc(&txt(&d,"layer").unwrap_or("linux".into())),perms);
 let au=format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<automated schema=\"1\"><tos sha256=\"{}\" accept=\"{}\"/><privacy sha256=\"{}\" accept=\"{}\"/></automated>\n",sig::sha256(&tos),acc("tos"),sig::sha256(&pr),acc("privacy"));
 let out=proj.join(need("output")?);
 if let Some(x)=out.parent(){fs::create_dir_all(x).map_err(e)?}
 let mut p=Pkg::create(&out).map_err(e)?;
 for(n,dta)in [("Softwaremanifest.xml",man.into_bytes()),("Termsofservice.md",tos),("PrivacyPolicy.md",pr),("Automated.xml",au.into_bytes())]{p.put(n,&dta,0o644).map_err(e)?}
 p.put("App.pkg",&fs::read(proj.join(need("app")?)).map_err(|x|format!("<app>: {x}"))?,0o644).map_err(e)?;
 for x in all(&d,"extra"){let fp=proj.join(&x);p.put(&format!("extra/{}",fp.file_name().unwrap().to_string_lossy()),&fs::read(&fp).map_err(|y|format!("{x}: {y}"))?,0o644).map_err(e)?}
 if !uns{let k=env::var("STW_KEY").ok().or_else(||txt(&d,"key").map(|k|proj.join(k).to_string_lossy().to_string())).ok_or("no key: set STW_KEY, <key> in MN, or --unsigned")?;
  sig::sign_stw(&mut p,&sig::seed_from(&k)?)?}
 p.commit().map_err(e)?;println!("built {}",out.display());Ok(())}
