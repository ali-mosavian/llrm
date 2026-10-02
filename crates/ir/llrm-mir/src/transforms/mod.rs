//! llrm's optimizer over MIR, each pass one of LLVM's: MIR in, MIR out,
//! and no machine named (agents.md, the fifth rule).

pub mod inline;

use crate::module::Module;
use crate::passes::{Pass, PassManager};

/// The default pipeline, in order.
const PIPELINE: &[&str] = &["inline"];

fn pass(name: &str) -> Result<Pass, String> {
    match name {
        "inline" => Ok(Pass::Module(Box::new(inline::Inline))),
        _ => Err(format!("no MIR pass {name}")),
    }
}

/// The module through the pipeline, verified after each pass. As `opt
/// -passes`, `LLRM_MIR_PASSES` names another, comma-separated; with
/// `LLRM_MIR_STAGES` set to a directory, each pass's output goes there as
/// `NN-pass.ll` (agents.md, the fourth rule); `LLRM_MIR_BISECT` limits
/// how many pass runs happen, as LLVM's `-opt-bisect-limit`.
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
    let bisect = std::env::var("LLRM_MIR_BISECT").ok().map(|limit| limit.parse().map_err(|_| format!("LLRM_MIR_BISECT={limit} is no count"))).transpose()?;
    let mut manager = PassManager { verify_each: true, dump: std::env::var_os("LLRM_MIR_STAGES").map(Into::into), bisect, ..Default::default() };
    for name in names {
        manager.passes.push(pass(name)?);
    }
    manager.run_module(module, std::rc::Rc::new(crate::target::Neutral)).map(|_| ())
}
