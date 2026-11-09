//! .pkg: header(64) | blocks... | zstd(index) ; header points at newest index.
//! Edits append blocks + a new index, then atomically rewrite the 64-byte header.
use std::{collections::BTreeMap,fs::{File,OpenOptions},io::{self,Read,Seek,SeekFrom,Write},path::Path};
pub mod sig;
const MAGIC:&[u8;8]=b"STWPKG\0\x01";
pub const BS:usize=1<<20;
const LVL:i32=19;
fn bad(m:&str)->io::Error{io::Error::new(io::ErrorKind::InvalidData,m.to_string())}
#[derive(Clone,Debug,Default)]
pub struct Entry{pub mode:u32,pub size:u64,pub blocks:Vec<u64>}
pub struct Pkg{f:File,pub idx:BTreeMap<String,Entry>}
fn le32(b:&[u8])->u32{u32::from_le_bytes(b.try_into().unwrap())}
fn le64(b:&[u8])->u64{u64::from_le_bytes(b.try_into().unwrap())}
impl Pkg{
 pub fn create(p:&Path)->io::Result<Pkg>{
  let f=OpenOptions::new().read(true).write(true).create(true).truncate(true).open(p)?;
  let mut s=Pkg{f,idx:BTreeMap::new()};
  s.f.write_all(&[0u8;64])?;s.commit()?;Ok(s)}
 pub fn open(p:&Path)->io::Result<Pkg>{
  let mut f=OpenOptions::new().read(true).write(true).open(p).or_else(|_|File::open(p))?;
  let mut h=[0u8;64];f.read_exact(&mut h)?;
  if &h[..8]!=MAGIC{return Err(bad("not a .pkg"))}
  let(off,len,crc)=(le64(&h[16..24]),le64(&h[24..32]) as usize,le32(&h[32..36]));
  f.seek(SeekFrom::Start(off))?;
  let mut c=vec![0u8;len];f.read_exact(&mut c)?;
  if crc32fast::hash(&c)!=crc{return Err(bad("index crc"))}
  Ok(Pkg{f,idx:dec(&zstd::decode_all(&c[..])?)?})}
 pub fn commit(&mut self)->io::Result<()>{
  let c=zstd::encode_all(&enc(&self.idx)[..],LVL)?;
  let off=self.f.seek(SeekFrom::End(0))?;
  self.f.write_all(&c)?;self.f.sync_data()?;
  let mut h=[0u8;64];
  h[..8].copy_from_slice(MAGIC);h[8..10].copy_from_slice(&1u16.to_le_bytes());
  h[12..16].copy_from_slice(&(BS as u32).to_le_bytes());
  h[16..24].copy_from_slice(&off.to_le_bytes());h[24..32].copy_from_slice(&(c.len() as u64).to_le_bytes());
  h[32..36].copy_from_slice(&crc32fast::hash(&c).to_le_bytes());h[36..40].copy_from_slice(&(self.idx.len() as u32).to_le_bytes());
  self.f.seek(SeekFrom::Start(0))?;self.f.write_all(&h)?;self.f.sync_data()}
 fn wblock(&mut self,ch:&[u8])->io::Result<u64>{
  let off=self.f.seek(SeekFrom::End(0))?;
  let c=zstd::bulk::compress(ch,LVL)?;
  let mut hd=Vec::with_capacity(12);
  hd.extend((c.len() as u32).to_le_bytes());hd.extend((ch.len() as u32).to_le_bytes());hd.extend(crc32fast::hash(ch).to_le_bytes());
  self.f.write_all(&hd)?;self.f.write_all(&c)?;Ok(off)}
 fn rblock(&mut self,o:u64)->io::Result<Vec<u8>>{
  self.f.seek(SeekFrom::Start(o))?;
  let mut h=[0u8;12];self.f.read_exact(&mut h)?;
  let mut c=vec![0u8;le32(&h[0..4]) as usize];self.f.read_exact(&mut c)?;
  let r=zstd::bulk::decompress(&c,le32(&h[4..8]) as usize)?;
  if crc32fast::hash(&r)!=le32(&h[8..12]){return Err(bad("block crc"))}Ok(r)}
 pub fn put(&mut self,name:&str,data:&[u8],mode:u32)->io::Result<()>{
  let mut blocks=vec![];
  for ch in data.chunks(BS){blocks.push(self.wblock(ch)?)}
  self.idx.insert(name.into(),Entry{mode,size:data.len() as u64,blocks});Ok(())}
 pub fn get(&mut self,name:&str)->io::Result<Vec<u8>>{
  let e=self.idx.get(name).ok_or_else(||bad("no such file"))?.clone();
  let mut out=Vec::with_capacity(e.size as usize);
  for o in e.blocks{out.extend(self.rblock(o)?)}Ok(out)}
 /// overwrite bytes at offset, rewriting only the touched blocks
 pub fn patch(&mut self,name:&str,at:u64,b:&[u8])->io::Result<()>{
  let e=self.idx.get(name).ok_or_else(||bad("no such file"))?.clone();
  let end=at+b.len() as u64;
  if b.is_empty(){return Ok(())}
  if end>e.size{let mut d=self.get(name)?;d.resize(end as usize,0);d[at as usize..].copy_from_slice(b);return self.put(name,&d,e.mode)}
  let mut blocks=e.blocks.clone();
  for i in (at as usize/BS)..=((end as usize-1)/BS){
   let mut d=self.rblock(blocks[i])?;
   let base=(i*BS) as u64;
   let(s,t)=(at.max(base),end.min(base+d.len() as u64));
   d[(s-base) as usize..(t-base) as usize].copy_from_slice(&b[(s-at) as usize..(t-at) as usize]);
   blocks[i]=self.wblock(&d)?;}
  self.idx.get_mut(name).unwrap().blocks=blocks;Ok(())}
 pub fn remove(&mut self,name:&str)->bool{self.idx.remove(name).is_some()}
}
fn enc(m:&BTreeMap<String,Entry>)->Vec<u8>{
 let mut v=vec![];
 for(n,e)in m{v.extend((n.len() as u16).to_le_bytes());v.extend(n.as_bytes());v.extend(e.mode.to_le_bytes());v.extend(e.size.to_le_bytes());
  v.extend((e.blocks.len() as u32).to_le_bytes());for b in &e.blocks{v.extend(b.to_le_bytes())}}
 v}
fn take<'a>(b:&mut &'a[u8],n:usize)->io::Result<&'a[u8]>{if b.len()<n{return Err(bad("truncated index"))}let(x,y)=b.split_at(n);*b=y;Ok(x)}
fn dec(mut b:&[u8])->io::Result<BTreeMap<String,Entry>>{
 let mut m=BTreeMap::new();
 while !b.is_empty(){
  let l=u16::from_le_bytes(take(&mut b,2)?.try_into().unwrap()) as usize;
  let name=String::from_utf8(take(&mut b,l)?.to_vec()).map_err(|_|bad("utf8"))?;
  let mode=le32(take(&mut b,4)?);let size=le64(take(&mut b,8)?);let nb=le32(take(&mut b,4)?) as usize;
  let mut blocks=Vec::with_capacity(nb);for _ in 0..nb{blocks.push(le64(take(&mut b,8)?))}
  m.insert(name,Entry{mode,size,blocks});}
 Ok(m)}
