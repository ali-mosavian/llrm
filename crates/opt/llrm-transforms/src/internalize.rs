//! Make every definition the program does not export local to it.
//!
//! # How it works
//! The translation of LLVM's `InternalizePass`
//! ([Internalize.cpp#L132](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/Internalize.cpp#L132)):
//! a definition with external linkage that is not preserved becomes internal
//! with default visibility. Preserved are declarations, available-externally
//! bodies, globals already local, the `llvm.` metadata arrays, the entry
//! points and the globals the program keeps (`exports`), and the names given
//! to the pass. [`crate::globaldce`] then deletes what no live code names,
//! which is what a linker's `--gc-sections` does for a whole program.
//!
//! # Example
//! With `main` preserved, `@scale` and `@table` become internal and the next
//! GlobalDCE deletes them if nothing reaches them:
//!
//! ```mir
//! @table = global [2 x i16] [i16 1, i16 2]
//! define i16 @scale(i16 %x) { ... }
//! define i16 @main() { ... }
//! ```

use std::collections::BTreeSet;

use llrm_mir::module::{GlobalKind, GlobalValue, Linkage, Visibility};
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};

/// The globals the pin never internalizes besides the declared API
/// (`InternalizePass::internalizeModule`).
const ALWAYS_PRESERVED: &[&str] = &["__stack_chk_fail", "__stack_chk_guard"];

pub struct Internalize {
    preserve: BTreeSet<String>,
}

impl Internalize {
    /// The pass keeping `preserve` external.
    pub fn new(preserve: impl IntoIterator<Item = String>) -> Self {
        Self { preserve: preserve.into_iter().collect() }
    }
}

impl ProgramPass for Internalize {
    fn name(&self) -> &'static str {
        "internalize"
    }

    fn run(
        &mut self,
        program: &mut Program,
        analyses: &mut ProgramAnalyses,
    ) -> Result<(), String> {
        let mut changed = false;
        let exports = program.exports.clone();
        for module in &mut program.modules {
            for global in &mut module.globals {
                if preserved(global, &self.preserve, &exports) {
                    continue;
                }
                global.visibility = Visibility::Default;
                global.linkage = Linkage::Internal;
                changed = true;
            }
        }
        if changed {
            analyses.invalidate();
        }
        Ok(())
    }
}

/// `InternalizePass::shouldPreserveGV` and the always-preserved names.
fn preserved(
    global: &GlobalValue,
    preserve: &BTreeSet<String>,
    exports: &llrm_mir::program::Exports,
) -> bool {
    let defined = match &global.kind {
        GlobalKind::Function(function) => !function.is_declaration(),
        GlobalKind::Variable(variable) => variable.initializer.is_some(),
    };
    if !defined || matches!(global.linkage, Linkage::Internal | Linkage::Private | Linkage::AvailableExternally) {
        return true;
    }
    if global.linkage == Linkage::Appending {
        return true;
    }
    let Some(name) = global.name.as_ref() else { return true };
    name.starts_with("llvm.")
        || ALWAYS_PRESERVED.contains(&name.as_str())
        || preserve.contains(name)
        || exports.entries.contains(name)
        || exports.kept.contains(name)
}
