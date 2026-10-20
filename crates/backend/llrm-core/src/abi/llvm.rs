//! The ABI of a MIR module read from LLVM IR: every function links under its
//! own name, and a call is the target's convention (`calling.toml`) with
//! memory effects stated conservatively.

use std::collections::BTreeSet;

use crate::abi::runtime::Contract;
use crate::backend::assemble::Abi;
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
        let cleanup = if pops { StackCleanup::Callee } else { StackCleanup::Caller };
        Ok(super::qb::ordinary(callee, cleanup, pushed, &BTreeSet::new()))
    }

    fn linked(
        &self,
        name: &str,
    ) -> String {
        name.to_owned()
    }
}
