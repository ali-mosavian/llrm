//! x86's backend code above `llrm-codegen`.
//!
//! `install` hands the shared backend this crate's `BackendHooks`; the driver
//! calls it once where it builds its targets.

use llrm_codegen::backend::hooks::{self, BackendHooks};
use llrm_codegen::backend::selectiondag::SelectionDAG;
use llrm_codegen::backend::selectiondag::emit_api::{EmitContext, EmitError, EmitState, Emitted};
use llrm_codegen::backend::selectiondag::schedule::Unit;
use llrm_codegen::model::lir::LirBody;
use llrm_codegen::support::hash::IndexMap;
use llrm_lir::registers::RegId;

pub mod dag_function;
pub mod emit;
pub mod fastisel;
#[cfg(test)]
mod emit_tests;
pub mod mirprint;

struct X86;

impl BackendHooks for X86 {
    fn mir_print(
        &self,
        body: &LirBody,
        pins: &IndexMap<u32, RegId>,
    ) -> Result<String, String> {
        mirprint::mir(body, pins)
    }

    fn emit_units(
        &self,
        dag: &SelectionDAG,
        units: &[Unit],
        ctx: &EmitContext<'_>,
        state: &mut EmitState,
        first_at: i64,
    ) -> Result<Emitted, EmitError> {
        emit::emit_in(dag, units, ctx, state, first_at)
    }

    fn select_function(
        &self,
        module: &llrm_mir::module::Module,
        name: &str,
        arch: &dyn llrm_target::Target,
        cpu: &llrm_codegen::backend::cpu::Profile,
        libcalls: &[(String, u32, String)],
        contracts: Option<&llrm_codegen::backend::selectiondag::select_api::RuntimeContracts<'_>>,
        pool: &mut llrm_codegen::backend::constpool::Pool,
    ) -> Result<llrm_codegen::backend::isel::Selected, llrm_codegen::backend::isel::Unselected> {
        dag_function::selected_in_pool(module, name, arch, cpu, libcalls, contracts, pool)
    }

    fn take_fast_took(&self) -> bool {
        fastisel::take_took()
    }
}

static X86_HOOKS: X86 = X86;

/// Installs x86's backend code into the shared backend.
pub fn install() {
    hooks::install(&X86_HOOKS);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `LLRM_MIR_PRINT` printed "no backend installed" for every function
    /// when nothing installed x86's printer; `install` makes it reachable.
    #[test]
    fn test_install_makes_the_backend_reachable() {
        install();
        assert!(hooks::installed().is_some());
    }
}
