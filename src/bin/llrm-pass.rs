//! `llrm-pass [-O1|-O2|-Os|...] PASS FILE`: reads and verifies MIR, runs the one pass PASS once on each function (the pipeline's own `only`
//! selector, at the options of the level, on the neutral target) and writes the module back. What tools/difftest compares with LLVM's
//! counterpart: llrm's answer to that input, not the input of the next step.

use std::process::ExitCode;
use std::rc::Rc;

use llrm_mir::passes::PassManager;
use llrm_transforms::pipeline::{Applied, Fixed};

#[global_allocator]
static ALLOCATOR: llrm_alloc::Slab = llrm_alloc::Slab;

fn main() -> ExitCode {
    let mut arguments: Vec<String> = std::env::args().skip(1).collect();
    let level = match arguments.first().filter(|one| one.starts_with("-O")) {
        Some(flag) => match llrm_core::driver::flags::Level::parse(&flag[2..]) {
            Ok(level) => {
                arguments.remove(0);
                level
            }
            Err(error) => {
                eprintln!("llrm-pass: {error}");
                return ExitCode::from(2);
            }
        },
        None => llrm_core::driver::flags::Level::O2,
    };
    let [pass, path] = arguments.as_slice() else {
        eprintln!("usage: llrm-pass [-O1|-O2|-Os|...] PASS FILE");
        return ExitCode::from(2);
    };
    if !llrm_transforms::pipeline::passes().contains(&pass.as_str()) {
        eprintln!("llrm-pass: no pass named {pass}; the pipeline's: {}", llrm_transforms::pipeline::passes().join(" "));
        return ExitCode::from(2);
    }
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("{path}: {error}");
            return ExitCode::from(2);
        }
    };
    let mut module = match llrm_mir::parse::module(&text) {
        Ok(module) => module,
        Err(error) => {
            eprintln!("{path}:{error}");
            return ExitCode::FAILURE;
        }
    };
    let applied = Applied { options: level.options(), only: Some(pass.clone()), ..Applied::default() };
    let mut manager = PassManager::default();
    manager.add(Fixed::new(&applied));
    if let Err(error) = manager.run_module(&mut module, Rc::new(llrm_mir::target::Neutral)) {
        eprintln!("{path}: {pass}: {error}");
        return ExitCode::FAILURE;
    }
    print!("{}", llrm_mir::print::module(&module));
    ExitCode::SUCCESS
}
