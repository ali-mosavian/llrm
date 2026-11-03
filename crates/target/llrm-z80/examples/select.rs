//! `select [-O<n>] FILE.ll`: clang's IR through the mid-end and the Z80
//! selector; prints each function's machine nodes by block, or why it was
//! refused. A coverage survey, not a driver.

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_z80::dagisel::generated::z80_z80::MACHINE_OPCODES;
use llrm_z80::driver::{Level, Request, selections};

fn main() {
    let mut request = Request::new(Level::O2);
    let mut input = None;
    for argument in std::env::args().skip(1) {
        match argument.strip_prefix("-O") {
            Some(level) => request.level = Level::parse(level).expect("a level"),
            None => input = Some(argument),
        }
    }
    let text = std::fs::read_to_string(input.expect("a file")).expect("readable file");
    // The mid-end recurses along dependency chains: run on a thread with room.
    let worker = std::thread::Builder::new().stack_size(1 << 30).spawn(move || run(&text, &request)).expect("a thread");
    worker.join().expect("no panic");
}

fn run(
    text: &str,
    request: &Request,
) {
    match selections(text, request) {
        Err(why) => println!("FAILED {why}"),
        Ok(functions) => {
            for (name, selected) in functions {
                match selected {
                    Err(why) => println!("{name}: REFUSED {why}"),
                    Ok(selection) => {
                        for (at, block) in selection.blocks.iter().enumerate() {
                            let names: Vec<&str> = block
                                .units
                                .iter()
                                .flat_map(|unit| unit.nodes.iter().rev().copied())
                                .filter_map(|node| match block.dag.nodes()[node as usize].opcode {
                                    ISD::Machine(code) => MACHINE_OPCODES
                                        .iter()
                                        .find(|(v, _)| u32::from(*v) == code)
                                        .map(|(_, name)| *name),
                                    _ => None,
                                })
                                .collect();
                            println!("{name}.{at}: {}", names.join(" "));
                        }
                    }
                }
            }
        }
    }
}
