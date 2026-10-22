//! Rewrites three-address arithmetic into x86's two-address form before
//! register allocation.
//!
//! # How it works
//! An instruction that writes the register of its first source (`Binary`,
//! `Unary`, `Funnel`, and a one-destination `Multiply`) is split into a copy
//! and the instruction on the copy:
//!
//! ```text
//!  c := a + b          ==>      c := a
//!                               c := c + b
//! ```
//!
//! Three things happen first so the copy is less often needed.
//!
//! - Update in place: `c := a + b` followed by the copy `a := c`, with `a` dead
//!   between, and `c` defined once and read only by that copy, becomes `a := a
//!   + b`. No interval grows and the copy the coalescer might refuse is gone.
//! - Commuting (`twoaddress::Commuter`, `tryInstructionCommute`): for `add`,
//!   `and`, `or`, `xor` and `imul` the sources are swapped when the second dies
//!   here and the first does not, or when `isProfitableToCommute` says so: a
//!   register the sum is to end in, a use of the first between its definition
//!   and here, a copy chain, or the second defined later. A sum that ends in a
//!   register (`requires`) keeps the operand delivered in it as the tied one,
//!   so `a + b` returned in EAX is `add eax, edx`.
//! - A memory destination whose cell is not the first source (`[m] := a + b`)
//!   is computed in a fresh register: load, operate, store.
//!
//! A source that is not a value (an immediate) is copied the same way. The
//! inserted copies own no bytes of the input.
//!
//! # Example
//! The body of a counted loop, before and after. Update in place made `v4 :=
//! add v5, v7; v5 := mov v4` into `v5 := add v5, v7`, leaving self-moves for
//! the coalescer:
//!
//! ```lir
//! ; before (stage FloatAlloc)          ; after (stage TwoAddress)
//! block 0x0005                         block 0x0005
//!   v4 := add v5, v7                     v5 := add v5, v7
//!   v6 := add v7, 0x1                    v7 := add v7, 0x1
//!   v5 := mov v4   ; parallel-copy 2     v5 := mov v5   ; parallel-copy 2
//!   v7 := mov v6   ; parallel-copy 2     v7 := mov v7   ; parallel-copy 2
//! ```
//!
//! From `ISEL_DUMP=1` of unit test `test_a_counted_loop` in `isel_tests.rs`.
//!
//! # Runs
//! A machine phase after `floatalloc` and before `coalesce`
//! (`flow::machine_with`), at every level. The coalescer then removes the
//! copies the allocator can share a register across.
//!
//! # See also
//! LLVM's
//! [`TwoAddressInstructionPass`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp),
//! translated in `twoaddress.rs` for the commuting; the rest is this file's
//! own.

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::backend::coalesce;
use crate::model::ir::{self, Held, Loc, Mem, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{IndexMap, IndexSet};

/// Operations that read their destination.
pub const _TIED: [Operation; 4] = [Operation::Binary, Operation::Unary, Operation::Funnel, Operation::Select];

/// `lea`: which tied instructions become a `lea` before allocation (DEVIATE
/// TA-4, twoaddress.rs).
#[derive(Default)]
pub struct TwoAddress {
    pub lea: crate::twoaddress::ThreeAddress,
}

impl TwoAddress {
    pub const NAME: &'static str = "twoaddr";
}

impl LIRTransform for TwoAddress {
    fn class_name(&self) -> &'static str {
        "TwoAddress"
    }

    fn name(&self) -> &str {
        Self::NAME
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(tied(&body, self.lea))
    }
}

/// `body` with every tied instruction reading what it writes.
pub fn tied(
    body: &LirBody,
    lea: crate::twoaddress::ThreeAddress,
) -> LirBody {
    let reused = _reused(body);
    let body = &reused;
    let mut changed = false;
    let mut counter = body.next_value();
    let mut mint = || {
        counter += 1;
        counter - 1
    };

    // What is live at each block's end is read from the rows of bits and what
    // the block's own instructions change laid over them: a set of every live
    // value copied for each block was quadratic in the values live at once.
    let fixed = _fixed(body);
    // The pin's liveness: the intervals, whose ends are the kills
    // (`LiveIntervals`, TwoAddressInstructionPass.cpp#L394-L483).
    let facts = crate::twoaddress::BodyFacts::of(body);
    let mut blocks = Vec::new();
    for (at, block) in body.blocks.iter().enumerate() {
        let mut walk = crate::twoaddress::Commuter::new(&facts, &fixed, at);
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        let mut work: Vec<Arc<Insn>> = block.insns.to_vec();
        let mut place = 0;
        while place < work.len() {
            let mut one = Arc::clone(&work[place]);
            let dist = walk.pass(&one);
            let commuted = walk.try_instruction_commute(&one, dist);
            if let Some(chosen) = &commuted {
                changed = true;
                one = Arc::clone(chosen);
            }
            // `rescheduleMIBelowKill`: a tied instruction whose source dies in
            // a later reader goes below it.
            if commuted.is_none() && walk.reschedule_mi_below_kill(&mut work, place, body.regs(), body.bits) {
                changed = true;
                continue;
            }
            // `convertInstTo3Addr`: a tied add whose source lives on and whose
            // flags nothing reads makes no copy.
            if let Some(three) = crate::twoaddress::three_address(&one, lea)
                && let Some(Loc::Held(base)) = one.what.as_ref().and_then(|what| what.sources.first())
                && !walk.killed_at(&work[place], base.value)
                && crate::twoaddress::flags_dead_after(&work, place, body.regs(), body.bits)
            {
                changed = true;
                insns.push(three);
                place += 1;
                continue;
            }
            place += 1;
            let Some(fix) = _untied(&one, &mut mint) else {
                insns.push(one);
                continue;
            };
            insns.extend(fix);
            changed = true;
        }
        blocks.push(block.with_insns(insns));
    }
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// An update in place: `c := a + b` then `a := c`, with `a` dead between,
/// is `a := a + b`. `c` is defined once and read only by that copy, so the
/// copy the coalescer may refuse is gone and no interval grows.
fn _reused(body: &LirBody) -> LirBody {
    let mut defined: IndexMap<u32, usize> = IndexMap::default();
    let mut read: IndexMap<u32, usize> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        for value in &one.defines {
            *defined.entry(*value).or_insert(0) += 1;
        }
        for value in &one.uses {
            *read.entry(*value).or_insert(0) += 1;
        }
    }
    let mut changed = false;
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
        // Where each value is read and written in the block, in order: what the
        // scans below ask, in a log of the block's length.
        let mut reads: IndexMap<u32, BTreeSet<usize>> = IndexMap::default();
        let mut writes: IndexMap<u32, BTreeSet<usize>> = IndexMap::default();
        let index = |reads: &mut IndexMap<u32, BTreeSet<usize>>,
                     writes: &mut IndexMap<u32, BTreeSet<usize>>,
                     at: usize,
                     one: &Insn,
                     add: bool| {
            for (places, values) in [(&mut *reads, &one.uses), (&mut *writes, &one.defines)] {
                for value in values {
                    if add {
                        places.entry(*value).or_default().insert(at);
                    } else if let Some(found) = places.get_mut(value) {
                        found.remove(&at);
                    }
                }
            }
        };
        for (at, one) in insns.iter().enumerate() {
            index(&mut reads, &mut writes, at, one, true);
        }
        let none = BTreeSet::new();
        for at in 0..insns.len() {
            let Some(what) = insns[at].what.as_ref().filter(|what| ties(what)) else { continue };
            let (Some(Loc::Held(into)), Some(Loc::Held(first))) = (what.dests.first(), what.sources.first()) else {
                continue;
            };
            let (into, first) = (*into, *first);
            let pinned = |value: u32| body.pins.contains_key(&value);
            if what.dests.len() != 1
                || into.value == first.value
                || into.width != first.width
                || pinned(into.value)
                || pinned(first.value)
                || insns[at].group.is_some()
            {
                continue;
            }
            // Defined once and read once in the whole body: not live out, and
            // the one reader is found below.
            if defined.get(&into.value) != Some(&1)
                || read.get(&into.value) != Some(&1)
                || reads.get(&into.value).unwrap_or(&none).range(..at).next().is_some()
            {
                continue;
            }
            // `first` dies here: nothing after reads it, not even a parallel
            // copy beside the one that writes it, which reads the old value.
            if reads.get(&first.value).unwrap_or(&none).range(at + 1..).next().is_some() {
                continue;
            }
            // Only `first := into` reads it: the copy back of an update in
            // place.
            let readers: Vec<usize> = reads.get(&into.value).unwrap_or(&none).range(at + 1..).copied().collect();
            let [last] = readers[..] else { continue };
            let back = insns[last]
                .what
                .as_ref()
                .is_some_and(
                    |what| what.op == Operation::Move && matches!(
                        (what.dests.as_slice(), what.sources.as_slice()),
                        ([Loc::Held(to)], [Loc::Held(from)]) if to.value == first.value && from.value == into.value && to.width == from.width
                    ),
                );
            if !back {
                continue;
            }
            if writes.get(&first.value).unwrap_or(&none).range(at + 1..last).next().is_some() {
                continue;
            }
            let swap = |value: u32| if value == into.value { first.value } else { value };
            for place in at..=last {
                index(&mut reads, &mut writes, place, &insns[place], false);
                insns[place] = coalesce::_renamed(&insns[place], &swap);
                index(&mut reads, &mut writes, place, &insns[place], true);
            }
            changed = true;
        }
        blocks.push(block.with_insns(insns));
    }
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// The values a commutative instruction writes and reads, where
/// `twoaddress::Commuter` may swap its sources.
fn _swappable(one: &Insn) -> Option<(u32, u32, u32)> {
    swappable_held(one).map(|(into, first, second)| (into.value, first.value, second.value))
}

/// Whether the two sources of `what` can swap places (`isCommutable`).
pub fn commutative(what: &Semantics) -> bool {
    (what.op == Operation::Binary && matches!(what.name.as_deref(), Some("add" | "and" | "or" | "xor")))
        || (what.op == Operation::Multiply && what.name.as_deref() == Some("imul"))
}

pub(crate) fn swappable_held(one: &Insn) -> Option<(&Held, &Held, &Held)> {
    let what = one.what.as_ref()?;
    if !commutative(what)
        || what.dests.len() != 1
        || what.sources.len() != 2
        || one.group.is_some()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
    {
        return None;
    }
    let (Loc::Held(into), Loc::Held(first), Loc::Held(second)) = (&what.dests[0], &what.sources[0], &what.sources[1])
    else {
        return None;
    };
    if !(into.width == first.width && first.width == second.width) || into.value == first.value {
        return None;
    }
    Some((into, first, second))
}

/// The registers values are fixed to: those delivered in one (`from`) and
/// those that must end in one (`to`), the latter followed back along the
/// values that die into a sum (LLVM's `SrcRegMap` and `DstRegMap`).
#[derive(Default)]
pub struct Fixed {
    pub from: IndexMap<u32, RegId>,
    pub to: IndexMap<u32, RegId>,
}

fn _fixed(body: &LirBody) -> Fixed {
    let mut fixed = Fixed::default();
    let mut reads: IndexMap<u32, usize> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        for (held, register) in &one.delivers {
            fixed.from.entry(held.value).or_insert(*register);
        }
        for (held, register) in &one.requires {
            fixed.to.entry(held.value).or_insert(*register);
        }
        for value in &one.uses {
            *reads.entry(*value).or_default() += 1;
        }
    }
    // A value read once, by a sum that ends in a register, ends in it too: the
    // sum's operand and its result share the register the pass ties. LLVM
    // follows the chain forward from each value to a copy to the register
    // (`scanUses`, at most `MaxDataFlowEdge` instructions); one backward walk
    // over the body gives the same registers.
    for block in &body.blocks {
        for one in block.insns.iter().rev() {
            let Some((into, first, second)) = _swappable(one) else { continue };
            let Some(register) = fixed.to.get(&into).copied() else { continue };
            for value in [first, second] {
                if reads.get(&value) == Some(&1) {
                    fixed.to.entry(value).or_insert(register);
                }
            }
        }
    }
    fixed
}

/// An empty span at the neighbour's address: this claims no bytes.
fn _nothing(beside: &Insn) -> (i64, i64) {
    let at = beside.covers.map_or(beside.at, |covers| covers.0);
    (at, at)
}

fn _inserted(
    beside: &Insn,
    what: Semantics,
    defines: Vec<u32>,
    uses: Vec<u32>,
) -> Arc<Insn> {
    let mut made = Insn::new(beside.at, Some(_nothing(beside)), Some(what), defines, uses);
    made.call = beside.call.clone();
    Arc::new(made)
}

fn _move(
    into: Loc,
    out_of: Loc,
) -> Semantics {
    Semantics { name: Some("mov".into()), dests: vec![into], sources: vec![out_of], ..Semantics::new(Operation::Move) }
}

/// Whether `what` writes the register of its first source: x86's two-address
/// form.
pub fn ties(what: &Semantics) -> bool {
    !what.dests.is_empty()
        && !what.sources.is_empty()
        && (_TIED.contains(&what.op)
            || (what.op == Operation::Multiply && what.dests.len() == 1 && what.sources.len() == 2))
}

/// The copy and the fixed instruction, or None where it is already tied.
fn _untied(
    one: &Insn,
    mint: &mut dyn FnMut() -> u32,
) -> Option<Vec<Arc<Insn>>> {
    let what = one.what.as_ref()?;
    if what.dests.is_empty() || what.sources.is_empty() {
        return None;
    }
    if !ties(what) {
        return None;
    }
    let (into, first) = (&what.dests[0], &what.sources[0]);
    if let Loc::Mem(cell) = into {
        return _through_register(one, what, cell, mint);
    }
    let Loc::Held(into) = into else {
        return None;
    };
    if !matches!(first, Loc::Held(_) | Loc::Imm(_)) {
        return None;
    }
    if let Loc::Held(first) = first {
        if into.value == first.value {
            return None;
        }
    }
    let first_value = match first {
        Loc::Held(held) => Some(held.value),
        _ => None,
    };
    let mut movement =
        _inserted(one, _move(Loc::Held(*into), first.clone()), vec![into.value], first_value.into_iter().collect());
    // The copy of an undef source reads it as undef
    // (TwoAddressInstructionPass.cpp: `RegBUndef`).
    if let Some(value) = first_value.filter(|value| one.undefs.contains(value)) {
        let copy = Arc::make_mut(&mut movement);
        copy.uses.retain(|used| *used != value);
        copy.undefs.push(value);
    }
    let remaining: BTreeSet<u32> = what.sources[1..].iter().flat_map(ir::values).map(|value| value.value).collect();
    let uses: Vec<u32> = one
        .uses
        .iter()
        .copied()
        .filter(|value| first_value.is_none_or(|first| *value != first || remaining.contains(value)))
        .collect();
    let mut fixed = one.clone();
    let mut sources = vec![Loc::Held(*into)];
    sources.extend(what.sources[1..].iter().cloned());
    fixed.what = Some(Semantics { sources, ..what.clone() });
    fixed.uses = std::iter::once(into.value).chain(uses).collect::<IndexSet<u32>>().into_iter().collect();
    fixed.undefs.retain(|value| first_value != Some(*value) || remaining.contains(value));
    Some(vec![movement, Arc::new(fixed)])
}

/// A memory destination computed in a register, then stored.
fn _through_register(
    one: &Insn,
    what: &Semantics,
    into: &Mem,
    mint: &mut dyn FnMut() -> u32,
) -> Option<Vec<Arc<Insn>>> {
    if what.dests.len() != 1 || what.sources[0] == Loc::Mem(into.clone()) || one.group.is_some() {
        return None;
    }
    if what.sources.iter().any(|source| matches!(source, Loc::Mem(_)))
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
    {
        return None;
    }
    let held = Held { value: mint(), width: into.width };
    let first = &what.sources[0];
    let load = _inserted(
        one,
        _move(Loc::Held(held), first.clone()),
        vec![held.value],
        ir::values(first).iter().map(|value| value.value).collect(),
    );
    let mut computed = one.clone();
    let mut sources = vec![Loc::Held(held)];
    sources.extend(what.sources[1..].iter().cloned());
    computed.what = Some(Semantics { dests: vec![Loc::Held(held)], sources, ..what.clone() });
    computed.defines = vec![held.value];
    computed.uses = std::iter::once(held.value)
        .chain(what.sources[1..].iter().flat_map(ir::values).map(|value| value.value))
        .collect::<IndexSet<u32>>()
        .into_iter()
        .collect();
    let store = _inserted(
        one,
        _move(Loc::Mem(into.clone()), Loc::Held(held)),
        Vec::new(),
        std::iter::once(held.value)
            .chain(ir::values(&Loc::Mem(into.clone())).iter().map(|value| value.value))
            .collect::<IndexSet<u32>>()
            .into_iter()
            .collect(),
    );
    Some(vec![load, Arc::new(computed), store])
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_twoaddr.py`.

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use super::{_untied, Fixed, tied};
    use crate::model::ir::{Held, Imm, Loc, Operation, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;
    use crate::twoaddress::Commuter;
    use crate::twoaddress::ThreeAddress::{All, Off};

    fn held(
        value: u32,
        width: u32,
    ) -> Loc {
        Loc::Held(Held { value, width })
    }

    fn addition(name: &str) -> Insn {
        let what = Semantics {
            name: Some(name.into()),
            dests: vec![held(3, 4)],
            sources: vec![held(1, 4), held(2, 4)],
            ..Semantics::new(Operation::Binary)
        };
        Insn::new(0, Some((0, 2)), Some(what), vec![3], vec![1, 2])
    }

    fn sources(one: &Insn) -> Vec<Loc> {
        one.what.as_ref().expect("semantics").sources.clone()
    }

    fn reversed(one: &Insn) -> Vec<Loc> {
        sources(one).into_iter().rev().collect()
    }

    fn imm_def(
        at: i64,
        value: u32,
    ) -> Arc<Insn> {
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![held(value, 4)],
            sources: vec![Loc::Imm(Imm { value: 7, width: 4, address: None })],
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![value], vec![]))
    }

    fn read(
        at: i64,
        value: u32,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(at, Some((at, at)), None, vec![], vec![value]))
    }

    fn sum(
        at: i64,
        name: &str,
        op: Operation,
    ) -> Arc<Insn> {
        let mut one = addition(name);
        one.what.as_mut().unwrap().op = op;
        one.at = at;
        one.covers = Some((at, at));
        Arc::new(one)
    }

    fn sum_into(
        at: i64,
        into: u32,
        first: u32,
        second: u32,
    ) -> Arc<Insn> {
        let what = Semantics {
            name: Some("add".into()),
            dests: vec![held(into, 4)],
            sources: vec![held(first, 4), held(second, 4)],
            ..Semantics::new(Operation::Binary)
        };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![into], vec![first, second]))
    }

    /// The instruction at `at` of one block, as the commuting walk of `tied`
    /// leaves it.
    fn commuted(
        insns: Vec<Arc<Insn>>,
        at: usize,
        fixed: &Fixed,
    ) -> Option<Arc<Insn>> {
        let body = LirBody::new("t", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        let facts = crate::twoaddress::BodyFacts::of(&body);
        let mut walk = Commuter::new(&facts, fixed, 0);
        let mut found = None;
        for (place, one) in body.blocks[0].insns.iter().enumerate() {
            let dist = walk.pass(one);
            if place == at {
                found = walk.try_instruction_commute(one, dist);
            }
        }
        found
    }

    /// `v3 := op v1, v2` with `v1` read afterwards and `v2` dying at it.
    fn dying_second(
        op_name: &str,
        op: Operation,
    ) -> Vec<Arc<Insn>> {
        vec![imm_def(0, 1), imm_def(1, 2), sum(2, op_name, op), read(3, 1), read(4, 3)]
    }

    /// `six(a..f)` in registers: `a` arrives in EAX, `b` in EDX, the sum is
    /// returned in EAX. The add took `b` for its tied operand and the sum
    /// stayed in EDX: a `mov eax, edx` at the end (LLVM commutes it the other
    /// way, `TwoAddressInstructionPass::isProfitableToCommute`).
    #[test]
    fn test_a_sum_returned_in_eax_is_tied_to_the_operand_that_arrives_there() {
        use llrm_lir::registers::RegId;
        let add = |at: i64, into: u32, first: u32, second: u32| {
            let what = Semantics {
                name: Some("add".into()),
                dests: vec![held(into, 4)],
                sources: vec![held(first, 4), held(second, 4)],
                ..Semantics::new(Operation::Binary)
            };
            Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![into], vec![first, second]))
        };
        let nothing = |at: i64| {
            let what = Semantics { name: Some("".into()), ..Semantics::new(Operation::Nothing) };
            Insn::new(at, Some((at, at)), Some(what), vec![], vec![])
        };
        let mut arrival = nothing(0);
        arrival.defines = vec![1, 2, 3];
        arrival.delivers = vec![
            (Held { value: 1, width: 4 }, RegId::EAX),
            (Held { value: 2, width: 4 }, RegId::EDX),
            (Held { value: 3, width: 4 }, RegId::EBX),
        ];
        let mut ret = nothing(3);
        ret.requires = vec![(Held { value: 5, width: 4 }, RegId::EAX)];
        ret.uses = vec![5];
        let input = LirBody::new(
            "six",
            0,
            vec![LirBlock::new(0, vec![Arc::new(arrival), add(1, 4, 2, 1), add(2, 5, 4, 3), Arc::new(ret)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let got = tied(&input, Default::default());
        let first = got
            .insns()
            .iter()
            .find_map(
                |one| {
                    let what = one.what.as_ref()?;
                    (what.name.as_deref() == Some("add")).then(|| what.clone())
                },
            );
        let first = first.expect("an add");
        let copied: Vec<_> = got
            .insns()
            .iter()
            .filter_map(|one| one.what.as_ref())
            .filter(|what| what.op == Operation::Move)
            .map(|what| what.sources.clone())
            .collect();
        assert_eq!(copied[0], vec![held(1, 4)], "the first add copies the operand that arrives in EAX: {first:?}");
    }

    /// LNGMXX copied its accumulator out and back each iteration to preserve
    /// the invariant addend.
    #[test]
    fn test_commutative_instruction_reuses_the_dying_operand() {
        for name in ["add", "and", "or", "xor"] {
            let one = addition(name);
            let chosen = commuted(dying_second(name, Operation::Binary), 2, &Fixed::default()).expect(name);
            assert_eq!(sources(&chosen), reversed(&one), "{name}");
            assert!(chosen.uses == one.uses && chosen.defines == one.defines, "{name}");
            let mut counter = 1000..2000;
            let (copy, tied) = match _untied(&chosen, &mut || counter.next().unwrap()).expect(name).as_slice() {
                [copy, tied] => (copy.clone(), tied.clone()),
                other => panic!("{name}: {} instructions", other.len()),
            };
            assert_eq!(sources(&copy), vec![held(2, 4)], "{name}");
            assert_eq!(sources(&tied), vec![held(3, 4), held(1, 4)], "{name}");
        }
    }

    /// Matmul tied ``imul`` to its live, spilled factor and reloaded it eight
    /// times.
    #[test]
    fn test_multiply_reuses_the_dying_operand() {
        let chosen = commuted(dying_second("imul", Operation::Multiply), 2, &Fixed::default()).expect("swapped");

        assert_eq!(sources(&chosen), reversed(&addition("imul")));
    }

    /// Nib fixed multiply shifted its multiplier instead of its product.
    #[test]
    fn test_funnel_shift_copies_its_low_source_into_the_destructive_destination() {
        let what = Semantics {
            name: Some("shrd".into()),
            dests: vec![held(3, 4)],
            sources: vec![held(1, 4), held(2, 4), Loc::Imm(Imm { value: 9, width: 1, address: None })],
            ..Semantics::new(Operation::Funnel)
        };
        let one = Insn::new(0, Some((0, 0)), Some(what), vec![3], vec![1, 2]);

        let mut counter = 1000..2000;
        let fix = _untied(&one, &mut || counter.next().unwrap()).expect("untied");
        let [copy, tied] = fix.as_slice() else { panic!("{} instructions", fix.len()) };

        assert_eq!(sources(copy), vec![held(1, 4)]);
        assert_eq!(sources(tied), vec![held(3, 4), held(2, 4), Loc::Imm(Imm { value: 9, width: 1, address: None })]);
    }

    /// copy1d's huge pointer stepped `c := a + 4` then copied `a := c` back
    /// at the latch; under BASIC's register pressure the allocator kept both
    /// copies, and its loop differed from C's by two moves.
    #[test]
    fn test_an_update_copied_back_is_made_in_place() {
        let what = |op, name: &str, dests, sources| {
            Some(Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) })
        };
        let add = Insn::new(
            0,
            Some((0, 0)),
            what(
                Operation::Binary,
                "add",
                vec![held(2, 2)],
                vec![held(1, 2), Loc::Imm(Imm { value: 4, width: 2, address: None })],
            ),
            vec![2],
            vec![1],
        );
        let back = Insn::new(
            1,
            Some((1, 1)),
            what(Operation::Move, "mov", vec![held(1, 2)], vec![held(2, 2)]),
            vec![1],
            vec![2],
        );
        let input = LirBody::new(
            "update",
            0,
            vec![LirBlock { succ: vec![0].into(), ..LirBlock::new(0, vec![Arc::new(add), Arc::new(back)]) }.into()],
            IndexMap::default(),
            IndexMap::default(),
        );

        let got = tied(&input, Default::default());

        let insns = got.insns();
        let first = insns[0].what.as_ref().expect("semantics");
        assert_eq!(
            (first.name.as_deref(), first.dests.clone(), sources(&insns[0])[0].clone()),
            (Some("add"), vec![held(1, 2)], held(1, 2)),
            "{insns:?}"
        );
        assert_eq!(insns.len(), 2, "{insns:?}");
    }

    /// The runtime's number formatting kept `ax := cx` beside `cx := cx + 1`
    /// in its latch's parallel copy; made in place, `ax` read the stepped
    /// count and mandel.nib never finished.
    #[test]
    fn test_an_update_whose_old_value_is_still_copied_is_not_made_in_place() {
        let what = |op, name: &str, dests, sources| {
            Some(Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) })
        };
        let add = Insn::new(
            0,
            Some((0, 0)),
            what(
                Operation::Binary,
                "add",
                vec![held(2, 2)],
                vec![held(1, 2), Loc::Imm(Imm { value: 1, width: 2, address: None })],
            ),
            vec![2],
            vec![1],
        );
        let back = Insn::new(
            1,
            Some((1, 1)),
            what(Operation::Move, "mov", vec![held(1, 2)], vec![held(2, 2)]),
            vec![1],
            vec![2],
        );
        let old = Insn::new(
            1,
            Some((1, 1)),
            what(Operation::Move, "mov", vec![held(3, 2)], vec![held(1, 2)]),
            vec![3],
            vec![1],
        );
        let input = LirBody::new(
            "update",
            0,
            vec![
                LirBlock {
                    succ: vec![0].into(),
                    ..LirBlock::new(0, vec![Arc::new(add), Arc::new(back), Arc::new(old)])
                }
                .into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );

        let got = tied(&input, Default::default());

        let insns = got.insns();
        assert!(
            insns.iter().any(|one| one
                .what
                .as_ref()
                .is_some_and(|what| what.dests == vec![held(2, 2)] && what.name.as_deref() == Some("add"))),
            "{insns:?}"
        );
    }

    #[test]
    fn test_noncommutative_or_implicit_arithmetic_is_not_swapped() {
        for name in ["sub", "adc", "sbb", "shl"] {
            assert!(commuted(dying_second(name, Operation::Binary), 2, &Fixed::default()).is_none(), "{name}");
        }
    }

    #[test]
    fn test_live_operands_and_grouped_operations_keep_their_order() {
        let both_live = vec![imm_def(0, 1), imm_def(1, 2), sum(2, "add", Operation::Binary), read(3, 1), read(4, 2)];
        assert!(commuted(both_live, 2, &Fixed::default()).is_none());
        let mut grouped = dying_second("add", Operation::Binary);
        grouped[2] = Arc::new(Insn { group: Some(1), ..(*grouped[2]).clone() });
        assert!(commuted(grouped, 2, &Fixed::default()).is_none());
    }

    /// `isProfitableToCommute`, the last line: both sources die here and
    /// nothing uses either since its definition; the one defined later has the
    /// shorter interval, so it is the tied one.
    #[test]
    fn test_of_two_dying_sources_the_one_defined_later_is_tied() {
        let later = |first: u32, second: u32| {
            vec![imm_def(0, first), imm_def(1, second), sum(2, "add", Operation::Binary), read(3, 3)]
        };
        // v2 is defined after v1: tied.
        let chosen = commuted(later(1, 2), 2, &Fixed::default()).expect("swapped");
        assert_eq!(sources(&chosen), reversed(&addition("add")));
        // v2 is defined before v1: stays.
        assert!(commuted(later(2, 1), 2, &Fixed::default()).is_none());
    }

    /// `isProfitableToCommute`: a use of the tied source between its last
    /// definition and the instruction (its copy would not join), where the
    /// other source has none.
    #[test]
    fn test_a_tied_source_used_in_between_is_swapped_for_one_that_is_not() {
        let insns = vec![imm_def(0, 1), imm_def(1, 2), read(2, 1), sum(3, "add", Operation::Binary), read(4, 3)];
        let chosen = commuted(insns, 3, &Fixed::default()).expect("swapped");
        assert_eq!(sources(&chosen), reversed(&addition("add")));
        // The same use of the other source: nothing to gain, stays.
        let insns = vec![imm_def(0, 1), imm_def(1, 2), read(2, 2), sum(3, "add", Operation::Binary), read(4, 3)];
        assert!(commuted(insns, 3, &Fixed::default()).is_none());
    }

    fn copy_insn(
        at: i64,
        into: u32,
        from: u32,
    ) -> Arc<Insn> {
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![held(into, 4)],
            sources: vec![held(from, 4)],
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![into], vec![from]))
    }

    /// `getSingleDef(Reg, MBB)` ignores definitions in other blocks: the loop's
    /// temporary is defined in the preheader and by the back-edge copy, and
    /// the chain `c = copy tmp; tmp = copy a` still reaches `a`. A walk
    /// that gave up on a definition elsewhere found no chain, and x_adler's
    /// `(x & 255) + s` tied the `and` result and copied the accumulator
    /// (x_adler -O2 0.968 -> 1.029 of min(gcc, clang)).
    #[test]
    fn test_a_copy_chain_through_a_loop_temporary_defined_in_the_preheader_is_found() {
        let body = LirBody::new(
            "t",
            0,
            vec![
                LirBlock { succ: vec![1].into(), ..LirBlock::new(0, vec![imm_def(0, 2)]) }.into(),
                LirBlock {
                    succ: vec![1, 2].into(),
                    ..LirBlock::new(
                        1,
                        vec![copy_insn(1, 3, 2), imm_def(2, 4), sum_into(3, 5, 4, 3), copy_insn(4, 2, 5)],
                    )
                }
                .into(),
                LirBlock::new(2, vec![read(5, 5)]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let facts = crate::twoaddress::BodyFacts::of(&body);
        let fixed = Fixed::default();
        let walk = Commuter::new(&facts, &fixed, 1);
        assert!(walk.is_rev_copy_chain(3, 5, 10));
    }

    /// `v8 := add v7, k` with `v7` read again by the `and` that follows (so the
    /// add's flags are dead and `v7` lives on).
    fn live_source_add(
        step: i64,
        name: &str,
    ) -> Vec<Arc<Insn>> {
        let what = Semantics {
            name: Some(name.into()),
            dests: vec![held(8, 4)],
            sources: vec![held(7, 4), Loc::Imm(Imm { value: step, width: 4, address: None })],
            ..Semantics::new(Operation::Binary)
        };
        let add = Arc::new(Insn::new(1, Some((1, 1)), Some(what), vec![8], vec![7]));
        vec![imm_def(0, 7), add, sum_into(2, 6, 8, 7), read(3, 6)]
    }

    fn names_of(body: &LirBody) -> Vec<String> {
        body.insns().iter().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
    }

    fn of(insns: Vec<Arc<Insn>>) -> LirBody {
        LirBody::new("t", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default())
    }

    /// pop's `x &= x - 1`: the pin's `convertInstTo3Addr` makes the `dec` of a
    /// live `x` a `lea` (TwoAddressInstructionPass.cpp#L1402), so no `t :=
    /// mov x` pins `t` to `x`'s register; the old pass copied, and
    /// x_popcount kept a `mov` per iteration (+10.4% clocks, AM-TA-1).
    #[test]
    fn test_an_add_of_a_live_source_with_dead_flags_is_a_lea_not_a_copy() {
        let got = tied(&of(live_source_add(-1, "add")), All);
        let names = names_of(&got);
        assert_eq!(names.iter().filter(|name| *name == "lea").count(), 1, "{names:?}");
        assert_eq!(
            names.iter().filter(|name| *name == "mov").count(),
            2,
            "the def of v7 and the copy for the tied and: {names:?}"
        );
        let lea = got
            .insns()
            .iter()
            .find(|one| one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some("lea")))
            .cloned()
            .unwrap();
        let Loc::Mem(cell) = &lea.what.as_ref().unwrap().sources[0] else { panic!("a cell") };
        assert_eq!((cell.base.map(|held| held.value), cell.offset), (Some(7), -1));
    }

    #[test]
    fn test_a_sub_of_an_immediate_is_a_lea_of_its_negation() {
        let got = tied(&of(live_source_add(5, "sub")), All);
        let lea = got
            .insns()
            .iter()
            .find(|one| one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some("lea")))
            .cloned()
            .expect("a lea");
        let Loc::Mem(cell) = &lea.what.as_ref().unwrap().sources[0] else { panic!("a cell") };
        assert_eq!(cell.offset, -5);
    }

    /// `SHL32ri` by 1 to 3 of a live source is a `lea` too: `[a + a]`, `[a*4]`,
    /// `[a*8]`; a shift of 4 or a shift of a register stays.
    #[test]
    fn test_a_shift_by_one_to_three_of_a_live_source_is_a_scaled_lea() {
        for (shift, scale, base) in [(1, 1, true), (2, 4, false), (3, 8, false)] {
            let got = tied(&of(live_source_add(shift, "shl")), All);
            let lea = got
                .insns()
                .iter()
                .find(|one| one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some("lea")))
                .cloned()
                .unwrap_or_else(|| panic!("a lea for shl {shift}"));
            let Loc::Mem(cell) = &lea.what.as_ref().unwrap().sources[0] else { panic!("a cell") };
            assert_eq!((cell.index.map(|held| held.value), cell.scale, cell.base.is_some()), (Some(7), scale, base));
        }
        assert!(!names_of(&tied(&of(live_source_add(4, "shl")), All)).contains(&"lea".to_owned()));
    }

    /// A commuted instruction is a new one, which the intervals do not know:
    /// the kill of its tied source is asked of the instruction as it was
    /// (the sweep failed 120 of 568 objects with "Instruction not found in
    /// maps" on the first version of the conversion).
    #[test]
    fn test_a_commuted_add_is_still_asked_about_its_kills_by_the_instruction_it_was() {
        let insns = vec![imm_def(0, 1), imm_def(1, 2), sum_into(2, 3, 1, 2), read(3, 1), read(4, 3)];
        let got = tied(&of(insns), All);
        assert!(names_of(&got).contains(&"add".to_owned()) || names_of(&got).contains(&"lea".to_owned()));
    }

    /// A target that reads addresses through a prefix (m16) converts adds and
    /// subs and leaves a shift to the peephole, which fuses it with the add
    /// after it: `c*3` was `lea eax, [ecx+ecx*2]` and became
    /// `lea eax, [ecx+ecx]; add eax, ecx` (test_m16_emits_386_forms).
    #[test]
    fn test_the_prefixed_mode_converts_an_add_and_leaves_a_shift() {
        use crate::twoaddress::ThreeAddress::AddSub;
        let lea = |got: &LirBody| names_of(got).contains(&"lea".to_owned());
        assert!(lea(&tied(&of(live_source_add(-1, "add")), AddSub)));
        assert!(!lea(&tied(&of(live_source_add(1, "shl")), AddSub)));
    }

    /// `v5 := lea [v1+4]`: reads `v1`, writes no flags, is no copy and no tied
    /// instruction.
    fn lea_of(
        at: i64,
        into: u32,
        base: u32,
    ) -> Arc<Insn> {
        use crate::model::ir::Mem;
        let cell = Mem { base: Some(Held { value: base, width: 4 }), offset: 4, ..Mem::new(None, 4) };
        let what = Semantics {
            name: Some("lea".into()),
            dests: vec![held(into, 4)],
            sources: vec![Loc::Mem(cell)],
            ..Semantics::new(Operation::Address)
        };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![into], vec![base]))
    }

    fn position(
        body: &LirBody,
        name: &str,
    ) -> usize {
        names_of(body).iter().position(|one| one == name).unwrap_or_else(|| panic!("no {name}"))
    }

    /// `rescheduleMIBelowKill`: `v3 := add v1, 5` while `v1` is read once more,
    /// by a `lea`, and dies there: the add goes below the `lea`, so `v1`
    /// dies at the add and the copy it needs joins.
    #[test]
    fn test_a_tied_add_goes_below_the_one_reader_that_kills_its_source() {
        let add = live_source_add(5, "add")[1].clone();
        let insns = vec![imm_def(0, 7), add, lea_of(2, 5, 7), read(3, 8), read(4, 5)];
        // `live_source_add` reads v7 and defines v8.
        let got = tied(&of(insns), Off);
        assert!(position(&got, "lea") < position(&got, "add"), "{:?}", names_of(&got));
    }

    /// The pin's checks: a kill that is a copy, or a call between, leaves the
    /// add where it was.
    #[test]
    fn test_a_tied_add_stays_where_the_kill_is_a_copy_or_something_stands_between() {
        let add = || live_source_add(5, "add")[1].clone();
        let copy_kill = vec![imm_def(0, 7), add(), copy_insn(2, 5, 7), read(3, 8), read(4, 5)];
        let got = tied(&of(copy_kill), Off);
        assert!(
            position(&got, "add") < position(&got, "mov") || names_of(&got).iter().filter(|n| *n == "mov").count() >= 2
        );
        let call = {
            let what = Semantics { name: Some("call".into()), ..Semantics::new(Operation::Call) };
            Arc::new(Insn::new(2, Some((2, 2)), Some(what), vec![], vec![]))
        };
        let blocked = vec![imm_def(0, 7), add(), call, lea_of(3, 5, 7), read(4, 8), read(5, 5)];
        let got = tied(&of(blocked), Off);
        assert!(position(&got, "add") < position(&got, "lea"), "{:?}", names_of(&got));
    }

    #[test]
    fn test_an_add_stays_two_address_where_its_source_dies_its_flags_live_or_the_target_is_not_native() {
        // not native
        assert!(!names_of(&tied(&of(live_source_add(-1, "add")), Off)).contains(&"lea".to_owned()));
        // the source dies: `v7` is not read after the add
        let dies = vec![imm_def(0, 7), live_source_add(-1, "add")[1].clone(), read(2, 8)];
        assert!(!names_of(&tied(&of(dies), All)).contains(&"lea".to_owned()));
        // the flags are read: a conditional jump follows the add
        let mut flags_live = live_source_add(-1, "add");
        flags_live[2] = Arc::new(Insn::new(
            2,
            Some((2, 2)),
            Some(Semantics { name: Some("jne".into()), target: Some(0), ..Semantics::new(Operation::Branch) }),
            vec![],
            vec![],
        ));
        flags_live.push(read(4, 7));
        assert!(!names_of(&tied(&of(flags_live), All)).contains(&"lea".to_owned()));
    }

    // ------------------------------------------------------ tests/test_lir.py

    fn imm(
        value: i64,
        width: u32,
    ) -> Loc {
        Loc::Imm(Imm { value, width, address: None })
    }

    fn fixed(one: &Insn) -> Option<Vec<std::sync::Arc<Insn>>> {
        let mut counter = 1000..2000;
        _untied(one, &mut || counter.next().unwrap())
    }

    /// hotlop printed 420 for 630 when 21 + accumulator lost its 21.
    #[test]
    fn test_two_address_materializes_a_constant_first_operand() {
        let (result, source) = (held(900, 2), held(901, 2));
        let what = Semantics {
            name: Some("add".into()),
            dests: vec![result.clone()],
            sources: vec![imm(21, 2), source.clone()],
            ..Semantics::new(Operation::Binary)
        };
        let insn = Insn::new(0, Some((0, 3)), Some(what), vec![900], vec![901]);
        let fixed = fixed(&insn).expect("untied");
        assert_eq!(sources(&fixed[0]), [imm(21, 2)]);
        assert!(fixed[0].uses.is_empty());
        assert_eq!(sources(&fixed[1]), [result, source]);
    }

    /// Experimental matrix setup emitted 20 * 20 for 0 * 20 without a
    /// destination copy.
    #[test]
    fn test_two_address_multiply_preserves_its_first_factor() {
        let (result, first, second) = (held(900, 2), held(901, 2), held(902, 2));
        let what = Semantics {
            name: Some("imul".into()),
            dests: vec![result.clone()],
            sources: vec![first.clone(), second.clone()],
            ..Semantics::new(Operation::Multiply)
        };
        let insn = Insn::new(0, Some((0, 3)), Some(what), vec![900], vec![901, 902]);
        let fixed = fixed(&insn).expect("untied");
        assert_eq!(sources(&fixed[0]), [first]);
        assert_eq!(sources(&fixed[1]), [result, second]);
        assert_eq!(fixed[1].uses.iter().copied().collect::<BTreeSet<u32>>(), BTreeSet::from([900, 902]));
    }

    /// hotlop kept its old accumulator live through the add after copying it.
    #[test]
    fn test_two_address_copy_ends_the_original_source_use() {
        let (result, source) = (held(900, 2), held(901, 2));
        let what = Semantics {
            name: Some("add".into()),
            dests: vec![result],
            sources: vec![source, imm(21, 2)],
            ..Semantics::new(Operation::Binary)
        };
        let insn = Insn::new(0, Some((0, 3)), Some(what), vec![900], vec![901]);
        let fixed = fixed(&insn).expect("untied");
        assert_eq!(fixed[1].uses, [900]);
    }

    /// Update in place and the tied rewrite each walked the body for liveness
    /// rows: twice a body, 4.0 M of TwoAddress's 10.0 M at d_faces -O2. The
    /// update in place asks only whether a value read once is read in its own
    /// block, which the use counts answer.
    #[test]
    fn test_two_address_walks_the_body_for_liveness_once_and_builds_no_intervals() {
        let imm = |at: i64, value: u32| imm_def(at, value);
        let add = |at: i64, into: u32, first: u32, second: u32| {
            let what = Semantics {
                name: Some("add".into()),
                dests: vec![held(into, 4)],
                sources: vec![held(first, 4), held(second, 4)],
                ..Semantics::new(Operation::Binary)
            };
            Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![into], vec![first, second]))
        };
        let copy = |at: i64, into: u32, from: u32| {
            let what = Semantics {
                name: Some("mov".into()),
                dests: vec![held(into, 4)],
                sources: vec![held(from, 4)],
                ..Semantics::new(Operation::Move)
            };
            Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![into], vec![from]))
        };
        let input = LirBody::new(
            "update",
            0,
            vec![LirBlock::new(0, vec![imm(0, 5), imm(1, 7), add(2, 4, 5, 7), copy(3, 5, 4)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let before = crate::backend::allocate::live_rows_walks(&input);
        let got = tied(&input, Default::default());
        let names: Vec<_> =
            got.insns().iter().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect();
        assert_eq!(names, ["mov", "mov", "add", "mov"], "the premise: the update in place was found: {names:?}");
        assert_eq!(crate::backend::allocate::live_rows_walks(&input) - before, 1);
        // The pin shares one LiveIntervals with the coalescer; here the kills
        // are one backward scan over the live-out rows and none is
        // built (TA-6): building one per phase cost +3.0 M Ir at m32.
        let built = crate::liveness::liveintervals::BUILT.with(std::cell::Cell::get);
        let _ = tied(&input, Default::default());
        assert_eq!(crate::liveness::liveintervals::BUILT.with(std::cell::Cell::get), built);
    }
}
