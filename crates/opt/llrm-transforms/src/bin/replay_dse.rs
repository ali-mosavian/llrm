//! Dump replay for gvn: `replay_dse FILE.ll [--parse-only]` parses a module
//! dump, runs dse alone on it (the neutral target) and prints an FNV hash of
//! the result. Copy into `crates/opt/llrm-transforms/src/bin/` of the revision
//! to measure; the engine's instructions are the run's minus `--parse-only`'s.
use std::rc::Rc;

use llrm_mir::passes::PassManager;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&args[1]).expect("a readable dump");
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let stores =
        |text: &str| text.lines().filter(|l| l.trim_start().starts_with("store ") || l.contains(" = store ")).count();
    let before = stores(&llrm_mir::print::module(&module));
    if !args.iter().any(|arg| arg == "--parse-only") {
        let mut manager = PassManager::default();
        if std::env::var_os("HOIST").is_some() {
            manager.add(llrm_transforms::hoist::Hoist { size: std::env::var_os("SIZE").is_some() });
        } else if std::env::var_os("POC").is_some() {
            manager.add(llrm_transforms::dse_poc::DsePoc);
        } else {
            manager.add(llrm_transforms::dse::Dse);
        }
        manager.run_module(&mut module, Rc::new(llrm_mir::target::Neutral)).expect("dse runs");
    }
    let out = llrm_mir::print::module(&module);
    if std::env::var_os("POC").is_some() && std::env::var_os("COUNTS").is_some() {
        llrm_transforms::dse_poc::COUNTS.with(|c| eprintln!("counts {:?}", c.borrow()));
    }
    if std::env::var_os("HOIST").is_some() {
        eprintln!("hoisted {}", llrm_transforms::hoist::HOISTED.with(|n| n.get()));
    }
    if let Some(path) = std::env::var_os("OUT") {
        std::fs::write(path, &out).unwrap();
    }
    let after = stores(&out);
    eprintln!("stores {before} {after}");
    let hash = out.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3));
    println!("{hash:016x}");
}
