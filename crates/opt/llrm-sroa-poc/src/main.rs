//! `sroa-poc MODE FILE...`: runs a variant of SROA + mem2reg on the body
//! `pipeline.body` of each MIR file and prints its opcode histogram and what
//! the run cost. MODE: `none` (the input), `ours` (Sroa, Promote, Dse, Dead as
//! the pipeline has them).

use std::collections::BTreeMap;
use std::rc::Rc;

use llrm_mir::module::Module;
use llrm_mir::passes::PassManager;

fn histogram(module: &Module) -> BTreeMap<&'static str, usize> {
    let mut found = BTreeMap::new();
    for (_, global, function) in module.functions() {
        if global.name.as_deref() != Some("pipeline.body") {
            continue;
        }
        for (_, inst) in function.walk() {
            *found.entry(function.instruction(inst).opcode.mnemonic()).or_default() += 1;
        }
    }
    found
}

fn sroa_promote(module: &mut Module) {
    let mut manager = PassManager::default();
    manager.add(llrm_transforms::promote::Sroa);
    manager.add(llrm_transforms::promote::Promote);
    manager.run_module(module, Rc::new(llrm_mir::target::Neutral)).expect("the pipeline runs");
}

fn ours(module: &mut Module) {
    let mut manager = PassManager::default();
    manager.add(llrm_transforms::promote::Sroa);
    manager.add(llrm_transforms::promote::Promote);
    manager.add(llrm_transforms::dse::Dse);
    manager.add(llrm_transforms::dead::Dead);
    manager.run_module(module, Rc::new(llrm_mir::target::Neutral)).expect("the pipeline runs");
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    let mode = arguments.next().expect("a mode");
    let mut total: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut work = 0;
    for path in arguments {
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
        let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{path}: {error}"));
        let before = llrm_support::debug::work();
        match mode.as_str() {
            "none" => {}
            "ours" => ours(&mut module),
            "sroa-promote" => sroa_promote(&mut module),
            other => panic!("mode {other}"),
        }
        work += llrm_support::debug::work() - before;
        for (name, count) in histogram(&module) {
            *total.entry(name).or_default() += count;
        }
    }
    let size: usize = total.values().sum();
    println!("{mode}: {size} instructions, {work} work");
    println!("{}", total.iter().map(|(name, count)| format!("{name} {count}")).collect::<Vec<_>>().join(", "));
}
