//! llrm's optimizer over MIR, each pass one of LLVM's: MIR in, MIR out,
//! and no machine named (agents.md, the fifth rule).

pub mod mem2reg;

use crate::module::Module;
use crate::passes::PassManager;

/// The module through the pipeline, verified after each pass. With
/// `LLRM_MIR_STAGES` set to a directory, each pass's output goes there as
/// `NN-pass.ll` (agents.md, the fourth rule).
pub fn optimized(module: &mut Module) -> Result<(), String> {
    let mut manager = PassManager { verify_each: true, dump: std::env::var_os("LLRM_MIR_STAGES").map(Into::into), ..Default::default() };
    manager.add(mem2reg::Mem2Reg);
    manager.run(module).map(|_| ())
}
