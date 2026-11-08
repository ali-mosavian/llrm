//! What the shared backend asks of a target's own backend code.
//!
//! A target whose code sits above this crate (`llrm-x86-backend`) installs
//! itself once; the shared passes reach it here, as LLVM's passes reach a
//! target through `TargetRegistry` and not by name.

use std::sync::OnceLock;

use llrm_lir::registers::RegId;

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
}

static INSTALLED: OnceLock<&'static dyn BackendHooks> = OnceLock::new();

/// Installs `hooks`; the first install wins, a repeat is a no-op.
pub fn install(hooks: &'static dyn BackendHooks) {
    let _ = INSTALLED.set(hooks);
}

/// The installed backend code, if any.
pub fn installed() -> Option<&'static dyn BackendHooks> {
    INSTALLED.get().copied()
}
