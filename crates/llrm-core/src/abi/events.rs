//! Port of `qbopt/abi/events.py`: the compiler's near-to-far event-poll
//! adapter, recognized before MIR.

use std::collections::BTreeSet;

use iced_x86::Code;

use crate::abi::handlers::registered;
use crate::abi::runtime::{self, Contract};
use crate::frontends::bc::blocks;
use crate::objectfile::module::{self, Module};
use crate::objectfile::omf::{self, Fixup};
use crate::support::hash::IndexMap;

/// Handlers passed by the established TIMER registration sequence.
pub fn handler_entries(found: &Module) -> BTreeSet<i64> {
    registered(found, "B$ONTA", &["pds71", "vbdos"])
}

pub fn contracts(found: &Module) -> IndexMap<i64, Contract> {
    let family = module::family(&found.records);
    let routine = runtime::VARIANTS.get(&("B$EVK1", family.value()));
    let Some(routine) = routine else {
        return IndexMap::default();
    };
    if module::defines(&found.records, found.seg).contains("B$EVK1") {
        return IndexMap::default();
    }
    let Some(start) = blocks::event_stub(found) else {
        return IndexMap::default();
    };
    let fields: Vec<Fixup> =
        omf::fixups(&found.records).into_iter().filter(|fixup| fixup.seg == Some(found.seg)).collect();
    let Ok(mapped) = blocks::code_map(found) else {
        return IndexMap::default();
    };
    let contract = Contract {
        evidence: format!(
            "BC event adapter: CMP EVTFLG/JNE/RET; POP AX/PUSH CS/PUSH AX/\
             JMP FAR B$EVK1 converts the near return address to a far one. \
             No caller arguments; all event effects retained. {}",
            routine.evidence
        ),
        ..routine.clone()
    };
    let mut out = IndexMap::default();
    for block in blocks::partition(found, &mapped) {
        for one in &block.insns {
            if one.insn.code() == Code::Call_rel16
                && one.insn.near_branch_target() == start as u64
                && !fields.iter().any(|fixup| one.at as i64 <= fixup.offset && fixup.offset < one.end() as i64)
            {
                out.insert(one.at as i64, contract.clone());
            }
        }
    }
    out
}
