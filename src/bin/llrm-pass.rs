//! `llrm-pass [-O1|-O2|-Os|...] PASS FILE`: reads and verifies MIR, runs the
//! one pass PASS once on each function (the pipeline's own `only` selector, at
//! the options of the level, on the neutral target) and writes the module back.
//! What tools/difftest compares with LLVM's counterpart: llrm's answer to that
//! input, not the input of the next step.
//!
//! `llrm-pass [-On] [--mode 16|32] --order PASS@SLOT FILE` runs one slot of
//! LLVM's order (`sroa@1179`) the same way, and says on stderr what stands in
//! it.

use std::process::ExitCode;
use std::rc::Rc;

use llrm_mir::passes::PassManager;
use llrm_transforms::pipeline::{Applied, Fixed};

#[global_allocator]
static ALLOCATOR: llrm_alloc::Slab = llrm_alloc::Slab;

fn main() -> ExitCode {
    let mut arguments: Vec<String> = std::env::args().skip(1).collect();
    if llrm_codegen::support::version::asked(&arguments, "llrm-pass", env!("LLRM_BUILD_SHA"), env!("LLRM_BUILD_DIRTY"), env!("LLRM_BUILD_INCREMENTAL"))
    {
        return ExitCode::SUCCESS;
    }
    let level = match arguments.first().filter(|one| one.starts_with("-O")) {
        Some(flag) => match llrm_codegen::driver::flags::Level::parse(&flag[2..]) {
            Ok(level) => {
                arguments.remove(0);
                level
            }
            Err(error) => {
                eprintln!("llrm-pass: {error}");
                return ExitCode::from(2);
            }
        },
        None => llrm_codegen::driver::flags::Level::O2,
    };
    // `--mode N`: the module gets the datalayout of the target whose
    // `datalayout.toml` says `mode = N`, so pointer-width answers (a memset's
    // length type) are the target's.
    let mut layout = None;
    let mut machine: Rc<dyn llrm_mir::target::Machine> = Rc::new(llrm_mir::target::Neutral);
    let mut described = "neutral".to_owned();
    if arguments.first().is_some_and(|one| one == "--mode") {
        let size = matches!(
            level,
            llrm_codegen::driver::flags::Level::Os | llrm_codegen::driver::flags::Level::Oz
        );
        let target = match llrm_driver::mode_target(arguments.get(1).map_or("", String::as_str), size) {
            Ok(target) => target,
            Err(error) => {
                eprintln!("llrm-pass: {error}");
                return ExitCode::from(2);
            }
        };
        layout = Some(target.datalayout);
        machine = target.machine;
        described = target.described;
        arguments.drain(..2);
    }
    let ordered = arguments.first().is_some_and(|one| one == "--order");
    if ordered {
        arguments.remove(0);
    }
    let [pass, path] = arguments.as_slice() else {
        eprintln!("usage: llrm-pass [-O1|-O2|-Os|...] [--mode N] [--order] PASS FILE");
        return ExitCode::from(2);
    };
    if ordered {
        let probe = Applied { options: level.options(), ..Applied::default() };
        match llrm_transforms::parity::describe(pass) {
            Some((phase, kind, llvm)) if llrm_transforms::pipeline::slot_built(&probe, pass) => {
                eprintln!("llrm-pass: slot {pass} {phase:?} {kind:?} (LLVM: {llvm})");
            }
            _ => {
                eprintln!("llrm-pass: no slot {pass} in the order at this level");
                return ExitCode::from(2);
            }
        }
    } else if !llrm_transforms::pipeline::passes().contains(&pass.as_str()) {
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
    if layout.is_some() {
        module.datalayout = layout;
    }
    eprintln!("llrm-pass: target {described}");
    let applied = Applied { options: level.options(), only: Some(pass.clone()), ..Applied::default() };
    let mut manager = PassManager::default();
    manager.add(Fixed::new(&applied));
    if let Err(error) = manager.run_module(&mut module, machine) {
        eprintln!("{path}: {pass}: {error}");
        return ExitCode::FAILURE;
    }
    print!("{}", llrm_mir::print::module(&module));
    ExitCode::SUCCESS
}
