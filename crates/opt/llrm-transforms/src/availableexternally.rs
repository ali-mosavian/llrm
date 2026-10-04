//! LLVM's EliminateAvailableExternally: a body the program holds only to
//! inline from is a declaration again once inlining is done, so no copy of
//! it is emitted and what calls it still reaches the runtime's own routine.

use llrm_mir::module::{GlobalKind, Linkage};
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};

pub struct EliminateAvailableExternally;

impl ProgramPass for EliminateAvailableExternally {
    fn name(&self) -> &'static str {
        "available-externally"
    }

    fn run(&mut self, program: &mut Program, analyses: &mut ProgramAnalyses) -> Result<(), String> {
        let mut changed = false;
        for module in &mut program.modules {
            for global in &mut module.globals {
                if global.linkage == Linkage::AvailableExternally && matches!(&global.kind, GlobalKind::Function(function) if !function.is_declaration()) {
                    *global = global.declaration();
                    global.linkage = Linkage::External;
                    changed = true;
                }
            }
        }
        if changed {
            analyses.invalidate();
        }
        Ok(())
    }
}
