//! The ABI of a MIR module read from LLVM IR: every function links under its
//! own name, and a call is the target's convention (`calling.toml`) with
//! memory effects stated conservatively.

use std::collections::BTreeSet;

use crate::abi::runtime::Contract;
use crate::backend::assemble::{Abi, Registers};
use crate::hir::model::StackCleanup;

/// Calls and symbols of a module no HIR program made.
pub struct LlvmAbi;

impl Abi for LlvmAbi {
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
