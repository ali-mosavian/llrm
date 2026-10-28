//! Checks that a lowered body is well formed, between machine phases.
//!
//! # How it works
//! `verify` returns the complaints as sentences; empty is well formed. It
//! checks:
//!
//! - Blocks: the entry and every successor exist, and every block is reachable,
//!   except unreachable blocks that only hold byte ownership.
//! - Spans: no two instructions claim the same original byte, and no span runs
//!   backwards.
//! - Operands: every register named exists on the target at the width it is.
//! - Values: each is defined before it is read, once in SSA, no phi survives
//!   phi elimination, and a value read is defined or an explicit body input.
//! - Slots: once instruction selection tagged them (`slotted`), every frame
//!   cell names its slot.
//! - Frequencies: the block frequency table has entries for existing blocks and
//!   runs no block more often than its predecessors send
//!   (`Frequency::violations`).
//!
//! `verify_for` adds, for a target with one address space and no segments, that
//! no operand is a segment register, no cell has a selector or an override
//! other than the stack's or data's, and no base or index register is narrower
//! than the target's addresses.
//!
//! # Example
//! A body whose only instruction, `v2 := mov v1`, reads a value nothing
//! defines.
//!
//! Schematic, from the fixture of test
//! `test_a_read_value_must_be_defined_or_an_explicit_body_input` (not printer
//! output):
//! ```lir
//! v2 := mov v1
//! ```
//!
//! ```text
//! value#1 is read but never defined or supplied
//! ```
//!
//! From unit test `test_a_read_value_must_be_defined_or_an_explicit_body_input`
//! in `verify.rs`.
//!
//! # Runs
//! After instruction selection and after every machine phase when `LLRM_VERIFY`
//! is set to anything but `0` (`flow::verified`, `flow::checked`). Tests, the
//! gate and torture set it. A failure names the phase.
//!
//! # See also
//! LLVM's
//! [`MachineVerifier`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/MachineVerifier.cpp).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use llrm_lir::registers::RegId;

use crate::backend::peephole::is_frame_base;
use crate::model::ir::{self, Loc, Operation};
use crate::model::lir::{Block, LirBlock, LirBody};

/// A machine phase produced LIR that violates an established invariant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Malformed(pub String);

impl fmt::Display for Malformed {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Malformed {}

/// `verify`, and for a target with one address space what a flat program does
/// not have: a segment register, a selector or a segment override. The target's
/// description says whether it has one (`Spaces::far_is_near`, no segments).
pub fn verify_for(
    body: &LirBody,
    in_ssa: bool,
    spaces: &llrm_mir::spaces::Spaces,
) -> Vec<String> {
    let mut out = verify(body, in_ssa);
    if spaces.far_is_near() && spaces.segment_bytes.is_none() {
        out.extend(_flat(body));
        out.extend(_wide_addresses(body));
    }
    out
}

/// No operand is a segment register, and no cell has a selector or an override
/// of any segment but the stack's and the data's, which are the one space here
/// (the encoder drops them): the machine of the target has no other to name.
fn _flat(body: &LirBody) -> Vec<String> {
    const DS: RegId = RegId::DS;
    const NO_SEGMENT: RegId = RegId::None;
    const SS: RegId = RegId::SS;
    let other = |segment: iced_x86::Register| {
        let segment = RegId::from(segment);
        segment != NO_SEGMENT && segment != SS && segment != DS
    };
    let mut out = vec![];
    for block in &body.blocks {
        for one in &block.insns {
            let Some(what) = &one.what else { continue };
            for place in what.dests.iter().chain(&what.sources) {
                let named = match place {
                    Loc::Reg(reg) if llrm_x86::registers::SEGMENTS.contains(&reg.register) => {
                        Some(format!("the segment register {:?}", reg.register))
                    }
                    Loc::Mem(cell) if cell.selector.is_some() => Some("a cell with a selector".to_owned()),
                    _ if place.address().and_then(|one| one.addr).is_some_and(|addr| other(addr.segment)) => {
                        Some("a segment override".to_owned())
                    }
                    _ => None,
                };
                if let Some(what) = named {
                    out.push(format!("{:#06x} names {what}, which a target with one address space has not", one.at));
                }
            }
        }
    }
    out
}

/// Everything wrong with this body, as sentences. Empty is well formed.
pub fn verify(
    body: &LirBody,
    in_ssa: bool,
) -> Vec<String> {
    let mut out = _blocks(body);
    out.extend(_spans(body));
    out.extend(_operands(body));
    out.extend(_values(body, in_ssa));
    out.extend(_slots(body));
    out.extend(crate::analysis::frequency::Frequency::violations(body));
    out
}

/// After allocation, what the machine form reads: no register lane is read
/// before every path to it has written it, unless the entry delivers it (an
/// argument), the function preserves it (its epilogue reads it) or it is
/// machine state (stack, frame and segment registers; flags are not checked).
/// The checks of [`_values`] are of virtual values, which the machine form does
/// not have (MachineVerifier: "Using an undefined physical register").
pub fn physical(
    body: &LirBody,
    preserved: &BTreeSet<RegId>,
) -> Vec<String> {
    use crate::backend::lanes::Lanes;
    use crate::backend::liveness;
    use crate::backend::peephole::_lanes;
    use crate::support::hash::IndexMap;
    let regs = body.regs();
    let machine_state =
        |register: RegId| regs.is_segment(register) || regs.is_frame(register) || regs.is_stack(register);
    let mut entering = Lanes::new();
    for block in body.blocks.iter().filter(|block| block.at == body.entry) {
        for (held, register) in block.insns.iter().flat_map(|one| one.delivers.iter()) {
            entering = entering.or(&_lanes(regs, regs.named(*register, i64::from(held.width))));
        }
    }
    for register in regs.integer_registers().iter().copied().chain(regs.segments()) {
        if preserved.contains(&regs.root(register)) || machine_state(register) {
            entering = entering.or(&_lanes(regs, register));
        }
    }
    let everything = liveness::_universe(regs);
    let effects: IndexMap<i64, Vec<Option<liveness::Effect>>> =
        body.blocks.iter().map(|block| (block.at, liveness::_effects(regs, body.bits, block))).collect();
    // Forward, must: a lane is defined on entry to a block when it is defined
    // at the end of every predecessor. Walked to a fixed point from "all".
    let preds = {
        let mut preds: IndexMap<i64, Vec<i64>> = body.blocks.iter().map(|block| (block.at, Vec::new())).collect();
        for block in &body.blocks {
            for next in block.succ.iter().filter(|next| preds.contains_key(*next)).copied().collect::<Vec<_>>() {
                preds[&next].push(block.at);
            }
        }
        preds
    };
    // At 64 bits a read of RAX also reaches the lane above EAX, which no
    // 32-bit view of the register names.
    let whole = |register: RegId| {
        let lanes = _lanes(regs, register);
        regs.view(regs.root(register), body.bits).map_or(lanes, |all| lanes.or(&_lanes(regs, all)))
    };
    let walked = |at: i64, defined: Lanes, complaint: &mut Lanes| -> Lanes {
        let mut defined = defined;
        let block = body.blocks.iter().find(|block| block.at == at).expect("a block of the body");
        for (one, effect) in block.insns.iter().zip(&effects[&at]) {
            match effect {
                // `movzx ebx, bx` zeroes the upper half and hands the word on
                // as it is: an undefined word stays undefined, and is not
                // read (the preheader's copy for a loop that writes the word,
                // `upperzero::established`).
                Some(effect) if _widened_word(one).is_some() => {
                    let word = _lanes(regs, _widened_word(one).expect("tested"));
                    *complaint = complaint.or(&effect.reads.minus(&defined).minus(&word));
                    defined = defined.or(&effect.writes.minus(&word.minus(&defined)));
                }
                // An instruction nothing is known of may write anything.
                None => defined = everything,
                Some(effect) => {
                    // A register with some lane defined is read whole: its
                    // other lanes are the consumer's to ignore (a short
                    // argument is read as the register that holds it: at m64 a
                    // word argument is read as the quadword).
                    let touched: Lanes = effect
                        .reads
                        .iter()
                        .filter(|&&(register, _)| !_lanes(regs, register).is_disjoint(&defined))
                        .fold(Lanes::new(), |all, &(register, _)| all.or(&whole(register)));
                    *complaint = complaint.or(&effect.reads.minus(&defined).minus(&touched));
                    defined = defined.or(&effect.writes);
                }
            }
        }
        defined
    };
    let mut out_of: IndexMap<i64, Lanes> = body.blocks.iter().map(|block| (block.at, everything)).collect();
    loop {
        let mut changed = false;
        for block in &body.blocks {
            let defined = if block.at == body.entry {
                entering
            } else {
                preds[&block.at].iter().fold(everything, |all, pred| all.and(&out_of[pred]))
            };
            let after = walked(block.at, defined, &mut Lanes::new());
            if after != out_of[&block.at] {
                out_of.insert(block.at, after);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut complaint = Lanes::new();
    for block in &body.blocks {
        let defined = if block.at == body.entry {
            entering
        } else {
            preds[&block.at].iter().fold(everything, |all, pred| all.and(&out_of[pred]))
        };
        walked(block.at, defined, &mut complaint);
    }
    let seen: BTreeSet<RegId> =
        complaint.iter().map(|&(register, _)| register).filter(|register| *register != RegId::None).collect();
    if seen.is_empty() {
        return Vec::new();
    }
    let names: Vec<String> = seen.iter().map(|one| regs.name(*one).unwrap_or("?").to_owned()).collect();
    vec![format!(
        "{} reads {} before every path writes it, and the entry does not deliver it",
        body.name,
        names.join(", ")
    )]
}

/// The word register of `movzx r32, r16` on one register: its low word is not
/// an input but passed through.
fn _widened_word(one: &crate::model::lir::Insn) -> Option<RegId> {
    use crate::model::ir::Loc;
    let what = one.what.as_ref().filter(|what| what.name.as_deref() == Some("movzx"))?;
    match (what.dests.as_slice(), what.sources.as_slice()) {
        ([Loc::Reg(wide)], [Loc::Reg(word)])
            if wide.width == 4 && word.width == 2 && wide.register.full_register() == word.register.full_register() =>
        {
            Some(word.register)
        }
        _ => None,
    }
}

/// Once instruction selection has tagged them, every frame cell names the slot
/// it lies in: a phase that makes a cell from a displacement alone leaves the
/// frame layout unable to move it.
fn _slots(body: &LirBody) -> Vec<String> {
    if !body.slotted {
        return Vec::new();
    }
    let mut out = Vec::new();
    for block in &body.blocks {
        for one in &block.insns {
            let Some(what) = &one.what else { continue };
            for place in what.dests.iter().chain(&what.sources) {
                let addr = place.address().and_then(|one| one.addr);
                if let Some(addr) = addr.filter(|addr| addr.space == ir::Space::Frame && addr.slot_home().is_none()) {
                    out.push(format!("{:#06x}: a frame cell at {} names no slot", one.at, addr.disp));
                }
            }
        }
    }
    out
}

/// The entry and successors exist; unreachable work is rejected.
fn _blocks(body: &LirBody) -> Vec<String> {
    let mut out = vec![];
    let at_of: BTreeMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
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
    for block in &body.blocks {
        for what in block.insns.iter().filter_map(|one| one.what.as_ref()).filter(|what| !what.table.is_empty()) {
            let (mut listed, mut leaves) = (what.table.clone(), block.succ.clone());
            listed.sort_unstable();
            listed.dedup();
            leaves.sort_unstable();
            leaves.dedup();
            if listed != leaves || listed.iter().any(|one| !at_of.contains_key(one)) {
                out.push(format!(
                    "block {:#06x}'s table jump lists blocks {listed:x?} and the block goes to {leaves:x?}",
                    block.at
                ));
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
    block
        .insns
        .iter()
        .all(
            |one| one.what.as_ref().is_some_and(|what| {
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
                && !one.frame_adjust,
        )
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

/// In flat code a base or an index is as wide as the target's addresses: the
/// frame register's width. The frame and stack bases the frame layout resolves
/// are no register read.
fn _wide_addresses(body: &LirBody) -> Vec<String> {
    let regs = body.regs();
    let wide = regs.width_of(regs.frame).unwrap_or(0);
    let mut out = vec![];
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        let Some(what) = &one.what else { continue };
        for at in what.dests.iter().chain(&what.sources).filter_map(Loc::address) {
            for (register, framed) in [
                (at.through, is_frame_base(regs, at.addr, at.through) || regs.is_stack(at.through)),
                (at.index_through, false),
            ] {
                if register != RegId::None && !framed && regs.width_of(register).is_some_and(|width| width < wide) {
                    out.push(format!(
                        "{:#06x} addresses through {}, narrower than this target's {wide}-byte addresses",
                        one.at,
                        regs.name(register).unwrap_or("?")
                    ));
                }
            }
        }
    }
    out
}

/// No operand names a register class it cannot be in. A MIR operand cannot
/// survive lowering here: `ir::Loc` has no variant for one.
fn _operands(body: &LirBody) -> Vec<String> {
    let regs = body.regs();
    let mut out = vec![];
    for block in &body.blocks {
        for one in &block.insns {
            let Some(what) = &one.what else {
                continue;
            };
            for r#where in what.dests.iter().chain(&what.sources) {
                // Only a segment register can be the segment of an address: a
                // dword register there is `byte ptr ecx:[bx]`, a miscompile.
                if let Some(segment) = r#where.address().and_then(|one| one.addr).map(|addr| RegId::from(addr.segment))
                {
                    if segment != RegId::None && !regs.is_segment(segment) {
                        out.push(format!("{:#06x} takes {segment:?} as the segment of an address", one.at));
                    }
                }
                let Loc::Reg(reg) = r#where else {
                    continue;
                };
                // A position in a stack is not a register the target's tables
                // list.
                if regs.positional(reg.register) {
                    continue;
                }
                // FP0..FP6 are the stackifier's to place, never encoded.
                if regs.in_class(reg.register, llrm_lir::registers::class::RFP) {
                    continue;
                }
                let register = reg.register.index() as u32;
                if !regs.described(reg.register) {
                    out.push(format!("{:#06x} names {register}, which is not a register this target has", one.at));
                }
                if regs.width_of(reg.register).is_some_and(|width| width != i64::from(reg.width)) {
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
fn _values(
    body: &LirBody,
    in_ssa: bool,
) -> Vec<String> {
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
    // A phi reads one value for each predecessor of its block, and from none
    // else (the IR verifier's rule: "PHI nodes must have one entry for each
    // predecessor").
    let mut preds: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
    for block in &body.blocks {
        for &to in block.succ.iter() {
            preds.entry(to).or_default().insert(block.at);
        }
    }
    for block in &body.blocks {
        for phi in &block.phis {
            for (from, _) in &phi.incoming {
                if !preds.get(&block.at).is_some_and(|set| set.contains(from)) {
                    out.push(format!("a phi at {:#06x} reads from {from:#06x}, which is not a predecessor", block.at));
                }
            }
        }
    }
    if !in_ssa && body.blocks.iter().any(|block| !block.phis.is_empty()) {
        let stuck: Vec<String> = body
            .blocks
            .iter()
            .filter(|block| !block.phis.is_empty())
            .map(|block| format!("{:#06x}", block.at))
            .collect();
        out.push(format!("a phi survives at {} after elimination", stuck.join(", ")));
    }

    // Every value an operand names is a value some instruction defines, or
    // one the caller supplied.
    let mut read: BTreeSet<u32> = BTreeSet::new();
    for block in &body.blocks {
        for one in &block.insns {
            read.extend(one.uses.iter().copied());
            // A value an operand reads as undef needs no definition.
            read.extend(one.requires.iter().map(|(held, _)| held.value).filter(|value| !one.undefs.contains(value)));
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
    let undef: BTreeSet<u32> =
        body.blocks.iter().flat_map(|block| block.insns.iter()).flat_map(|one| one.undefs.iter().copied()).collect();
    for value in &named {
        if !read.contains(value) && !written.contains_key(value) && !undef.contains(value) {
            out.push(format!("value#{value} is named by an operand and neither defined nor used anywhere"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::*;
    use crate::model::lir::Insn;
    use crate::support::hash::IndexMap;

    fn held(
        value: u32,
        width: u32,
    ) -> Loc {
        Loc::Held(ir::Held { value, width })
    }

    fn semantics(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> ir::Semantics {
        ir::Semantics { name: Some(name.into()), dests, sources, ..ir::Semantics::new(op) }
    }

    fn body(
        name: &str,
        blocks: Vec<Block>,
    ) -> LirBody {
        LirBody::new(name, 1, blocks, IndexMap::default(), IndexMap::default())
    }

    /// C crc32 returned -1141145971 after an eliminated phi lost its
    /// definition.
    /// An IR phi has one entry for each predecessor and no other: a LIR phi
    /// that reads from a block that is not one is a body a pass broke (the
    /// old PHIElimination left it alone, silently).
    #[test]
    fn test_a_phi_reading_from_a_block_that_is_not_a_predecessor_is_reported() {
        use crate::model::lir::{LirBlock, Phi};
        let entry = LirBlock { succ: vec![1].into(), ..LirBlock::new(0, Vec::new()) };
        let next = LirBlock {
            phis: vec![Phi { result: 2, incoming: vec![(0, 1), (7, 1)] }].into(),
            ..LirBlock::new(1, Vec::new())
        };
        let body = LirBody::new("p", 0, vec![entry.into(), next.into()], IndexMap::default(), IndexMap::default());
        let said = verify(&body, true);
        assert!(said.iter().any(|one| one.contains("is not a predecessor")), "{said:?}");
    }

    #[test]
    fn test_a_read_value_must_be_defined_or_an_explicit_body_input() {
        let stale = Insn::new(
            1,
            Some((1, 1)),
            Some(semantics(Operation::Move, "mov", vec![held(2, 4)], vec![held(1, 4)])),
            vec![2],
            vec![1],
        );
        let said = verify(&body("undefined", vec![LirBlock::new(1, vec![Arc::new(stale)]).into()]), false);
        assert!(
            said.iter().any(|complaint| complaint.contains("value#1 is read but never defined or supplied")),
            "{said:?}"
        );
    }

    #[test]
    fn test_an_explicit_body_input_satisfies_the_definition_rule() {
        let incoming = Insn::new(
            1,
            Some((1, 1)),
            Some(semantics(Operation::Push, "push", vec![], vec![held(1, 2)])),
            vec![],
            vec![1],
        );
        let mut body = body("input", vec![LirBlock::new(1, vec![Arc::new(incoming)]).into()]);
        body.inputs = BTreeSet::from([1]);
        assert!(verify(&body, false).is_empty());
    }

    /// A table jump whose block went to other blocks than the table names: the
    /// listing then had a label the object did not define ("references to
    /// nothing defined or declared: L0_14").
    #[test]
    fn test_a_table_jump_goes_where_its_block_goes() {
        let jump = |table: Vec<i64>| {
            let what =
                ir::Semantics { indirect: true, table, ..semantics(Operation::Jump, "jmp", vec![], vec![held(1, 2)]) };
            Arc::new(Insn::new(1, Some((1, 1)), Some(what), vec![], vec![1]))
        };
        let ret = || {
            Arc::new(Insn::new(
                2,
                Some((2, 2)),
                Some(semantics(Operation::Return, "ret", vec![], vec![])),
                vec![],
                vec![],
            ))
        };
        let blocks = |table: Vec<i64>| {
            vec![
                LirBlock { succ: vec![2, 3].into(), ..LirBlock::new(1, vec![jump(table)]) },
                LirBlock::new(2, vec![ret()]),
                LirBlock::new(3, vec![ret()]),
            ]
        };
        let mut body = body("table", crate::model::lir::shared(blocks(vec![2, 3, 2])));
        body.inputs = BTreeSet::from([1]);
        assert!(verify(&body, false).is_empty(), "{:?}", verify(&body, false));
        let mut stale = self::body("stale", crate::model::lir::shared(blocks(vec![2, 9, 2])));
        stale.inputs = BTreeSet::from([1]);
        let said = verify(&stale, false);
        assert!(said.iter().any(|complaint| complaint.contains("table jump lists")), "{said:?}");
    }

    #[test]
    fn test_unreachable_byte_ownership_markers_are_not_executable_blocks() {
        let marker =
            Insn::new(2, Some((2, 4)), Some(semantics(Operation::Nothing, "", vec![], vec![])), vec![], vec![]);
        let body = body(
            "dead-ownership",
            vec![LirBlock::new(1, vec![]).into(), LirBlock::new(2, vec![Arc::new(marker)]).into()],
        );
        assert!(verify(&body, false).is_empty());
    }

    #[test]
    fn test_unreachable_executable_work_is_still_malformed() {
        let push =
            semantics(Operation::Push, "push", vec![], vec![Loc::Imm(ir::Imm { value: 1, width: 2, address: None })]);
        let work = Insn::new(2, Some((2, 4)), Some(push), vec![], vec![]);
        let body =
            body("dead-work", vec![LirBlock::new(1, vec![]).into(), LirBlock::new(2, vec![Arc::new(work)]).into()]);
        assert!(verify(&body, false).iter().any(|complaint| complaint.contains("block 0x0002 is not reachable")));
    }

    /// A flat target has one address space and no segments: what its
    /// description says, not an `-m32` check, decides that a segment
    /// register or a selector in its code is a defect (real mode's
    /// description says otherwise).
    #[test]
    fn test_a_target_with_one_address_space_refuses_what_names_a_segment() {
        let reg = |register| Loc::Reg(ir::Reg { register, width: 2 });
        let push = |source: Loc, uses: Vec<u32>| {
            let what = semantics(Operation::Push, "push", vec![], vec![source]);
            let mut body = body(
                "segments",
                vec![LirBlock::new(1, vec![Arc::new(Insn::new(1, Some((1, 1)), Some(what), vec![], uses))]).into()],
            );
            body.inputs = BTreeSet::from([1]);
            body
        };
        let selected = Loc::Mem(ir::Mem {
            selector: Some(ir::Held { value: 1, width: 2 }),
            ..ir::Mem::new(Some(ir::Addr::new(ir::Space::Segment, 0)), 2)
        });
        let flat = llrm_target::Target::layout(&llrm_x86_m32::M32).spaces.roles;
        let real = llrm_target::Target::layout(&llrm_x86_m16::M16).spaces.roles;
        for (source, uses, says) in
            [(reg(RegId::ES), vec![], "segment register ES"), (selected, vec![1], "a cell with a selector")]
        {
            let body = push(source, uses);
            assert_eq!(verify_for(&body, false, &real), Vec::<String>::new(), "real mode has segments");
            let said = verify_for(&body, false, &flat);
            assert!(said.iter().any(|complaint| complaint.contains(says)), "{said:?}");
        }
    }

    /// `ecx:[bx]`, a call's dword register as the segment of an address, was
    /// emitted by QCport's `copyrows` and ran; only a segment register can be
    /// one.
    #[test]
    fn test_only_a_segment_register_is_the_segment_of_an_address() {
        let cell =
            |segment| Loc::Mem(ir::Mem::new(Some(ir::Addr { segment, ..ir::Addr::new(ir::Space::Segment, 0) }), 1));
        let load = |segment| {
            let what = semantics(
                Operation::Move,
                "mov",
                vec![Loc::Reg(ir::Reg { register: RegId::AL, width: 1 })],
                vec![cell(segment)],
            );
            body(
                "segment",
                vec![LirBlock::new(1, vec![Arc::new(Insn::new(1, Some((1, 1)), Some(what), vec![], vec![]))]).into()],
            )
        };
        let said = verify(&load(RegId::ECX.iced()), false);
        assert!(said.iter().any(|complaint| complaint.contains("ECX as the segment of an address")), "{said:?}");
        assert_eq!(verify(&load(RegId::ES.iced()), false), Vec::<String>::new());
    }

    /// Under RFP an x87 operand is `FP0`..`FP6` after allocation;
    /// the check named it "not a register this target has" and refused every
    /// float function (`LLRM_VERIFY=1`, tests/run/c/float_in_place_update.c).
    #[test]
    fn test_an_x87_register_operand_is_a_register_the_target_has() {
        let store = semantics(
            Operation::FloatStore,
            "fstp",
            vec![Loc::Mem(ir::Mem::new(Some(ir::Addr::new(ir::Space::Frame, -8)), 8))],
            vec![Loc::Reg(ir::Reg {
                register: llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO).rfp_registers()[0],
                width: 10,
            })],
        );
        let body = body(
            "x87",
            vec![LirBlock::new(1, vec![Arc::new(Insn::new(1, Some((1, 1)), Some(store), vec![], vec![]))]).into()],
        );
        assert_eq!(verify(&body, false), Vec::<String>::new());
    }

    /// A phase that made a frame cell from a displacement alone left the frame
    /// layout unable to move it: a `slotted` body is refused one, and a
    /// cell of another space (a global whose `index` is a symbol's) is not a
    /// frame cell.
    #[test]
    fn test_a_slotted_body_refuses_a_frame_cell_that_names_no_slot() {
        let cell = |addr: ir::Addr| Loc::Mem(ir::Mem { through: RegId::BP, ..ir::Mem::new(Some(addr), 2) });
        let store = |addr| {
            let what = semantics(
                Operation::Move,
                "mov",
                vec![cell(addr)],
                vec![Loc::Imm(ir::Imm { value: 1, width: 2, address: None })],
            );
            Insn::new(2, Some((2, 4)), Some(what), vec![], vec![])
        };
        let named = |addr: ir::Addr, slotted: bool| {
            let mut made = body("slots", vec![LirBlock::new(1, vec![Arc::new(store(addr))]).into()]);
            made.slotted = slotted;
            verify(&made, false)
        };
        let bare = ir::Addr::new(ir::Space::Frame, -4);
        assert!(named(bare, false).is_empty(), "an untagged body is not checked");
        assert!(named(bare, true).iter().any(|complaint| complaint.contains("a frame cell at -4 names no slot")));
        assert!(named(bare.in_slot(-4), true).is_empty());
        assert!(named(ir::Addr::new(ir::Space::Literal, -4), true).is_empty());
    }

    fn register(
        register: RegId,
        width: u32,
    ) -> Loc {
        Loc::Reg(ir::Reg { register, width })
    }

    fn physical_body(insns: Vec<Insn>) -> LirBody {
        body("allocated", vec![LirBlock::new(1, insns.into_iter().map(Arc::new).collect()).into()])
    }

    /// `mov ax, bx` with nothing before it: BX is not an argument, so the
    /// machine form reads a register nothing wrote ("Using an undefined
    /// physical register"). The virtual check could not see it: no value is
    /// named.
    #[test]
    fn test_a_register_read_before_any_write_is_refused_after_allocation() {
        let read = Insn::new(
            1,
            Some((1, 1)),
            Some(semantics(Operation::Move, "mov", vec![register(RegId::AX, 2)], vec![register(RegId::BX, 2)])),
            vec![],
            vec![],
        );
        let said = physical(&physical_body(vec![read]), &BTreeSet::new());
        assert!(said.iter().any(|complaint| complaint.contains("reads ebx before every path writes it")), "{said:?}");
    }

    /// `movzx ebx, bx` before a loop that writes the word (STATIC.BAS -O2,
    /// LLRM_VERIFY=1 refused a valid program): the word is passed on, not read,
    /// so only a later read of it complains.
    #[test]
    fn test_a_widening_of_an_undefined_word_is_not_a_read_of_it() {
        let widen = Insn::new(
            1,
            Some((1, 1)),
            Some(semantics(Operation::Extend, "movzx", vec![register(RegId::BX, 4)], vec![register(RegId::BX, 2)])),
            vec![],
            vec![],
        );
        let mov = |at: i64, to: Loc, from: Loc| {
            Insn::new(at, Some((at, at)), Some(semantics(Operation::Move, "mov", vec![to], vec![from])), vec![], vec![])
        };
        let one = Loc::Imm(ir::Imm { value: 1, width: 2, address: None });
        let writes = mov(2, register(RegId::BX, 2), one);
        let reads = mov(3, register(RegId::AX, 2), register(RegId::BX, 2));
        let none = BTreeSet::new();
        assert!(physical(&physical_body(vec![widen.clone(), writes]), &none).is_empty());
        let said = physical(&physical_body(vec![widen, reads]), &none);
        assert!(said.iter().any(|complaint| complaint.contains("reads ebx before every path writes it")), "{said:?}");
    }

    /// The same read is fine where the function preserves BX (its epilogue
    /// reads it) and where an earlier write defines it.
    #[test]
    fn test_a_register_written_first_or_preserved_is_not_refused() {
        let write = Insn::new(
            1,
            Some((1, 1)),
            Some(semantics(
                Operation::Move,
                "mov",
                vec![register(RegId::BX, 2)],
                vec![Loc::Imm(ir::Imm { value: 1, width: 2, address: None })],
            )),
            vec![],
            vec![],
        );
        let read = Insn::new(
            2,
            Some((2, 2)),
            Some(semantics(Operation::Move, "mov", vec![register(RegId::AX, 2)], vec![register(RegId::BX, 2)])),
            vec![],
            vec![],
        );
        assert!(physical(&physical_body(vec![write, read.clone()]), &BTreeSet::new()).is_empty());
        assert!(
            physical(&physical_body(vec![read]), &BTreeSet::from([physical_body(vec![]).regs().root(RegId::BX)]))
                .is_empty()
        );
    }

    /// `mov ax, 1` then `mov rcx, rax` at 64 bits (`load volatile i16` and
    /// word stores, `LLRM_VERIFY=1 -m64 -O2 --isel=dag`): the quadword read
    /// reached the high-dword lane of RAX, which the "read whole" allowance
    /// left out, and a valid program was refused.
    #[test]
    fn test_a_word_written_register_readwhole_at_64_bits_is_not_refused() {
        let mov = |at: i64, to: Loc, from: Loc| {
            Insn::new(at, Some((at, at)), Some(semantics(Operation::Move, "mov", vec![to], vec![from])), vec![], vec![])
        };
        let one = Loc::Imm(ir::Imm { value: 1, width: 2, address: None });
        let mut made = physical_body(vec![
            mov(1, register(RegId::AX, 2), one),
            mov(2, register(RegId::RCX, 8), register(RegId::RAX, 8)),
        ]);
        made.bits = 64;
        made.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m64::REGISTER_INFO));
        let said = physical(&made, &BTreeSet::new());
        assert!(said.is_empty(), "{said:?}");
    }
}
