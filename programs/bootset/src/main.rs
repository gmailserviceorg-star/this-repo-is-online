//! bootset: CLI launcher that writes the hybrid Stratum ISO to a device. (Graphical UI: not implemented.)
use std::{env,fs::{File,OpenOptions},io::{self,Read,Write}};
fn main(){
 let a:Vec<String>=env::args().collect();
 if a.len()!=3{println!("Stratum OS bootset (CLI)\nusage: {} <stratum-os.iso> <target-device>\nwrites the hybrid ISO raw to the device and ERASES it.\nRun with admin/root rights. Windows device example: \\\\.\\PhysicalDrive2",a[0]);return}
 println!("ERASE {}? type YES:",a[2]);let mut l=String::new();io::stdin().read_line(&mut l).unwrap();if l.trim()!="YES"{return}
 let mut s=File::open(&a[1]).expect("cannot open iso");let mut d=OpenOptions::new().write(true).open(&a[2]).expect("cannot open device (admin rights?)");
 let mut buf=vec![0u8;1<<20];let mut tot=0u64;
 loop{let n=s.read(&mut buf).unwrap();if n==0{break}d.write_all(&buf[..n]).unwrap();tot+=n as u64;eprint!("\r{} MiB",tot>>20)}
 d.sync_all().unwrap();println!("\ndone")}
