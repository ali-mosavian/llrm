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
    (manager.verify_each, manager.verify_invalidation) = (true, true);
    manager.add(Licm { apply: std::env::var_os("LICM_APPLY").is_some(), speculate: true });
    manager.run_module(&mut module, Rc::new(llrm_mir::target::Neutral)).expect("licm decides");
    if let Some(path) = std::env::var_os("OUT") {
        std::fs::write(path, llrm_mir::print::module(&module)).expect("a writable OUT");
    }
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
        for one in &all.promoted {
            let Some(id) = one.function else { continue };
            let global = &module.globals[id.0 as usize];
            let function = global.function().expect("a function");
            let names: Vec<String> = one
                .pointers
                .iter()
                .map(|pointer| match pointer {
                    llrm_mir::module::Operand::Value(value) => function.value(*value).name.clone().unwrap_or_default(),
                    llrm_mir::module::Operand::Constant(constant) => match module.context.get(*constant).kind {
                        llrm_mir::context::ConstantKind::Global(g) => {
                            module.globals[g.0 as usize].name.owned().unwrap_or_default()
                        }
                        _ => "constant".to_owned(),
                    },
                    llrm_mir::module::Operand::Block(_) => "block".to_owned(),
                })
                .collect();
            println!("PROMOTE {} {} {}", global.name.as_deref().unwrap_or("?"), names.join(","), one.store_safe);
        }
        println!(
            "COUNTS {} {} {} {} {} {} {} {} {} {}",
            all.hoisted.len(),
            all.sunk.len(),
            all.erased.len(),
            all.moved_loads,
            all.moved_calls,
            all.promotion_candidates,
            all.promoted.len(),
            all.rebuilds,
            all.max_rebuilds,
            all.rebuilt_instructions
        );
    });
}
