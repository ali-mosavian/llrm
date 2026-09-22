//! Port of `qbopt/abi/callsite.py`: a language ABI's inputs and cleanup for
//! a call the runtime table does not establish.

use std::collections::BTreeSet;

use iced_x86::{Code, Register};

use crate::abi::runtime::{Contract, Reg};
use crate::frontend::blocks;
use crate::frontend::declen::Insn;
use crate::objectfile::module::Module;
use crate::support::hash::IndexMap;

pub fn caller_cleanup(following: Option<&Insn>) -> bool {
    let Some(following) = following else {
        return false;
    };
    let instruction = &following.insn;
    matches!(instruction.code(), Code::Add_rm16_imm8 | Code::Add_rm16_imm16)
        && instruction.op0_register() == Register::SP
        && 0 < instruction.immediate(1)
        && instruction.immediate(1) < 0x8000
}

pub fn inferred(found: &Module, contracts: &IndexMap<i64, Contract>) -> IndexMap<i64, Contract> {
    let candidates: IndexMap<i64, &Contract> = contracts
        .iter()
        .filter(|(_, rule)| rule.inputs.is_none() && !rule.name.to_uppercase().starts_with("B$"))
        .map(|(&at, rule)| (at, rule))
        .collect();
    if candidates.is_empty() {
        return IndexMap::default();
    }
    let Ok(decoded) = blocks::instructions(found) else {
        return IndexMap::default();
    };
    let by_address: IndexMap<usize, &Insn> = decoded.iter().map(|one| (one.at, one)).collect();
    let inputs: BTreeSet<Reg> = BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di]);
    let mut result = IndexMap::default();
    for (&at, rule) in &candidates {
        let Some(call) = by_address.get(&(at as usize)) else {
            continue;
        };
        let caller = caller_cleanup(by_address.get(&call.end()).copied());
        result.insert(
            at,
            Contract {
                inputs: Some(inputs.clone()),
                cleanup: if caller { Some(0) } else { rule.cleanup },
                evidence: format!(
                    "{}language ABI takes no incoming arithmetic flags; all GP inputs retained; \
                     memory, clobber and control effects unchanged",
                    if caller {
                        "C caller cleanup assumed from adjacent ADD SP; "
                    } else {
                        "Pascal callee cleanup assumed; argument byte count remains unknown; "
                    }
                ),
                ..(*rule).clone()
            },
        );
    }
    result
}

#[cfg(test)]
#[path = "callsite_tests.rs"]
mod tests;
