//! llrm's optimizer over MIR, each pass one of LLVM's: MIR in, MIR out,
//! and no machine named (agents.md, the fifth rule).

pub mod earlycse;
pub mod inline;
pub mod instcombine;
pub mod licm;
pub mod loopreduce;
pub mod mem2reg;
pub mod simplifycfg;

use crate::module::Module;
use crate::passes::{Pass, PassManager};

/// The default pipeline, in order.
const PIPELINE: &[&str] = &["mem2reg", "instcombine", "simplifycfg", "inline", "mem2reg", "instcombine", "simplifycfg", "earlycse", "licm", "earlycse", "loop-reduce", "instcombine", "simplifycfg"];

fn pass(name: &str) -> Result<Pass, String> {
    if name == "inline" {
        return Ok(Pass::Module(Box::new(inline::Inline)));
    }
    Ok(Pass::Function(match name {
        "mem2reg" => Box::new(mem2reg::Mem2Reg),
        "instcombine" => Box::new(instcombine::InstCombine),
        "simplifycfg" => Box::new(simplifycfg::SimplifyCfg),
        "earlycse" => Box::new(earlycse::EarlyCse),
        "licm" => Box::new(licm::Licm),
        "loop-reduce" => Box::new(loopreduce::LoopReduce),
        _ => return Err(format!("no MIR pass {name}")),
    }))
}

/// The module through the pipeline, verified after each pass. As `opt
/// -passes`, `LLRM_MIR_PASSES` names another, comma-separated; with
/// `LLRM_MIR_STAGES` set to a directory, each pass's output goes there as
/// `NN-pass.ll` (agents.md, the fourth rule).
pub fn optimized(module: &mut Module) -> Result<(), String> {
    let chosen = std::env::var("LLRM_MIR_PASSES").ok();
    let names: Vec<&str> = match &chosen {
        Some(list) => list.split(',').map(str::trim).filter(|one| !one.is_empty()).collect(),
        None => PIPELINE.to_vec(),
    };
    optimized_with(module, &names)
}

/// The module through the passes `names`.
pub fn optimized_with(module: &mut Module, names: &[&str]) -> Result<(), String> {
    let mut manager = PassManager { verify_each: true, dump: std::env::var_os("LLRM_MIR_STAGES").map(Into::into), ..Default::default() };
    for name in names {
        manager.passes.push(pass(name)?);
    }
    manager.run(module).map(|_| ())
}
