//! The ABI of a MIR module read from LLVM IR: every function links under its
//! own name, and a call is the target's convention (`calling.toml`) with
//! memory effects stated conservatively.

use std::collections::BTreeSet;

use crate::abi::runtime::Contract;
use crate::backend::assemble::{Abi, Registers};
use crate::hir::model::StackCleanup;

/// Calls and symbols of a module no HIR program made.
pub struct LlvmAbi {
    /// Where a function marked `StackCheck` compares SP and what it calls on
    /// overflow (`-fsanitize=stack`).
    pub stack_check: Option<crate::hir::model::StackCheck>,
}

/// The check the target's C runtime states (`stack.toml`): the OS layer's stack
/// limit and the routine a checked function enters on overflow.
pub fn stack_check(target: &dyn llrm_target::Target) -> crate::hir::model::StackCheck {
    let description = target.runtime("c").expect("a C target has a C runtime");
    let name = description.string("stack").expect("c.toml names its stack check");
    let text = description.file(&name).expect("the stack check is shipped");
    let row = toml::Value::Table(text.parse().expect("stack.toml parses"));
    crate::hir::model::StackCheck::from_toml(&row).expect("stack.toml states a stack check")
}

impl Abi for LlvmAbi {
    fn stack_check(&self) -> Option<&crate::hir::model::StackCheck> {
        self.stack_check.as_ref()
    }

    fn contract(
        &self,
        callee: &str,
        pops: bool,
        pushed: i64,
    ) -> Result<Contract, String> {
        // Inline code that declares its registers (`#pragma aux`) is its own
        // contract.
        if let Some(block) = super::qb::asm_call(callee) {
            return block.map(|(contract, _)| contract).map_err(|error| error.0);
        }
        let cleanup = if pops { StackCleanup::Callee } else { StackCleanup::Caller };
        Ok(super::qb::ordinary(callee, cleanup, pushed, &BTreeSet::new()))
    }

    fn registers(
        &self,
        callee: &str,
    ) -> Option<Registers> {
        super::qb::asm_call(callee)?.ok().map(|(_, registers)| registers)
    }

    fn linked(
        &self,
        name: &str,
    ) -> String {
        name.to_owned()
    }
}
