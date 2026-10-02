//! Loop-closed SSA opened again once the loop passes are done: LLVM's
//! InstSimplify folds a phi of one value, and LCSSA is only a form the loop
//! passes need. Left closed, every exit phi reached isel as a phi of one
//! value, which a spiller treats as a value of its own.

use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::transform;

pub struct Unclose;

impl FunctionPass for Unclose {
    fn name(&self) -> &'static str {
        "unclose"
    }

    fn run(&mut self, unit: &mut Unit, _: &mut Analyses) -> PreservedAnalyses {
        let phis = |unit: &Unit| unit.function.walk().filter(|&(_, one)| unit.function.instruction(one).opcode == Opcode::Phi).count();
        let before = phis(unit);
        transform::_trivial_phis(unit.function).unwrap_or_else(|error| panic!("unclose: {error}"));
        if phis(unit) == before { PreservedAnalyses::all() } else { PreservedAnalyses::none() }
    }
}
