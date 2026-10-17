//! Shadow of the translated LICM: `shadow_licm FILE.ll` parses a module dump,
//! runs the hoisting decisions on it (the neutral target) and prints one
//! `HOIST function result` line for each instruction it would hoist, then
//! `COUNTS hoisted loads calls`. Nothing is moved.
use std::rc::Rc;

use llrm_mir::passes::PassManager;
use llrm_transforms::licm::{DECIDED, Licm};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&args[1]).expect("a readable dump");
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    if args.iter().any(|arg| arg == "--parse-only") {
        return;
    }
    let mut manager = PassManager::default();
    manager.add(Licm);
    manager.run_module(&mut module, Rc::new(llrm_mir::target::Neutral)).expect("licm decides");
    DECIDED.with(|all| {
        let all = all.borrow();
        for (tag, list) in [("HOIST", &all.hoisted), ("SINK", &all.sunk), ("ERASE", &all.erased)] {
            for one in list {
                let Some(id) = one.function else { continue };
                let global = &module.globals[id.0 as usize];
                let function = global.function().expect("a function");
                let instruction = function.instruction(one.inst);
                let name = instruction
                    .result
                    .and_then(|value| function.value(value).name.clone())
                    .unwrap_or_else(|| instruction.opcode.mnemonic().to_owned());
                println!("{tag} {} {}", global.name.as_deref().unwrap_or("?"), name);
            }
        }
        println!(
            "COUNTS {} {} {} {} {}",
            all.hoisted.len(),
            all.sunk.len(),
            all.erased.len(),
            all.moved_loads,
            all.moved_calls
        );
    });
}
