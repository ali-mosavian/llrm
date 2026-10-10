//! Port of `qbopt/backend/nativeframe.py`.

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;

use crate::model::ir::Loc;
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub floor: i64,
    pub reserve_at: i64,
    pub saved: Vec<RegId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    pub entry: Entry,
    pub releases: BTreeSet<i64>,
    pub registers: Vec<(i64, RegId)>,
    pub outgoing: BTreeSet<(i64, i64, u32)>,
    pub framed: bool,
    pub return_depth: i64,
}

impl Plan {}

#[must_use]
pub fn pins(
    body: &LirBody,
    layout: &Plan,
) -> IndexMap<u32, RegId> {
    let registers: IndexMap<i64, RegId> = layout.registers.iter().copied().collect();
    let mut result = IndexMap::default();
    for one in body.insns() {
        let Some(what) = &one.what else { continue };
        let Some(register) = registers.get(&one.at) else { continue };
        for operand in what.sources.iter().chain(&what.dests) {
            if let Loc::Held(held) = operand {
                result.insert(held.value, *register);
            }
        }
    }
    result
}
