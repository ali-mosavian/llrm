//! `z80c [-O0|-O1|-O2|-Os] FILE.ll`: every function the file defines, as
//! sdasz80 assembly on stdout. A function the backend refuses is named on
//! stderr and left out.

use llrm_z80::compile::{prepare, text_in};
use llrm_z80::driver::{Level, Request};
use llrm_z80::inline_rt::inline_sole_uses;

fn main() {
    let mut level = Level::O2;
    let mut path = None;
    let mut debug = false;
    let mut adb = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--adb" {
            adb = args.next();
            continue;
        }
        if llrm_debug::is_debug_flag(&arg) {
            debug = true;
            continue;
        }
        match arg.strip_prefix("-O") {
            Some(rest) => level = Level::parse(rest).unwrap_or_else(|why| panic!("{why}")),
            None => path = Some(arg),
        }
    }
    let ir = std::fs::read_to_string(path.expect("z80c [-O2] FILE.ll")).expect("reads the file");
    let request = Request { debug, ..Request::new(level) };
    let prepared = prepare(&ir, &request).expect("the file compiles through the mid-end");
    let names: Vec<String> = prepared.names().map(str::to_owned).collect();
    let mut module = String::from("\t.area _CODE\n");
    for (number, name) in names.iter().enumerate() {
        module.push_str(&format!("\t.globl {}\n", llrm_z80::data::symbol(name)));
        match text_in(&ir, &prepared, name, number, &request) {
            Ok(text) => module.push_str(&text),
            Err(why) => eprintln!("REFUSED @{name}: {why}"),
        }
    }
    if let Some(path) = adb {
        std::fs::write(path, llrm_z80::driver::adb(&ir, &request).unwrap_or_default()).expect("writes the .adb");
    }
    print!("{}", inline_sole_uses(&module, llrm_z80::RUNTIME));
    match llrm_z80::data::text(&ir, &request) {
        Ok(data) => print!("{data}"),
        Err(why) => eprintln!("REFUSED data: {why}"),
    }
}
