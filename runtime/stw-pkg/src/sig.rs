use crate::Pkg;
use ed25519_dalek::{Signature,Signer,SigningKey,Verifier,VerifyingKey};
use sha2::{Digest,Sha256};
use std::{fs::{self,File},io::{self,Read},path::Path};
pub fn hex(b:&[u8])->String{b.iter().map(|x|format!("{x:02x}")).collect()}
pub fn unhex(s:&str)->Option<Vec<u8>>{let s=s.trim();if s.len()%2!=0{return None}(0..s.len()).step_by(2).map(|i|u8::from_str_radix(s.get(i..i+2)?,16).ok()).collect()}
pub fn sha256(b:&[u8])->String{hex(&Sha256::digest(b))}
pub fn keygen()->io::Result<[u8;32]>{let mut b=[0u8;32];File::open("/dev/urandom")?.read_exact(&mut b)?;Ok(b)}
pub fn seed_from(path:&str)->Result<[u8;32],String>{
 let v=unhex(&fs::read_to_string(path).map_err(|e|format!("{path}: {e}"))?).ok_or("bad key file")?;
 <[u8;32]>::try_from(v.as_slice()).map_err(|_|"key must be 32 bytes".to_string())}
pub fn pubkey(seed:&[u8;32])->[u8;32]{SigningKey::from_bytes(seed).verifying_key().to_bytes()}
pub fn sign(seed:&[u8;32],msg:&[u8])->String{hex(&SigningKey::from_bytes(seed).sign(msg).to_bytes())}
pub fn verify(pubs:&[Vec<u8>],msg:&[u8],sig_hex:&str)->bool{
 let Some(sb)=unhex(sig_hex) else{return false};
 let Ok(sg)=Signature::from_slice(&sb) else{return false};
 pubs.iter().any(|k|{let Ok(a)=<[u8;32]>::try_from(k.as_slice()) else{return false};
  VerifyingKey::from_bytes(&a).map(|v|v.verify(msg,&sg).is_ok()).unwrap_or(false)})}
pub fn trusted(path:&str)->Vec<Vec<u8>>{
 fs::read_to_string(path).unwrap_or_default().lines().filter(|l|!l.trim().starts_with('#')).filter_map(unhex).filter(|v|v.len()==32).collect()}
pub fn digest(p:&mut Pkg)->Result<Vec<u8>,String>{
 let names:Vec<String>=p.idx.keys().filter(|k|k.as_str()!="META/SIGNATURE").cloned().collect();
 let mut o=String::new();
 for n in names{let d=p.get(&n).map_err(|e|e.to_string())?;o+=&format!("{n}\t{}\n",sha256(&d));}
 Ok(o.into_bytes())}
pub fn sign_stw(p:&mut Pkg,seed:&[u8;32])->Result<(),String>{
 let d=digest(p)?;
 let s=format!("stwsig1\n{}\n{}\n",hex(&pubkey(seed)),sign(seed,&d));
 p.put("META/SIGNATURE",s.as_bytes(),0o644).map_err(|e|e.to_string())}
pub fn verify_stw(path:&str,pubs:&[Vec<u8>])->Result<Pkg,String>{
 let mut p=Pkg::open(Path::new(path)).map_err(|e|format!("{path}: {e}"))?;
 let s=String::from_utf8(p.get("META/SIGNATURE").map_err(|_|"unsigned package".to_string())?).map_err(|e|e.to_string())?;
 let l:Vec<&str>=s.lines().collect();
 if l.len()<3||l[0]!="stwsig1"{return Err("bad signature file".into())}
 let pk=unhex(l[1]).ok_or("bad key")?;
 if !pubs.iter().any(|x|*x==pk){return Err("signer not trusted".into())}
 let d=digest(&mut p)?;
 if !verify(&[pk],&d,l[2]){return Err("signature invalid".into())}
 Ok(p)}
