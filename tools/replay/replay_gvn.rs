//! Dump replay for gvn: `replay_gvn FILE.ll [--parse-only]` parses a module dump, runs gvn alone on it
//! (the neutral target) and prints an FNV hash of the result. Copy into `crates/opt/llrm-transforms/src/bin/`
//! of the revision to measure; the engine's instructions are the run's minus `--parse-only`'s.
use std::rc::Rc;

use llrm_mir::passes::PassManager;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&args[1]).expect("a readable dump");
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    if !args.iter().any(|arg| arg == "--parse-only") {
        let mut manager = PassManager::default();
        manager.add(llrm_transforms::gvn::Gvn::default());
        manager.run_module(&mut module, Rc::new(llrm_mir::target::Neutral)).expect("gvn runs");
    }
    let out = llrm_mir::print::module(&module);
    let hash = out.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3));
    println!("{hash:016x}");
}
