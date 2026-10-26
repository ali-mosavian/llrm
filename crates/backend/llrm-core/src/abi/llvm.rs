//! The ABI of a MIR module read from LLVM IR: a function links under its
//! name as its calling convention spells it, and a call is the target's
//! convention (`calling.toml`) with memory effects stated conservatively.

use std::collections::BTreeSet;

use llrm_target::calling::Calling;

use crate::abi::runtime::Contract;
use crate::backend::assemble::Abi;
use crate::hir::model::StackCleanup;

/// Calls and symbols of a module no HIR program made: names are spelled by
/// `calling`'s conventions in the object `format` the module is written in.
pub struct LlvmAbi<'a> {
    pub calling: &'a Calling,
    pub format: &'a str,
}

impl Abi for LlvmAbi<'_> {
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

    fn symbol(
        &self,
        name: &str,
        convention: u32,
    ) -> String {
        self.calling
            .by_number(convention)
            .and_then(|found| found.decorated(self.format, name))
            .unwrap_or_else(|| name.to_owned())
    }
}
