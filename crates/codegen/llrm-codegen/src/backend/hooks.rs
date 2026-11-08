//! What the shared backend asks of a target's own backend code.
//!
//! A target whose code sits above this crate (`llrm-x86-backend`) installs
//! itself once; the shared passes reach it here, as LLVM's passes reach a
//! target through `TargetRegistry` and not by name.

use std::sync::OnceLock;

use llrm_lir::registers::RegId;

use crate::backend::selectiondag::SelectionDAG;
use crate::backend::selectiondag::emit_api::{EmitContext, EmitError, EmitState, Emitted};
use crate::backend::selectiondag::schedule::Unit;
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

/// The backend code of one target family.
pub trait BackendHooks: Sync + Send {
    /// `body` as the pin's MIR text (`LLRM_MIR_PRINT`), or why it cannot.
    fn mir_print(
        &self,
        body: &LirBody,
        pins: &IndexMap<u32, RegId>,
    ) -> Result<String, String>;

    /// The LIR blocks for the scheduled `units` of `dag`: one MIR block, its
    /// values and addresses continuing from `state`, the first block made at
    /// `first_at` (the pin's `InstrEmitter`).
    fn emit_units(
        &self,
        dag: &SelectionDAG,
        units: &[Unit],
        ctx: &EmitContext<'_>,
        state: &mut EmitState,
        first_at: i64,
    ) -> Result<Emitted, EmitError>;
}

static INSTALLED: OnceLock<&'static dyn BackendHooks> = OnceLock::new();

/// Installs `hooks`; the first install wins, a repeat is a no-op.
pub fn install(hooks: &'static dyn BackendHooks) {
    let _ = INSTALLED.set(hooks);
}

/// The installed backend code, if any. In this crate's own tests the x86
/// emitter is compiled in from its source in `llrm-x86-backend` (a dev
/// dependency on that crate would be a second copy of this one, with its own
/// hook), so the end-to-end selection tests run without an install.
pub fn installed() -> Option<&'static dyn BackendHooks> {
    #[cfg(test)]
    if INSTALLED.get().is_none() {
        return Some(&in_tests::TESTS);
    }
    INSTALLED.get().copied()
}

#[cfg(test)]
#[path = "../../../../target/llrm-x86-backend/src/emit.rs"]
mod x86_emit;

#[cfg(test)]
mod in_tests {
    use super::*;

    pub struct Tests;
    pub static TESTS: Tests = Tests;

    impl BackendHooks for Tests {
        fn mir_print(
            &self,
            _body: &LirBody,
            _pins: &IndexMap<u32, RegId>,
        ) -> Result<String, String> {
            Err("no MIR printer in this crate's tests".to_owned())
        }

        fn emit_units(
            &self,
            dag: &SelectionDAG,
            units: &[Unit],
            ctx: &EmitContext<'_>,
            state: &mut EmitState,
            first_at: i64,
        ) -> Result<Emitted, EmitError> {
            super::x86_emit::emit_in(dag, units, ctx, state, first_at)
        }
    }
}
