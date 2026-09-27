//! Port of `qbopt/abi/callsite.py`: a language ABI's inputs and cleanup for
//! a call the runtime table does not establish.

use std::collections::BTreeSet;

use iced_x86::{Code, Register};

use llrm_qbruntime::{Contract, Reg, per_call, worst};

use crate::abi::events;
use crate::frontends::bc::blocks;
use crate::frontends::bc::declen::Insn;
use crate::objectfile::module::{self, Module};
use crate::objectfile::omf::ValueError;
use crate::support::hash::IndexMap;
use crate::support::pyrepr;

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

/// The per-site map for a whole module, from the object itself.
///
/// One place, because the raise and the lowering must be handed the same
/// answer: built twice from different arguments they can differ.
pub fn for_module(
    found: &Module,
    external: Option<&IndexMap<String, Contract>>,
) -> Result<IndexMap<i64, Contract>, ValueError> {
    let family = module::family(&found.records);
    let mut contracts = per_call(&found.calls, family.value(), &module::defines(&found.records, found.seg));
    contracts.extend(events::contracts(found));
    if family.value() == "vbdos" {
        _zero_entry_sites(found, &mut contracts);
        _redim_sites(found, &mut contracts);
    }
    let inferred = inferred(found, &contracts);
    contracts.extend(inferred);
    for (name, routine) in external.into_iter().flatten() {
        if routine.name != *name {
            return Err(ValueError(format!(
                "external contract name mismatch: {} != {}",
                pyrepr::string(name),
                pyrepr::string(&routine.name)
            )));
        }
        for (&at, called) in &found.calls {
            if called == name {
                contracts.insert(at, routine.clone());
            }
        }
    }
    Ok(contracts)
}

/// B$ExitDim removes three header words and two bound words per dimension.
pub fn _redim_sites(found: &Module, contracts: &mut IndexMap<i64, Contract>) {
    if !found.calls.values().any(|name| name == "B$RDIM") {
        return;
    }
    let Ok(mapped) = blocks::code_map(found) else {
        return;
    };
    for block in blocks::partition(found, &mapped) {
        for window in block.insns.windows(3) {
            let [rank, descriptor, call] = window else { unreachable!() };
            if found.calls.get(&(call.at as i64)).map(String::as_str) != Some("B$RDIM")
                || rank.end() != descriptor.at
                || descriptor.end() != call.at
            {
                continue;
            }
            if !matches!(rank.insn.code(), Code::Push_imm16 | Code::Pushw_imm8) {
                continue;
            }
            if !matches!(descriptor.insn.code(), Code::Push_imm16 | Code::Pushw_imm8 | Code::Push_r16 | Code::Push_rm16) {
                continue;
            }
            if found.fixup_at.keys().any(|&field| rank.at as i64 <= field && field < rank.end() as i64) {
                continue;
            }
            let dimensions = (rank.insn.immediate(0) & 255) as i64;
            contracts.insert(
                call.at as i64,
                Contract {
                    inputs: Some(BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])),
                    cleanup: Some(6 + 4 * dimensions),
                    evidence: concat!(
                        "VBDCL10E.LIB erase.asm RDIM tails dynamic.asm DIM_COMMON. ",
                        "ExitDim 010a reads [bp+8], clears CH, doubles twice and adds 6; ",
                        "011d..0129 pops the return address, adds that count to SP and jumps back. ",
                        "Rank is an unrelocated immediate word push immediately before the descriptor ",
                        "and call in one basic block. All GP inputs and unknown effects retained."
                    )
                    .to_owned(),
                    ..worst("B$RDIM")
                },
            );
        }
    }
}

/// VBDOS's zero-BX entry bypasses its unresolved helper call.
pub fn _zero_entry_sites(found: &Module, contracts: &mut IndexMap<i64, Contract>) {
    if !found.calls.values().any(|name| name == "B$ENRA") {
        return;
    }
    let Ok(mapped) = blocks::code_map(found) else {
        return;
    };
    for block in blocks::partition(found, &mapped) {
        for window in block.insns.windows(2) {
            let [previous, call] = window else { unreachable!() };
            if found.calls.get(&(call.at as i64)).map(String::as_str) != Some("B$ENRA") || previous.end() != call.at {
                continue;
            }
            let insn = &previous.insn;
            if insn.code() != Code::Mov_r16_imm16 || insn.op0_register() != Register::BX || insn.immediate16() != 0 {
                continue;
            }
            if found.fixup_at.keys().any(|&field| previous.at as i64 <= field && field < previous.end() as i64) {
                continue;
            }
            contracts.insert(
                call.at as i64,
                Contract {
                    inputs: Some(BTreeSet::from([Reg::Bx, Reg::Cx])),
                    cleanup: Some(0),
                    evidence: concat!(
                        "VBDCL10E.LIB rtenexit.asm B$ENRA 0x17..0x55: CX sizes the frame; ",
                        "BX=0 at 0x4b bypasses the helper at 0x5b. A same-block immediate MOV BX,0 ",
                        "immediately precedes this call. All other effects remain worst-case."
                    )
                    .to_owned(),
                    ..worst("B$ENRA")
                },
            );
        }
    }
}

#[cfg(test)]
#[path = "callsite_tests.rs"]
mod tests;
