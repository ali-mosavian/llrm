//! x86's backend code above `llrm-codegen`.
//!
//! `install` hands the shared backend this crate's `BackendHooks`; the driver
//! calls it once where it builds its targets.

use llrm_codegen::backend::hooks::{self, BackendHooks};
use llrm_codegen::model::lir::LirBody;
use llrm_codegen::support::hash::IndexMap;
use llrm_lir::registers::RegId;

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
