//! `z80c [-O0|-O1|-O2|-Os] FILE.ll`: every function the file defines, as
//! sdasz80 assembly on stdout. A function the backend refuses is named on
//! stderr and left out.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request, selections};

fn main() {
    let mut level = Level::O2;
    let mut path = None;
    for arg in std::env::args().skip(1) {
        match arg.strip_prefix("-O") {
            Some(rest) => level = Level::parse(rest).unwrap_or_else(|why| panic!("{why}")),
            None => path = Some(arg),
        }
    }
    let ir = std::fs::read_to_string(path.expect("z80c [-O2] FILE.ll")).expect("reads the file");
    let request = Request::new(level);
    let names: Vec<String> = selections(&ir, &request)
        .expect("the file compiles through the mid-end")
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    println!("\t.area _CODE");
    for (number, name) in names.iter().enumerate() {
        println!("\t.globl _{name}");
        match text(&ir, name, number, &request) {
            Ok(text) => print!("{text}"),
            Err(why) => eprintln!("REFUSED @{name}: {why}"),
        }
    }
}
