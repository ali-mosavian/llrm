//! Port of `qbopt/backend/verify.py`: whether a lowered body is well formed,
//! checked where it was made. LLVM's `MachineVerifier`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::target;
use crate::model::ir::{self, Loc, Operation};
use crate::model::lir::{LirBlock, LirBody};

/// A machine phase produced LIR that violates an established invariant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Malformed(pub String);

impl fmt::Display for Malformed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Malformed {}

/// Everything wrong with this body, as sentences. Empty is well formed.
pub fn verify(body: &LirBody, in_ssa: bool) -> Vec<String> {
    let mut out = _blocks(body);
    out.extend(_spans(body));
    out.extend(_operands(body));
    out.extend(_values(body, in_ssa));
    out
}

/// The entry and successors exist; unreachable work is rejected.
fn _blocks(body: &LirBody) -> Vec<String> {
    let mut out = vec![];
    let at_of: BTreeMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    if !at_of.contains_key(&body.entry) {
        out.push(format!("the entry {:#06x} is not one of this body's blocks", body.entry));
        return out;
    }
    for block in &body.blocks {
        for r#where in &block.succ {
            if !at_of.contains_key(r#where) {
                out.push(format!("block {:#06x} goes to {:#06x}, which is not in this body", block.at, r#where));
            }
        }
    }
    let (mut seen, mut todo) = (BTreeSet::new(), vec![body.entry]);
    while let Some(at) = todo.pop() {
        if seen.contains(&at) || !at_of.contains_key(&at) {
            continue;
        }
        seen.insert(at);
        todo.extend(at_of[&at].succ.iter().copied());
    }
    for block in &body.blocks {
        if !seen.contains(&block.at) && !_ownership_only(block) {
            out.push(format!("block {:#06x} is not reachable from the entry", block.at));
        }
    }
    out
}

/// Whether an unreachable block exists only to retain original bytes.
fn _ownership_only(block: &LirBlock) -> bool {
    if !block.succ.is_empty() || !block.phis.is_empty() || block.insns.is_empty() {
        return false;
    }
    block.insns.iter().all(|one| {
        one.what.as_ref().is_some_and(|what| {
            what.op == Operation::Nothing
                && what.name.as_deref().is_none_or(str::is_empty)
                && what.dests.is_empty()
                && what.sources.is_empty()
                && what.target.is_none()
        }) && one.defines.is_empty()
            && one.uses.is_empty()
            && one.clobbers.is_empty()
            && one.clobbers_high.is_empty()
            && one.group.is_none()
            && one.requires.is_empty()
            && one.delivers.is_empty()
            && one.widths.is_empty()
            && one.symbol != Some(true)
            && !one.spill_reload
            && !one.spill_store
            && !one.frame_adjust
    })
}

/// No two instructions claim the same original byte.
fn _spans(body: &LirBody) -> Vec<String> {
    let mut out = vec![];
    let mut claimed: BTreeMap<i64, i64> = BTreeMap::new();
    for block in &body.blocks {
        for one in &block.insns {
            let Some((lo, hi)) = one.covers else {
                continue;
            };
            if hi < lo {
                out.push(format!("{:#06x} covers {lo:#x}..{hi:#x}, which runs backwards", one.at));
                continue;
            }
            for byte in lo..hi {
                if let Some(owner) = claimed.get(&byte) {
                    out.push(format!("byte {byte:#06x} is claimed by {owner:#06x} and by {:#06x}", one.at));
                    break;
                }
                claimed.insert(byte, one.at);
            }
        }
    }
    out
}

/// No operand names a register class it cannot be in. A MIR operand cannot
/// survive lowering here: `ir::Loc` has no variant for one.
fn _operands(body: &LirBody) -> Vec<String> {
    let mut out = vec![];
    for block in &body.blocks {
        for one in &block.insns {
            let Some(what) = &one.what else {
                continue;
            };
            for r#where in what.dests.iter().chain(&what.sources) {
                let Loc::Reg(reg) = r#where else {
                    continue;
                };
                let register = reg.register as u32;
                if !target::known(reg.register) {
                    out.push(format!("{:#06x} names {register}, which is not a register this target has", one.at));
                }
                if target::width_of(reg.register).is_some_and(|width| width != i64::from(reg.width)) {
                    out.push(format!(
                        "{:#06x} names {register} at width {}, which is not the width that register is",
                        one.at, reg.width
                    ));
                }
            }
        }
    }
    out
}

/// Every value is defined before it is read, and once if this is SSA.
fn _values(body: &LirBody, in_ssa: bool) -> Vec<String> {
    let mut out = vec![];
    let mut written: BTreeMap<u32, usize> = BTreeMap::new();
    for block in &body.blocks {
        for value in block.arrives() {
            *written.entry(value).or_insert(0) += 1;
        }
        for one in &block.insns {
            for value in &one.defines {
                *written.entry(*value).or_insert(0) += 1;
            }
        }
    }
    if in_ssa {
        for (value, times) in &written {
            if *times > 1 {
                out.push(format!("value#{value} is defined {times} times in a body that should be in SSA"));
            }
        }
    }
    if !in_ssa && body.blocks.iter().any(|block| !block.phis.is_empty()) {
        let stuck: Vec<String> =
            body.blocks.iter().filter(|block| !block.phis.is_empty()).map(|block| format!("{:#06x}", block.at)).collect();
        out.push(format!("a phi survives at {} after elimination", stuck.join(", ")));
    }

    // Every value an operand names is a value some instruction defines, or
    // one the caller supplied.
    let mut read: BTreeSet<u32> = BTreeSet::new();
    for block in &body.blocks {
        for one in &block.insns {
            read.extend(one.uses.iter().copied());
            read.extend(one.requires.iter().map(|(held, _)| held.value));
        }
        for phi in &block.phis {
            read.extend(phi.incoming.iter().map(|(_, value)| *value));
        }
    }
    for value in &read {
        if !written.contains_key(value) && !body.inputs.contains(value) {
            out.push(format!("value#{value} is read but never defined or supplied by the caller"));
        }
    }
    let mut named: BTreeSet<u32> = BTreeSet::new();
    for block in &body.blocks {
        for one in &block.insns {
            if let Some(what) = &one.what {
                for r#where in what.dests.iter().chain(&what.sources) {
                    named.extend(ir::values(r#where).into_iter().map(|one| one.value));
                }
            }
        }
    }
    for value in &named {
        if !read.contains(value) && !written.contains_key(value) {
            out.push(format!("value#{value} is named by an operand and neither defined nor used anywhere"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use indexmap::IndexMap;

    use super::*;
    use crate::model::lir::Insn;

    fn held(value: u32, width: u32) -> Loc {
        Loc::Held(ir::Held { value, width })
    }

    fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> ir::Semantics {
        ir::Semantics { name: Some(name.into()), dests, sources, ..ir::Semantics::new(op) }
    }

    fn body(name: &str, blocks: Vec<LirBlock>) -> LirBody {
        LirBody::new(name, 1, blocks, IndexMap::new(), IndexMap::new())
    }

    /// C crc32 returned -1141145971 after an eliminated phi lost its definition.
    #[test]
    fn test_a_read_value_must_be_defined_or_an_explicit_body_input() {
        let stale = Insn::new(
            1,
            Some((1, 1)),
            Some(semantics(Operation::Move, "mov", vec![held(2, 4)], vec![held(1, 4)])),
            vec![2],
            vec![1],
        );
        let said = verify(&body("undefined", vec![LirBlock::new(1, vec![Arc::new(stale)])]), false);
        assert!(said.iter().any(|complaint| complaint.contains("value#1 is read but never defined or supplied")), "{said:?}");
    }

    #[test]
    fn test_an_explicit_body_input_satisfies_the_definition_rule() {
        let incoming =
            Insn::new(1, Some((1, 1)), Some(semantics(Operation::Push, "push", vec![], vec![held(1, 2)])), vec![], vec![1]);
        let mut body = body("input", vec![LirBlock::new(1, vec![Arc::new(incoming)])]);
        body.inputs = BTreeSet::from([1]);
        assert!(verify(&body, false).is_empty());
    }

    #[test]
    fn test_unreachable_byte_ownership_markers_are_not_executable_blocks() {
        let marker = Insn::new(2, Some((2, 4)), Some(semantics(Operation::Nothing, "", vec![], vec![])), vec![], vec![]);
        let body =
            body("dead-ownership", vec![LirBlock::new(1, vec![]), LirBlock::new(2, vec![Arc::new(marker)])]);
        assert!(verify(&body, false).is_empty());
    }

    #[test]
    fn test_unreachable_executable_work_is_still_malformed() {
        let push = semantics(Operation::Push, "push", vec![], vec![Loc::Imm(ir::Imm { value: 1, width: 2, address: None })]);
        let work = Insn::new(2, Some((2, 4)), Some(push), vec![], vec![]);
        let body = body("dead-work", vec![LirBlock::new(1, vec![]), LirBlock::new(2, vec![Arc::new(work)])]);
        assert!(verify(&body, false).iter().any(|complaint| complaint.contains("block 0x0002 is not reachable")));
    }
}
