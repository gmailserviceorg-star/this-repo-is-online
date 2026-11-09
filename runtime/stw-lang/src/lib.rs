//! Brix (.compiler), Lash (.int), Strata (ui/config). Shared lexer.
use std::{collections::HashSet,fs,path::{Path,PathBuf},process::Command};
use stw_pkg::Pkg;
#[derive(Clone,Debug,PartialEq)]
pub enum T{Id(String),S(String),N(i64),P(&'static str)}
pub fn lex(src:&str)->Result<Vec<T>,String>{
 let b:Vec<char>=src.chars().collect();let mut i=0;let mut v=vec![];
 while i<b.len(){
  let c=b[i];
  if c.is_whitespace(){i+=1;}
  else if c=='/'&&b.get(i+1)==Some(&'/'){while i<b.len()&&b[i]!='\n'{i+=1;}}
  else if c=='/'&&b.get(i+1)==Some(&'*'){i+=2;while i+1<b.len()&&!(b[i]=='*'&&b[i+1]=='/'){i+=1;}i+=2;}
  else if c=='"'{
   i+=1;let mut s=String::new();
   while i<b.len()&&b[i]!='"'{if b[i]=='\\'&&i+1<b.len(){i+=1;s.push(if b[i]=='n'{'\n'}else{b[i]});}else{s.push(b[i]);}i+=1;}
   if i>=b.len(){return Err("unterminated string".into());}
   i+=1;v.push(T::S(s));}
  else if c=='-'&&b.get(i+1)==Some(&'>'){v.push(T::P("->"));i+=2;}
  else if c.is_ascii_digit(){
   let(radix,st)=if c=='0'&&b.get(i+1)==Some(&'o'){(8u32,i+2)}else{(10u32,i)};
   let mut j=st;while j<b.len()&&b[j].is_digit(radix){j+=1;}
   let s:String=b[st..j].iter().collect();
   v.push(T::N(i64::from_str_radix(&s,radix).map_err(|e|e.to_string())?));i=j;}
  else if c.is_alphabetic()||c=='_'||c=='.'{
   let mut j=i;while j<b.len()&&(b[j].is_alphanumeric()||b[j]=='_'||b[j]=='.'||b[j]=='-'){j+=1;}
   v.push(T::Id(b[i..j].iter().collect()));i=j;}
  else{v.push(T::P(match c{'{'=>"{",'}'=>"}",'('=>"(",')'=>")",';'=>";",':'=>":",','=>",",'='=>"=",_=>return Err(format!("bad char '{c}'"))}));i+=1;}
 }
 Ok(v)}
pub struct C{t:Vec<T>,i:usize}
impl C{
 fn new(s:&str)->Result<C,String>{Ok(C{t:lex(s)?,i:0})}
 fn eof(&self)->bool{self.i>=self.t.len()}
 fn pk(&self)->Option<&T>{self.t.get(self.i)}
 fn pk2(&self)->Option<&T>{self.t.get(self.i+1)}
 fn nx(&mut self)->Result<T,String>{let t=self.t.get(self.i).cloned().ok_or("unexpected end of file")?;self.i+=1;Ok(t)}
 fn is(&self,s:&str)->bool{matches!(self.pk(),Some(T::P(x)) if *x==s)}
 fn p(&mut self,s:&'static str)->Result<(),String>{match self.nx()?{T::P(x) if x==s=>Ok(()),t=>Err(format!("expected '{s}', got {t:?}"))}}
 fn id(&mut self)->Result<String,String>{match self.nx()?{T::Id(s)=>Ok(s),t=>Err(format!("expected identifier, got {t:?}"))}}
 fn kw(&mut self,k:&str)->Result<(),String>{let s=self.id()?;if s==k{Ok(())}else{Err(format!("expected '{k}', got '{s}'"))}}
 fn s(&mut self)->Result<String,String>{match self.nx()?{T::S(s)=>Ok(s),t=>Err(format!("expected string, got {t:?}"))}}
 fn n(&mut self)->Result<i64,String>{match self.nx()?{T::N(n)=>Ok(n),t=>Err(format!("expected number, got {t:?}"))}}
}
// ---------------- Lash ----------------
#[derive(Debug,Default)]
pub struct Lash{pub name:String,pub ver:String,pub entry:String,pub excl:Vec<String>,pub rules:Vec<(u32,String,String)>}
pub fn parse_lash(src:&str)->Result<Lash,String>{
 let mut c=C::new(src)?;let mut l=Lash::default();
 while !c.eof(){
  let k=c.id()?;
  match k.as_str(){
   "PACKAGE"=>{c.p("(")?;l.name=c.s()?;c.p(",")?;l.ver=c.s()?;c.p(")")?;c.p(";")?;}
   "ENTRY"=>{c.p("(")?;l.entry=c.s()?;c.p(")")?;c.p(";")?;}
   "EXCLUDE"=>{c.p("(")?;let x=c.s()?;l.excl.push(x);c.p(")")?;c.p(";")?;}
   "SECTIONS"=>{c.p("{")?;
    while !c.is("}"){
     let _sec=c.id()?;
     let has=matches!(c.pk(),Some(T::N(_)));
     let mode=if has{c.n()? as u32}else{0o644};
     c.p(":")?;c.p("{")?;
     while !c.is("}"){let f=c.s()?;c.p("->")?;let t=c.s()?;c.p(";")?;l.rules.push((mode,f,t));}
     c.p("}")?;}
    c.p("}")?;}
   _=>return Err(format!("unknown statement '{k}'"))}}
 if l.name.is_empty(){return Err("missing PACKAGE(...)".into())}
 Ok(l)}
fn wm(p:&[u8],s:&[u8])->bool{match p.first(){None=>s.is_empty(),Some(b'*')=>(0..=s.len()).any(|i|wm(&p[1..],&s[i..])),Some(c)=>s.first()==Some(c)&&wm(&p[1..],&s[1..])}}
fn walk(root:&Path,rel:&Path,out:&mut Vec<PathBuf>){
 if let Ok(rd)=fs::read_dir(root.join(rel)){for e in rd.flatten(){let r=rel.join(e.file_name());if e.path().is_dir(){walk(root,&r,out)}else{out.push(r)}}}}
/// .int files are only accepted at <project>/compileinstruction/<name>.int
pub fn check_int(proj:&Path,int:&Path)->Result<(),String>{
 let want=fs::canonicalize(proj.join("compileinstruction")).map_err(|_|"missing compileinstruction/ directory".to_string())?;
 let got=fs::canonicalize(int).map_err(|e|format!("{}: {e}",int.display()))?;
 if got.parent()!=Some(want.as_path())||got.extension().map_or(true,|e|e!="int"){return Err(format!(".int must be at {}/<name>.int",want.display()))}
 Ok(())}
pub fn build_lash(proj:&Path,int:&Path,out:&Path)->Result<(),String>{
 check_int(proj,int)?;
 let l=parse_lash(&fs::read_to_string(int).map_err(|e|e.to_string())?)?;
 if let Some(d)=out.parent(){fs::create_dir_all(d).map_err(|e|e.to_string())?;}
 let mut p=Pkg::create(out).map_err(|e|e.to_string())?;
 let mut seen=HashSet::new();
 for(mode,from,to)in &l.rules{
  let mut list:Vec<(PathBuf,String)>=vec![];
  let multi=from.contains('*')||proj.join(from).is_dir();
  if from.contains('*'){
   let fp=Path::new(from);let dir=fp.parent().unwrap_or(Path::new(""));
   let pat=fp.file_name().and_then(|s|s.to_str()).unwrap_or("*");
   if let Ok(rd)=fs::read_dir(proj.join(dir)){for e in rd.flatten(){let n=e.file_name().to_string_lossy().to_string();if wm(pat.as_bytes(),n.as_bytes())&&e.path().is_file(){list.push((dir.join(&n),n))}}}
  }else if proj.join(from).is_dir(){
   let mut v=vec![];walk(&proj.join(from),Path::new(""),&mut v);
   for r in v{list.push((Path::new(from).join(&r),r.to_string_lossy().to_string()))}
  }else{list.push((PathBuf::from(from),Path::new(from).file_name().unwrap().to_string_lossy().to_string()))}
  for(src,name)in list{
   let dest=if to.ends_with('/'){format!("{to}{name}")}else if multi{format!("{to}/{name}")}else{to.clone()};
   if l.excl.iter().any(|x|wm(x.as_bytes(),dest.as_bytes())){continue}
   let data=fs::read(proj.join(&src)).map_err(|e|format!("{}: {e}",src.display()))?;
   if !seen.insert(dest.clone()){return Err(format!("duplicate {dest}"))}
   p.put(&dest,&data,*mode).map_err(|e|e.to_string())?;}}
 if !l.entry.is_empty()&&!seen.contains(&l.entry){return Err(format!("ENTRY {} not in package",l.entry))}
 p.put("META/lash",format!("{} {}\n{}\n",l.name,l.ver,l.entry).as_bytes(),0o644).map_err(|e|e.to_string())?;
 p.commit().map_err(|e|e.to_string())}
// ---------------- Brix ----------------
#[derive(Debug)] pub enum Step{Need(String),Run(String),Echo(String),Lash(String,String)}
#[derive(Debug,Default)] pub struct Brix{pub vars:Vec<(String,String)>,pub tasks:Vec<(String,Vec<Step>)>,pub default:String}
pub fn parse_brix(src:&str)->Result<Brix,String>{
 let mut c=C::new(src)?;let mut b=Brix::default();
 c.kw("brix")?;if c.n()?!=1{return Err("unsupported brix version".into())}c.p(";")?;
 while !c.eof(){
  let k=c.id()?;
  match k.as_str(){
   "var"=>{let n=c.id()?;c.p("=")?;let v=c.s()?;c.p(";")?;b.vars.push((n,v));}
   "default"=>{b.default=c.id()?;c.p(";")?;}
   "task"=>{let n=c.id()?;c.p("{")?;let mut st=vec![];
    while !c.is("}"){let s=c.id()?;match s.as_str(){
     "need"=>{st.push(Step::Need(c.id()?));c.p(";")?}
     "run"=>{st.push(Step::Run(c.s()?));c.p(";")?}
     "echo"=>{st.push(Step::Echo(c.s()?));c.p(";")?}
     "lash"=>{let i=c.s()?;c.p("->")?;let o=c.s()?;c.p(";")?;st.push(Step::Lash(i,o))}
     _=>return Err(format!("unknown step '{s}'"))}}
    c.p("}")?;b.tasks.push((n,st));}
   _=>return Err(format!("unknown declaration '{k}'"))}}
 if b.default.is_empty(){return Err("missing 'default <task>;'".into())}
 Ok(b)}
fn interp(s:&str,v:&[(String,String)])->String{
 let cs:Vec<char>=s.chars().collect();let mut o=String::new();let mut i=0;
 while i<cs.len(){
  if cs[i]=='$'&&i+1<cs.len()&&(cs[i+1].is_alphabetic()||cs[i+1]=='_'){
   let mut j=i+1;while j<cs.len()&&(cs[j].is_alphanumeric()||cs[j]=='_'){j+=1}
   let n:String=cs[i+1..j].iter().collect();
   let val=v.iter().rev().find(|x|x.0==n).map(|x|x.1.clone()).or_else(||std::env::var(&n).ok()).unwrap_or_default();
   o+=&val;i=j;
  }else{o.push(cs[i]);i+=1}}
 o}
pub fn run_brix(proj:&Path,b:&Brix,task:&str,done:&mut HashSet<String>)->Result<(),String>{
 if !done.insert(task.to_string()){return Ok(())}
 let steps=&b.tasks.iter().find(|t|t.0==task).ok_or(format!("no task '{task}'"))?.1;
 for s in steps{match s{
  Step::Need(n)=>run_brix(proj,b,n,done)?,
  Step::Echo(m)=>println!("{}",interp(m,&b.vars)),
  Step::Run(c)=>{let c=interp(c,&b.vars);println!("+ {c}");
   let st=Command::new("sh").arg("-c").arg(&c).current_dir(proj).status().map_err(|e|e.to_string())?;
   if !st.success(){return Err(format!("failed: {c}"))}}
  Step::Lash(i,o)=>{let(i,o)=(interp(i,&b.vars),interp(o,&b.vars));build_lash(proj,&proj.join(&i),&proj.join(&o))?;println!("lash -> {o}")}}}
 Ok(())}
// ---------------- Strata ----------------
#[derive(Debug,Default)]
pub struct Node{pub kind:String,pub args:Vec<String>,pub act:Option<(String,Vec<String>)>,pub props:Vec<(String,String)>,pub kids:Vec<Node>}
fn arg(c:&mut C)->Result<String,String>{match c.nx()?{T::S(s)|T::Id(s)=>Ok(s),T::N(n)=>Ok(n.to_string()),t=>Err(format!("bad argument {t:?}"))}}
fn node(c:&mut C)->Result<Node,String>{
 let mut n=Node{kind:c.id()?,..Default::default()};
 while matches!(c.pk(),Some(T::Id(_)|T::S(_)|T::N(_))){n.args.push(arg(c)?)}
 if c.is("->"){c.p("->")?;let a=c.id()?;let mut av=vec![];
  if c.is("("){c.p("(")?;while !c.is(")"){av.push(arg(c)?);if c.is(","){c.p(",")?}}c.p(")")?}
  n.act=Some((a,av))}
 if c.is("{"){c.p("{")?;
  while !c.is("}"){
   if matches!((c.pk(),c.pk2()),(Some(T::Id(_)),Some(T::P(":")))){let k=c.id()?;c.p(":")?;let v=arg(c)?;c.p(";")?;n.props.push((k,v))}
   else{n.kids.push(node(c)?)}}
  c.p("}")?;
 }else{c.p(";")?}
 Ok(n)}
pub fn parse_strata(src:&str)->Result<Vec<Node>,String>{let mut c=C::new(src)?;let mut v=vec![];while !c.eof(){v.push(node(&mut c)?)}Ok(v)}
