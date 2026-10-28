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

use crate::model::ir::{self, Held, Loc, Mem, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{IndexMap, IndexSet};

/// Operations that read their destination.
pub const _TIED: [Operation; 5] =
    [Operation::Binary, Operation::Unary, Operation::Funnel, Operation::Select, Operation::Exchange];

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
    let mut counter = body.next_value();
    let mut mint = || {
        counter += 1;
        counter - 1
    };

    // What is live at each block's end is read from the rows of bits and what
    // the block's own instructions change laid over them: a set of every live
    // value copied for each block was quadratic in the values live at once.
    // The pin's liveness: the intervals, whose ends are the kills
    // (`LiveIntervals`, TwoAddressInstructionPass.cpp#L394-L483).
    let facts = crate::twoaddress::BodyFacts::of(body);
    let flags_out = crate::backend::peephole::_flags_live_out(body);
    for (at, block) in body.blocks.iter().enumerate() {
        let flags_dead_out = flags_out.entering.get(&block.at).is_some_and(crate::backend::lanes::Lanes::is_empty);
        let mut walk = crate::twoaddress::Commuter::sized(&facts, at, block.insns.len());
        let mut insns: Vec<Arc<Insn>> = Vec::with_capacity(block.insns.len() + block.insns.len() / 4);
        let mut work: Vec<Arc<Insn>> = block.insns.to_vec();
        let mut place = 0;
        while place < work.len() {
            let mut one = Arc::clone(&work[place]);
            let dist = walk.pass(&one);
            // The copies to and from registers this instruction carries.
            walk.process_copy(&one);
            // `scanUses(regA)` of a tied instruction that is not yet tied.
            if let Some(what) = one.what.as_ref().filter(|what| ties(what))
                && let (Some(Loc::Held(into)), Some(Loc::Held(first))) = (what.dests.first(), what.sources.first())
                && into.value != first.value
            {
                walk.scan_uses_of(into.value);
            }
            let commuted = walk.try_instruction_commute(&one, dist);
            if let Some(chosen) = &commuted {
                facts.rewrite(at, work[place].id(), Arc::clone(chosen));
                work[place] = Arc::clone(chosen);
                one = Arc::clone(chosen);
            }
            // `rescheduleMIBelowKill`: a tied instruction whose source dies in
            // a later reader goes below it.
            if commuted.is_none() && walk.reschedule_mi_below_kill(&mut work, place, body.regs(), body.bits) {
                continue;
            }
            // `convertInstTo3Addr`: a tied add whose source lives on and whose
            // flags nothing reads makes no copy.
            if let Some(three) = crate::twoaddress::three_address(&one, lea)
                && let Some(Loc::Held(base)) = one.what.as_ref().and_then(|what| what.sources.first())
                // A word source copied out of a register it arrives in
                // (`SrcRegMap`) is in ax, cx or dx, none of which is a 16-bit
                // address base: the
                // `lea` would need `mov bx,ax` first, and the peephole already
                // makes `lea cx,[eax+k]` of the copy and the add.
                && (base.width != 2 || walk.src_register(base.value).is_none())
                && (!walk.killed_at(&work[place], base.value)
                    || profitable_to_convert(&walk, &one, base.value, body.regs()))
                && crate::twoaddress::flags_dead_after(&work, place, body.regs(), body.bits, flags_dead_out)
            {
                if let Some(what) = one.what.as_ref()
                    && what.name.as_deref() == Some("add")
                    && matches!(what.sources.get(1), Some(Loc::Held(_)))
                    && let Some(Loc::Held(into)) = what.dests.first()
                {
                    walk.note_lea(into.value);
                }
                if let Some(what) = one.what.as_ref()
                    && let (Some(Loc::Held(into)), Some(Loc::Held(base))) = (what.dests.first(), what.sources.first())
                {
                    walk.converted_to_three_address(work[place].id(), three.id(), into.value, base.value);
                }
                facts.rewrite(at, work[place].id(), Arc::clone(&three));
                work[place] = Arc::clone(&three);
                walk.remove_clobbered_src_reg_map(&three);
                insns.push(three);
                place += 1;
                continue;
            }
            // `rescheduleKillAboveMI`: the reader that kills the source goes
            // above the tied instruction.
            if commuted.is_none() && walk.reschedule_kill_above_mi(&mut work, place, body.regs(), body.bits) {
                continue;
            }
            // An instruction with a load folded into it whose first source
            // lives on: the load is unfolded and the instruction
            // commuted, so the loaded value is the tied one
            // (`mov t, [m]; add t, a`, not `mov d, a; add d, [m]`;
            // `tryInstructionTransform`).
            if commuted.is_none()
                && let Some((load, add)) = _unfolded(&one, &mut mint)
                && let Some(Loc::Held(first)) = add.what.as_ref().and_then(|what| what.sources.get(1))
                && !walk.killed_at(&work[place], first.value)
                && let Some(fix) = _untied(&add, &mut mint)
            {
                place += 1;
                walk.remove_clobbered_src_reg_map(&one);
                let mut made = vec![load];
                made.extend(fix);
                if let [load, copy, fixed] = &made[..]
                    && let (Some(&into), Some(&tied)) = (copy.defines.first(), copy.uses.first())
                {
                    let _ = load;
                    let dies = !fixed.uses.contains(&tied);
                    walk.tied_pair_processed(copy.id(), fixed.id(), into, tied, dies);
                }
                facts.replace(at, &one, &made);
                insns.extend(made);
                continue;
            }
            place += 1;
            walk.remove_clobbered_src_reg_map(&one);
            let Some(fix) = _untied(&one, &mut mint) else {
                insns.push(one);
                continue;
            };
            // The copy `into := mov first` and what `first`'s death at `one`
            // makes of the maps.
            if let [copy, fixed] = &fix[..]
                && let (Some(&into), Some(&first)) = (copy.defines.first(), copy.uses.first())
            {
                let dies = facts.killed(one.id(), first) && !fixed.uses.contains(&first);
                walk.tied_pair_processed(copy.id(), fixed.id(), into, first, dies);
            }
            facts.replace(at, &one, &fix);
            insns.extend(fix);
        }
        debug_assert!(
            insns.iter().map(|one| one.id()).eq(facts.body_like(body).blocks[at].insns.iter().map(|one| one.id())),
            "the intervals' block is not the pass's"
        );
    }
    facts.finish(body)
}

/// `unfoldMemoryOperand` of a commutative instruction (`add`, `and`, `or`,
/// `xor`, the two-operand `imul`) `d := op a, [m]` whose
/// `a` is another value: the load `t := mov [m]` and `d := op t, a`, commuted
/// so that `t` is the tied source.
fn _unfolded(
    one: &Insn,
    mint: &mut dyn FnMut() -> u32,
) -> Option<(Arc<Insn>, Arc<Insn>)> {
    let what = one.what.as_ref()?;
    // The two-operand `imul` commutes like `add`: `IMUL32rm`'s unfold is
    // `IMUL32rr`.
    let commutative = match what.op {
        Operation::Binary => matches!(
            what.name.as_deref(),
            Some("add" | "adc" | "and" | "or" | "xor")
        ),
        Operation::Multiply => what.name.as_deref() == Some("imul"),
        _ => false,
    };
    if !commutative
        || what.dests.len() != 1
        || what.sources.len() != 2
        || one.volatile
        || one.group.is_some()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
    {
        return None;
    }
    let (Loc::Held(into), Loc::Held(first), Loc::Mem(cell)) = (&what.dests[0], &what.sources[0], &what.sources[1])
    else {
        return None;
    };
    if into.value == first.value || into.width != first.width || cell.width != first.width {
        return None;
    }
    let loaded = Held { value: mint(), width: first.width };
    let reads: Vec<u32> = ir::values(&Loc::Mem(cell.clone())).iter().map(|held| held.value).collect();
    let load = _inserted(one, _move(Loc::Held(loaded), Loc::Mem(cell.clone())), vec![loaded.value], reads);
    let mut add = one.clone();
    add.what = Some(Semantics { sources: vec![Loc::Held(loaded), Loc::Held(*first)], ..what.clone() });
    add.uses = std::iter::once(loaded.value).chain(std::iter::once(first.value)).collect();
    Some((load, Arc::new(add)))
}

/// An update in place: `c := a + b` then `a := c`, with `a` dead between,
/// is `a := a + b`. `c` is defined once and read only by that copy, so the
/// copy the coalescer may refuse is gone and no interval grows.
fn _reused(body: &LirBody) -> LirBody {
    // How many times each value is defined and read in the whole body.
    let mut defined: Vec<u32> = Vec::new();
    let mut read: Vec<u32> = Vec::new();
    let count = |counts: &mut Vec<u32>, value: u32| {
        let value = value as usize;
        if counts.len() <= value {
            counts.resize(value + 1, 0);
        }
        counts[value] += 1;
    };
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        one.defines.iter().for_each(|&value| count(&mut defined, value));
        one.uses.iter().for_each(|&value| count(&mut read, value));
    }
    let times = |counts: &Vec<u32>, value: u32| counts.get(value as usize).copied().unwrap_or(0);
    let mut changed = false;
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
        // Where each value is read and written in the block, in order: what the
        // scans below ask, by bisection. Made at the first instruction that
        // passes the counts of the whole body: most blocks have none.
        let mut reads = Places::default();
        let mut writes = Places::default();
        let mut indexed = false;
        let index = |reads: &mut Places, writes: &mut Places, at: usize, one: &Insn, add: bool| {
            for (places, values) in [(&mut *reads, &one.uses), (&mut *writes, &one.defines)] {
                for &value in values {
                    if add {
                        places.add(value, at);
                    } else {
                        places.remove(value, at);
                    }
                }
            }
        };
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
            if times(&defined, into.value) != 1 || times(&read, into.value) != 1 {
                continue;
            }
            if !indexed {
                indexed = true;
                for (place, one) in insns.iter().enumerate() {
                    index(&mut reads, &mut writes, place, one, true);
                }
            }
            if reads.at(into.value).first().is_some_and(|&place| place < at) {
                continue;
            }
            // `first` dies here: nothing after it reads it, not even a parallel
            // copy beside the one that writes it, which reads the old value.
            if !reads.in_range(first.value, at + 1, usize::MAX).is_empty() {
                continue;
            }
            // Only `first := into` reads it: the copy back of an update in
            // place.
            let readers = reads.in_range(into.value, at + 1, usize::MAX);
            let [last] = *readers else { continue };
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
            if !writes.in_range(first.value, at + 1, last).is_empty() {
                continue;
            }
            let swap = |value: u32| if value == into.value { first.value } else { value };
            for place in at..=last {
                index(&mut reads, &mut writes, place, &insns[place], false);
                insns[place] = insns[place].renamed(&swap);
                index(&mut reads, &mut writes, place, &insns[place], true);
            }
            changed = true;
        }
        blocks.push(block.with_insns(insns));
    }
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

// How many places `_reused` entered, for a test that a block with no update in
// place enters none.
#[cfg(test)]
thread_local! {
    static ADDED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The places in a block each value is named, ascending.
#[derive(Default)]
struct Places(IndexMap<u32, Vec<usize>>);

impl Places {
    fn add(
        &mut self,
        value: u32,
        at: usize,
    ) {
        #[cfg(test)]
        ADDED.with(|count| count.set(count.get() + 1));
        let places = self.0.entry(value).or_default();
        if let Err(found) = places.binary_search(&at) {
            places.insert(found, at);
        }
    }

    fn remove(
        &mut self,
        value: u32,
        at: usize,
    ) {
        if let Some(places) = self.0.get_mut(&value)
            && let Ok(found) = places.binary_search(&at)
        {
            places.remove(found);
        }
    }

    fn at(
        &self,
        value: u32,
    ) -> &[usize] {
        self.0.get(&value).map_or(&[], Vec::as_slice)
    }

    /// The places from `from` up to, not including, `to`.
    fn in_range(
        &self,
        value: u32,
        from: usize,
        to: usize,
    ) -> &[usize] {
        let places = self.at(value);
        let (low, high) = (places.partition_point(|&place| place < from), places.partition_point(|&place| place < to));
        &places[low..high]
    }
}

/// The values a commutative instruction writes and reads, where
/// `twoaddress::Commuter` may swap its sources.
fn _swappable(one: &Insn) -> Option<(u32, u32, u32)> {
    swappable_held(one).map(|(into, first, second)| (into.value, first.value, second.value))
}

/// Whether the two sources of `what` can swap places (`isCommutable`).
pub fn commutative(what: &Semantics) -> bool {
    (what.op == Operation::Binary && matches!(
        what.name.as_deref(),
        Some("add" | "adc" | "and" | "or" | "xor")
    ))
        || (what.op == Operation::Multiply && what.name.as_deref() == Some("imul"))
}

pub(crate) fn swappable_held(one: &Insn) -> Option<(&Held, &Held, &Held)> {
    let what = one.what.as_ref()?;
    let cmov = what.op == Operation::Select
        && what.name.as_deref().and_then(super::flagscopy::opposite_cmov).is_some();
    if !(commutative(what) || cmov)
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

/// `isProfitableToConv3Addr`: a source that dies is still made three-address
/// when the sum is to end in a register other than the one the source arrives
/// in, so no copy of it is made (`SrcRegMap[regB]` against `DstRegMap[regA]`,
/// `regsAreCompatible` false).
fn profitable_to_convert(
    walk: &crate::twoaddress::Commuter,
    one: &Insn,
    source: u32,
    regs: llrm_lir::registers::Regs,
) -> bool {
    let Some(Loc::Held(into)) = one.what.as_ref().and_then(|what| what.dests.first()) else { return false };
    match (walk.src_register(source), walk.dst_register(into.value)) {
        (Some(from), Some(to)) => regs.root(from) != regs.root(to),
        _ => false,
    }
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
            || (what.op == Operation::Multiply
                && what.dests.len() == 1
                && what.sources.len() == 2
                // `IMUL32rri` has no tied operand: it reads any source and
                // writes any register.
                && !matches!(what.sources[1], Loc::Imm(_))))
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
    // An undef source is dealt with at once: its operand is the destination,
    // read as undef, and no copy is made (`collectTiedOperands`: "rewrite
    // undef").
    if let Some(value) = first_value.filter(|value| one.undefs.contains(value))
        && !what.sources[1..].iter().flat_map(ir::values).any(|held| held.value == value)
    {
        let mut fixed = one.clone();
        let mut sources = vec![Loc::Held(*into)];
        sources.extend(what.sources[1..].iter().cloned());
        fixed.what = Some(Semantics { sources, ..what.clone() });
        fixed.uses.retain(|used| *used != value);
        fixed.undefs.retain(|undef| *undef != value);
        if !fixed.undefs.contains(&into.value) {
            fixed.undefs.push(into.value);
        }
        return Some(vec![Arc::new(fixed)]);
    }
    let mut movement =
        _inserted(one, _move(Loc::Held(*into), first.clone()), vec![into.value], first_value.into_iter().collect());
    // A source the instruction reads again as another operand is still copied,
    // reading it as undef where the instruction did (`RegBUndef`).
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

    use llrm_lir::registers::RegId;

    use super::{_untied, tied};
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
    ) -> Option<Arc<Insn>> {
        let body = LirBody::new("t", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        let facts = crate::twoaddress::BodyFacts::of(&body);
        let mut walk = Commuter::new(&facts, 0);
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

    /// The sum of four arguments in registers, the pin's tree: `(a+b)+(c+d)`.
    /// `c` and `d` die into `c+d`, whose sum ends in EAX, not where `c`
    /// arrives: it is an `lea` (`isProfitableToConv3Addr`), no copy of `c`, and
    /// the add it feeds keeps the `lea` as its second source
    /// (`hasCommutePreference`) so X86FixupLEAs can split the pair. Without
    /// both the listing of `six` kept `mov` + `add` for `c+d` and ended
    /// `mov eax, edx`.
    #[test]
    fn test_a_dying_source_whose_sum_ends_elsewhere_is_an_lea_that_the_next_add_keeps_second() {
        use llrm_lir::registers::RegId;
        let nothing = |at: i64| {
            let what = Semantics { name: Some("".into()), ..Semantics::new(Operation::Nothing) };
            Insn::new(at, Some((at, at)), Some(what), vec![], vec![])
        };
        let mut arrival = nothing(0);
        arrival.defines = vec![1, 2, 3, 4];
        arrival.delivers = vec![
            (Held { value: 1, width: 4 }, RegId::EAX),
            (Held { value: 2, width: 4 }, RegId::EDX),
            (Held { value: 3, width: 4 }, RegId::EBX),
            (Held { value: 4, width: 4 }, RegId::ECX),
        ];
        let mut ret = nothing(4);
        ret.requires = vec![(Held { value: 9, width: 4 }, RegId::EAX)];
        ret.uses = vec![9];
        let input = LirBody::new(
            "six",
            0,
            vec![
                LirBlock::new(
                    0,
                    vec![
                        Arc::new(arrival),
                        sum_into(1, 7, 2, 1),
                        sum_into(2, 12, 3, 4),
                        sum_into(3, 9, 7, 12),
                        Arc::new(ret),
                    ],
                )
                .into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let got = tied(&input, All);
        let named = |name: &str| {
            got.insns()
                .iter()
                .filter_map(|one| one.what.as_ref())
                .filter(|what| what.name.as_deref() == Some(name))
                .count()
        };
        assert_eq!(named("lea"), 1, "c+d is an lea");
        let insns = got.insns();
        let adds: Vec<&Semantics> = insns
            .iter()
            .filter_map(|one| one.what.as_ref())
            .filter(|what| what.name.as_deref() == Some("add"))
            .collect();
        let last = adds.last().expect("an add");
        assert_eq!(last.sources, vec![held(9, 4), held(12, 4)], "the lea stays second: the sum is tied to (a+b)");
    }

    /// LNGMXX copied its accumulator out and back each iteration to preserve
    /// the invariant addend.
    #[test]
    fn test_commutative_instruction_reuses_the_dying_operand() {
        for name in ["add", "and", "or", "xor"] {
            let one = addition(name);
            let chosen = commuted(dying_second(name, Operation::Binary), 2).expect(name);
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
        let chosen = commuted(dying_second("imul", Operation::Multiply), 2).expect("swapped");

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
        for name in ["sub", "sbb", "shl"] {
            assert!(commuted(dying_second(name, Operation::Binary), 2).is_none(), "{name}");
        }
    }

    /// `adc` is commutable like `add` (X86 `ADC32rr`, `isCommutable`): a dying
    /// second source was left untied, costing a copy in front of every wide
    /// add.
    #[test]
    fn test_the_dying_second_source_of_an_adc_is_tied() {
        let chosen = commuted(dying_second("adc", Operation::Binary), 2).expect("swapped");
        assert_eq!(sources(&chosen), reversed(&addition("adc")));
    }

    #[test]
    fn test_live_operands_and_grouped_operations_keep_their_order() {
        let both_live = vec![imm_def(0, 1), imm_def(1, 2), sum(2, "add", Operation::Binary), read(3, 1), read(4, 2)];
        assert!(commuted(both_live, 2).is_none());
        let mut grouped = dying_second("add", Operation::Binary);
        grouped[2] = Arc::new(Insn { group: Some(1), ..(*grouped[2]).clone() });
        assert!(commuted(grouped, 2).is_none());
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
        let chosen = commuted(later(1, 2), 2).expect("swapped");
        assert_eq!(sources(&chosen), reversed(&addition("add")));
        // v2 is defined before v1: stays.
        assert!(commuted(later(2, 1), 2).is_none());
    }

    /// `isProfitableToCommute`: a use of the tied source between its last
    /// definition and the instruction (its copy would not join), where the
    /// other source has none.
    #[test]
    fn test_a_tied_source_used_in_between_is_swapped_for_one_that_is_not() {
        let insns = vec![imm_def(0, 1), imm_def(1, 2), read(2, 1), sum(3, "add", Operation::Binary), read(4, 3)];
        let chosen = commuted(insns, 3).expect("swapped");
        assert_eq!(sources(&chosen), reversed(&addition("add")));
        // The same use of the other source: nothing to gain, stays.
        let insns = vec![imm_def(0, 1), imm_def(1, 2), read(2, 2), sum(3, "add", Operation::Binary), read(4, 3)];
        assert!(commuted(insns, 3).is_none());
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
        let walk = Commuter::new(&facts, 1);
        assert!(walk.is_rev_copy_chain(3, 5, 10));
    }

    /// `v8 := add v7, k` with `v7` read again by the `and` that follows (so the
    /// add's flags are dead and `v7` lives on).
    fn live_source_add(
        step: i64,
        name: &str,
    ) -> Vec<Arc<Insn>> {
        live_source_add_of(step, name, 4)
    }

    fn live_source_add_of(
        step: i64,
        name: &str,
        width: u32,
    ) -> Vec<Arc<Insn>> {
        let what = Semantics {
            name: Some(name.into()),
            dests: vec![held(8, width)],
            sources: vec![held(7, width), Loc::Imm(Imm { value: step, width, address: None })],
            ..Semantics::new(Operation::Binary)
        };
        let add = Arc::new(Insn::new(1, Some((1, 1)), Some(what), vec![8], vec![7]));
        vec![imm_def(0, 7), add, sum_into(2, 6, 8, 7), read(3, 6)]
    }

    fn names_of(body: &LirBody) -> Vec<String> {
        body.iter_insns().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
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

    /// queens (AM-BT-6): `row + 1` was `mov t, row; inc t`, which the allocator
    /// could not rematerialize and spilled and reloaded; the pin's `INC32r`
    /// is converted to `LEA32r` like an add of one
    /// (`convertToThreeAddress`).
    #[test]
    fn test_an_inc_or_dec_of_a_live_source_is_a_lea_not_a_copy() {
        for (name, offset) in [("inc", 1), ("dec", -1)] {
            let what = Semantics {
                name: Some(name.into()),
                dests: vec![held(8, 4)],
                sources: vec![held(7, 4)],
                ..Semantics::new(Operation::Unary)
            };
            let step = Arc::new(Insn::new(1, Some((1, 1)), Some(what), vec![8], vec![7]));
            let got = tied(&of(vec![imm_def(0, 7), step, sum_into(2, 6, 8, 7), read(3, 6)]), All);
            let lea = got
                .insns()
                .iter()
                .find(|one| one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some("lea")))
                .cloned()
                .unwrap_or_else(|| {
                    panic!("{name} of a live source stays a copy and a tied {name}: {:?}", names_of(&got))
                });
            let Loc::Mem(cell) = &lea.what.as_ref().unwrap().sources[0] else { panic!("a cell") };
            assert_eq!((cell.base.map(|held| held.value), cell.offset), (Some(7), offset));
        }
    }

    /// queens `_place`: `row + 1` at the end of a block whose successor writes
    /// the flags first stayed `mov t, row; inc t` (flags past the block's end
    /// were taken as live), so it was spilled, not made again at the call.
    #[test]
    fn test_an_inc_whose_flags_the_next_block_writes_first_is_a_lea() {
        let what = Semantics {
            name: Some("inc".into()),
            dests: vec![held(8, 4)],
            sources: vec![held(7, 4)],
            ..Semantics::new(Operation::Unary)
        };
        let step = Arc::new(Insn::new(1, Some((1, 1)), Some(what), vec![8], vec![7]));
        let body = LirBody::new(
            "t",
            0,
            vec![
                LirBlock { succ: vec![1].into(), ..LirBlock::new(0, vec![imm_def(0, 7), step]) }.into(),
                LirBlock::new(1, vec![sum_into(2, 6, 8, 7), read(3, 6)]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let names = names_of(&tied(&body, All));
        assert!(names.contains(&"lea".to_owned()), "an inc of a live source stays a copy and an inc: {names:?}");
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

    /// textfill at m16 looped `add ax,2; cmp bx,3997; mov bx,ax; jb`: a word
    /// add was never a `lea`, so its result needed the copy back. A prefixed
    /// target converts it (`cmp bx,3997; lea bx,[bx+2]; jb`); a word add of
    /// two registers stays an add (only bx+si and bx+di address).
    #[test]
    fn test_a_word_add_of_a_live_source_is_a_lea_on_a_prefixed_target() {
        use crate::twoaddress::ThreeAddress::{AddSub, AddSubWords};
        let lea = |got: &LirBody| names_of(got).contains(&"lea".to_owned());
        assert!(lea(&tied(&of(live_source_add_of(2, "add", 2)), AddSubWords)));
        assert!(!lea(&tied(&of(live_source_add_of(2, "add", 2)), AddSub)));
        // A unit step keeps its INC (the regparm3 `_incs` listing at -Os).
        for (step, name) in [(1, "add"), (1, "sub"), (-1, "add")] {
            assert!(!lea(&tied(&of(live_source_add_of(step, name, 2)), AddSubWords)), "{name} {step}");
        }
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

    /// `rescheduleKillAboveMI`: an add with a memory operand cannot move down
    /// (a load stays), so the `lea` that kills its source goes up above it.
    #[test]
    fn test_the_reader_that_kills_the_source_of_a_tied_add_with_a_load_goes_above_it() {
        use crate::model::ir::Mem;
        let what = Semantics {
            name: Some("add".into()),
            dests: vec![held(8, 4)],
            sources: vec![held(7, 4), Loc::Mem(Mem::new(None, 4))],
            ..Semantics::new(Operation::Binary)
        };
        let add = Arc::new(Insn::new(1, Some((1, 1)), Some(what), vec![8], vec![7]));
        let insns = vec![imm_def(0, 7), add, lea_of(2, 5, 7), read(3, 8), read(4, 5)];
        let got = tied(&of(insns), Off);
        assert!(position(&got, "lea") < position(&got, "add"), "{:?}", names_of(&got));
    }

    /// The copy of a tied add's result goes below the kill with it: the
    /// intervals are moved the copy first, below what it reads. The add
    /// moved first was a def moved below its own reader, and `handleMove`
    /// asserted on a segment that ends before the new place ("Must have
    /// following segment": 7 of QCport's modules did not compile at -Os).
    #[test]
    fn test_a_tied_add_with_a_copy_of_its_result_goes_below_the_kill_with_it() {
        let add = live_source_add(5, "add")[1].clone();
        let insns = vec![imm_def(0, 7), add, copy_insn(2, 9, 8), lea_of(3, 5, 7), read(4, 9), read(5, 5)];
        let got = tied(&of(insns), Off);
        let names = names_of(&got);
        assert!(position(&got, "lea") < position(&got, "add"), "{names:?}");
    }

    /// Finding the kill of a tied add's source walked the block from its end
    /// back to the last reader (`(at + 1..len).rev().find`), for every tied
    /// add: quadratic in a long straight-line block (jpeg-sized: 118 M of
    /// 615 M instructions at 64 statements, 1.9 G of 5.6 G at 256). The pin
    /// reads the kill from `LiveVariables`; the search is now the
    /// instructions the reschedule may move across.
    #[test]
    fn test_finding_the_kill_does_not_walk_the_rest_of_the_block() {
        use crate::twoaddress::KILL_SEARCHED;
        let searched = |tail: i64| {
            let mut insns = Vec::new();
            for p in 0..20_i64 {
                let (base, r) = (p * 5, 1000 + p as u32 * 10);
                let what = Semantics {
                    name: Some("add".into()),
                    dests: vec![held(r + 1, 4)],
                    sources: vec![held(r, 4), Loc::Mem(crate::model::ir::Mem::new(None, 4))],
                    ..Semantics::new(Operation::Binary)
                };
                insns.push(imm_def(base, r));
                insns.push(Arc::new(Insn::new(base + 1, Some((base + 1, base + 1)), Some(what), vec![r + 1], vec![r])));
                insns.push(lea_of(base + 2, r + 2, r));
                insns.push(read(base + 3, r + 1));
                insns.push(read(base + 4, r + 2));
            }
            insns.extend((0..tail).map(|k| imm_def(1000 + k, 9000 + k as u32)));
            KILL_SEARCHED.with(|n| n.set(0));
            let _ = tied(&of(insns), Off);
            KILL_SEARCHED.with(std::cell::Cell::get)
        };
        let (short, long) = (searched(50), searched(2000));
        assert!(short > 0, "the premise: the reschedules search for a kill");
        assert!(long <= 2 * short, "{long} instructions looked at behind a 2000-instruction tail, {short} behind 50");
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

    /// `s.a + s.c + a`: the sum `t1` then `t1 + a` with `a` loaded just before.
    /// The pin folds the load into the add (`addl 16(%esp), %eax`); tying
    /// the loaded value instead gave `mov eax, [a]; add eax, ecx` and `push
    /// ecx` (c32/regs.c `by_value`: 5 instructions against 3).
    #[test]
    fn test_a_value_a_load_defines_is_not_the_operand_that_is_tied() {
        use crate::model::ir::Mem;
        let load = {
            let what = Semantics {
                name: Some("mov".into()),
                dests: vec![held(3, 4)],
                sources: vec![Loc::Mem(Mem::new(None, 4))],
                ..Semantics::new(Operation::Move)
            };
            Arc::new(Insn::new(2, Some((2, 2)), Some(what), vec![3], vec![]))
        };
        // v1, v2 defined; t1 = v1 + v2 (v8 here: `sum_into` into 8); v3 loaded
        // after it; v9 = t1 + v3.
        let insns = vec![imm_def(0, 1), imm_def(1, 2), sum_into(2, 8, 1, 2), load, sum_into(4, 9, 8, 3), read(5, 9)];
        assert!(commuted(insns, 4).is_none());
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
        // a body the intervals can be made of: what is read is defined
        flags_live.insert(0, imm_def(5, 6));
        assert!(!names_of(&tied(&of(flags_live), All)).contains(&"lea".to_owned()));
    }

    /// `add; inc; jb`: `inc` leaves CF alone, so `jb` reads the add's carry.
    /// The add went to an `lea` (no flags) because `inc` "wrote the flags",
    /// and `jb` read the carry of whatever came before.
    #[test]
    fn test_an_add_whose_carry_a_later_inc_leaves_for_a_jb_stays_an_add() {
        let step = |at: i64, name: &str, dest: u32, source: u32| {
            let what = Semantics {
                name: Some(name.into()),
                dests: vec![held(dest, 4)],
                sources: vec![held(source, 4)],
                ..Semantics::new(Operation::Unary)
            };
            Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![dest], vec![source]))
        };
        let jb = Arc::new(Insn::new(
            3,
            Some((3, 3)),
            Some(Semantics { name: Some("jb".into()), target: Some(0), ..Semantics::new(Operation::Branch) }),
            vec![],
            vec![],
        ));
        let insns = vec![
            imm_def(0, 7),
            live_source_add(-1, "add")[1].clone(),
            step(2, "inc", 9, 8),
            jb,
            read(4, 7),
            read(5, 9),
        ];
        let names = names_of(&tied(&of(insns), All));
        assert!(!names.contains(&"lea".to_owned()), "the add became an lea: {names:?}");
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

    /// x_iir (AM-BT-22): `imul` by a constant was split into a copy and a tied
    /// `imul`, one more copy per product than the pin's three-address
    /// `IMUL32rri` (17 loop instructions against its 15).
    #[test]
    fn test_two_address_leaves_an_imul_by_a_constant_three_address() {
        let (result, source) = (held(900, 4), held(901, 4));
        let what = Semantics {
            name: Some("imul".into()),
            dests: vec![result],
            sources: vec![source, imm(1600, 4)],
            ..Semantics::new(Operation::Multiply)
        };
        let insn = Insn::new(0, Some((0, 3)), Some(what), vec![900], vec![901]);
        assert!(fixed(&insn).is_none(), "split into a copy and a tied multiply");
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

    /// The pin's TwoAddress asks `LiveVariables`, not intervals: ours built a
    /// `LiveIntervals` per function for its kill questions (2.97 M Ir of
    /// d_faces' 11.73 M in TwoAddress); now it builds none, and walks the body
    /// for no liveness rows.
    #[test]
    fn test_two_address_builds_no_live_intervals_and_walks_the_body_for_no_liveness_rows() {
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
            got.iter_insns().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect();
        assert_eq!(names, ["mov", "mov", "add", "mov"], "the premise: the update in place was found: {names:?}");
        assert_eq!(crate::backend::allocate::live_rows_walks(&input) - before, 0);
        let built = crate::liveness::liveintervals::BUILT.with(std::cell::Cell::get);
        let _ = tied(&input, Default::default());
        assert_eq!(crate::liveness::liveintervals::BUILT.with(std::cell::Cell::get) - built, 0);
    }

    /// `_reused` entered every name of every instruction of a block into its
    /// places (2,862 inserts on d_faces, 1.19 M Ir) before asking whether any
    /// instruction could be an update in place; a block whose values are read
    /// more than once, or defined twice, has none to ask about.
    #[test]
    fn test_a_block_with_no_candidate_update_in_place_enters_no_places() {
        let sum = |at: i64, into: u32, first: u32, second: u32| {
            let what = Semantics {
                name: Some("add".into()),
                dests: vec![held(into, 4)],
                sources: vec![held(first, 4), held(second, 4)],
                ..Semantics::new(Operation::Binary)
            };
            Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![into], vec![first, second]))
        };
        // Every sum is read twice or not at all: none is `c := a + b; a := c`.
        let body = LirBody::new(
            "t",
            0,
            vec![LirBlock::new(0, vec![sum(0, 3, 1, 2), sum(1, 4, 3, 3), sum(2, 5, 4, 4)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        super::ADDED.with(|count| count.set(0));
        let _ = super::_reused(&body);
        assert_eq!(super::ADDED.with(std::cell::Cell::get), 0);
    }

    /// `_reused` kept where each value is read in a `BTreeSet` per value (2,862
    /// inserts and their node allocations on d_faces); the sorted places answer
    /// the same asks: the first place, a range, and a place moved.
    #[test]
    fn test_the_places_a_value_is_named_answer_a_range_and_follow_a_move() {
        let mut places = super::Places::default();
        for at in [5, 1, 3, 3, 9] {
            places.add(7, at);
        }
        assert_eq!(places.at(7), [1, 3, 5, 9], "ascending, once each");
        assert_eq!(places.in_range(7, 2, usize::MAX), [3, 5, 9]);
        assert_eq!(places.in_range(7, 4, 9), [5], "up to, not including, the end");
        places.remove(7, 3);
        places.add(7, 4);
        assert_eq!(places.at(7), [1, 4, 5, 9]);
        assert!(places.in_range(8, 0, usize::MAX).is_empty(), "a value never named");
    }

    /// The walk of one block of `insns`, every instruction passed and its
    /// copies read (`processCopy`), then asked.
    fn walked(insns: Vec<Arc<Insn>>) -> (LirBody, Vec<(u32, Option<RegId>, Option<RegId>)>) {
        let body = LirBody::new("t", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        let facts = crate::twoaddress::BodyFacts::of(&body);
        let mut walk = Commuter::new(&facts, 0);
        for one in body.blocks[0].insns.iter() {
            walk.pass(one);
            walk.process_copy(one);
            walk.remove_clobbered_src_reg_map(one);
        }
        let asked = (1..=6).map(|value| (value, walk.src_register(value), walk.dst_register(value))).collect();
        (body, asked)
    }

    fn arrival(
        at: i64,
        arrives: &[(u32, RegId)],
    ) -> Arc<Insn> {
        let what = Semantics { name: Some("".into()), ..Semantics::new(Operation::Nothing) };
        let mut one =
            Insn::new(at, Some((at, at)), Some(what), arrives.iter().map(|(value, _)| *value).collect(), vec![]);
        one.delivers = arrives.iter().map(|(value, register)| (Held { value: *value, width: 4 }, *register)).collect();
        Arc::new(one)
    }

    fn returning(
        at: i64,
        value: u32,
        register: RegId,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some("".into()), ..Semantics::new(Operation::Nothing) };
        let mut one = Insn::new(at, Some((at, at)), Some(what), vec![], vec![value]);
        one.requires = vec![(Held { value, width: 4 }, register)];
        Arc::new(one)
    }

    /// The pin's own example of `processCopy`: `v1 = mov r0; v2 = mov r1; v3 =
    /// add v1, v2; r1 = mov v3`: v2 is mapped to r1, and v3 is potentially
    /// joined with r1 on the output side.
    #[test]
    fn test_the_maps_follow_a_sum_from_the_registers_it_arrives_in_to_the_one_it_ends_in() {
        let insns =
            vec![arrival(0, &[(1, RegId::EAX), (2, RegId::EDX)]), sum_into(1, 3, 1, 2), returning(2, 3, RegId::EDX)];
        let (_, asked) = walked(insns);
        let src = |value: u32| asked.iter().find(|one| one.0 == value).and_then(|one| one.1);
        let dst = |value: u32| asked.iter().find(|one| one.0 == value).and_then(|one| one.2);
        assert_eq!((src(1), src(2)), (Some(RegId::EAX), Some(RegId::EDX)), "{asked:?}");
        // `SrcRegMap[v3]` is set by the scan of each source; the last, v2's,
        // stays.
        assert_eq!(src(3), Some(RegId::EDX), "{asked:?}");
        assert_eq!((dst(3), dst(1)), (Some(RegId::EDX), Some(RegId::EDX)), "{asked:?}");
    }

    /// A copy to a register is in `DstRegMap` once the walk has passed it, not
    /// before: the maps were the whole body's, so an add ahead of the `ret`
    /// knew where its sum ends.
    #[test]
    fn test_a_value_returned_in_a_register_is_mapped_only_once_the_walk_reaches_it() {
        let body = LirBody::new(
            "t",
            0,
            vec![LirBlock::new(0, vec![imm_def(0, 1), sum_into(1, 2, 1, 1), returning(2, 2, RegId::EAX)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let facts = crate::twoaddress::BodyFacts::of(&body);
        let mut walk = Commuter::new(&facts, 0);
        let mut before = None;
        for one in body.blocks[0].insns.iter() {
            walk.pass(one);
            walk.process_copy(one);
            if one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some("add")) {
                before = Some(walk.dst_register(2));
            }
        }
        assert_eq!((before, walk.dst_register(2)), (Some(None), Some(RegId::EAX)));
    }

    /// `removeClobberedSrcRegMap`: after a call that writes EAX, a value that
    /// arrived in EAX does not arrive there any more.
    #[test]
    fn test_a_register_a_call_clobbers_holds_no_arrival_after_it() {
        let what = Semantics { name: Some("call".into()), ..Semantics::new(Operation::Call) };
        let mut call = Insn::new(1, Some((1, 1)), Some(what), vec![], vec![]);
        call.clobbers = [RegId::EAX].into_iter().collect();
        let (_, asked) = walked(vec![arrival(0, &[(1, RegId::EAX), (2, RegId::EDX)]), Arc::new(call)]);
        assert_eq!((asked[0].1, asked[1].1), (None, Some(RegId::EDX)), "{asked:?}");
    }

    /// A tied add whose first source is read as undef needs no copy: its
    /// operand is the destination, read as undef (`collectTiedOperands`);
    /// the copy of an undef gave the allocator a `mov` of nothing.
    #[test]
    fn test_a_tied_add_of_an_undef_source_is_made_with_no_copy() {
        let what = Semantics {
            name: Some("add".into()),
            dests: vec![held(8, 4)],
            sources: vec![held(7, 4), Loc::Imm(Imm { value: 5, width: 4, address: None })],
            ..Semantics::new(Operation::Binary)
        };
        let mut add = Insn::new(1, Some((1, 1)), Some(what), vec![8], vec![]);
        add.undefs = vec![7];
        let made = fixed(&add).expect("rewritten");
        assert_eq!(made.len(), 1, "no copy: {made:?}");
        let sources = &made[0].what.as_ref().expect("an add").sources;
        assert_eq!(sources[0], held(8, 4));
        assert_eq!((made[0].undefs.clone(), made[0].uses.clone()), (vec![8], vec![]));
    }

    /// `processTiedPairs`: the copy made before a tied instruction takes the
    /// distance the instruction had and the instruction the next one
    /// (`DistanceMap`), so the distances that follow count the copies the
    /// pin counts.
    #[test]
    fn test_a_copy_made_for_a_tied_pair_takes_a_distance_of_its_own() {
        let body = LirBody::new(
            "t",
            0,
            vec![LirBlock::new(0, vec![imm_def(0, 7), sum_into(1, 8, 7, 7), read(2, 8)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let facts = crate::twoaddress::BodyFacts::of(&body);
        let mut walk = Commuter::new(&facts, 0);
        let insns = &body.blocks[0].insns;
        walk.pass(&insns[0]);
        let at = walk.pass(&insns[1]);
        let (copy, mi) = (copy_insn(1, 8, 7), sum_into(1, 8, 8, 7));
        walk.tied_pair_processed(copy.id(), mi.id(), 8, 7, false);
        assert_eq!((walk.distance_of(copy.id()), walk.distance_of(mi.id())), (Some(at), Some(at + 1)));
        assert_eq!(walk.pass(&insns[2]), at + 2, "the next instruction is two after, the copy counted");
    }

    /// `SrcRegMap[LastCopiedReg] = RegB`: where the source died at the tied
    /// instruction, the copy's destination arrives as the source did.
    #[test]
    fn test_a_tied_copy_of_a_dying_source_arrives_as_the_source_did() {
        let body = LirBody::new(
            "t",
            0,
            vec![LirBlock::new(0, vec![arrival(0, &[(7, RegId::EAX)]), sum_into(1, 8, 7, 7)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let facts = crate::twoaddress::BodyFacts::of(&body);
        let mut walk = Commuter::new(&facts, 0);
        walk.pass(&body.blocks[0].insns[0]);
        walk.process_copy(&body.blocks[0].insns[0]);
        walk.pass(&body.blocks[0].insns[1]);
        // `v9` is a copy's destination the scan of `v7`'s uses did not reach.
        let (copy, mi) = (copy_insn(1, 9, 7), sum_into(1, 9, 9, 7));
        walk.tied_pair_processed(copy.id(), mi.id(), 9, 7, false);
        assert_eq!(walk.src_register(9), None, "a source that lives on maps nothing");
        walk.tied_pair_processed(copy.id(), mi.id(), 9, 7, true);
        assert_eq!(walk.src_register(9), Some(RegId::EAX));
    }

    /// `tryInstructionTransform`'s unfold: `d := add a, [m]` where `a` is read
    /// again takes the load out and commutes, so the loaded value is the
    /// tied one: `mov t,[m]; mov d,t; add d,a` (the copy joins the load),
    /// not `mov d,a; add d,[m]`, which holds `a` and `d` apart.
    #[test]
    fn test_a_folded_load_beside_a_live_source_is_unfolded_and_commuted() {
        use crate::model::ir::Mem;
        let what = Semantics {
            name: Some("add".into()),
            dests: vec![held(8, 4)],
            sources: vec![held(7, 4), Loc::Mem(Mem::new(None, 4))],
            ..Semantics::new(Operation::Binary)
        };
        let add = Arc::new(Insn::new(1, Some((1, 1)), Some(what), vec![8], vec![7]));
        let got = tied(&of(vec![imm_def(0, 7), add, read(2, 8), read(3, 7)]), Off);
        let mut shapes = Vec::new();
        for one in got.iter_insns() {
            let Some(what) = one.what.as_ref() else { continue };
            shapes.push((what.name.clone().unwrap_or_default().to_string(), what.sources.clone()));
        }
        let mem = |at: usize| matches!(shapes[at].1.first(), Some(Loc::Mem(_)));
        assert!(
            shapes.len() == 4 && shapes[1].0 == "mov" && mem(1) && shapes[2].0 == "mov" && shapes[3].0 == "add",
            "the load comes first, then the copy of what it loaded, then the add of `a`: {shapes:?}"
        );
    }

    /// The unfold is for every commutative two-address instruction with a
    /// folded load, not only `add`/`and`/`or`/`xor`: `imul esi,[edx+4]` beside
    /// a live counter was a copy and a memory multiply, where the pin loads
    /// first and multiplies registers (x_list's inner loop, `mov esi,ecx;
    /// imul esi,[edx+4]` against `mov edi,[edx+4]; imul edi,ecx`).
    #[test]
    fn test_a_folded_multiply_beside_a_live_source_is_unfolded_and_commuted() {
        use crate::model::ir::Mem;
        let what = Semantics {
            name: Some("imul".into()),
            dests: vec![held(8, 4)],
            sources: vec![held(7, 4), Loc::Mem(Mem::new(None, 4))],
            ..Semantics::new(Operation::Multiply)
        };
        let mul = Arc::new(Insn::new(1, Some((1, 1)), Some(what), vec![8], vec![7]));
        let got = tied(&of(vec![imm_def(0, 7), mul, read(2, 8), read(3, 7)]), Off);
        let mut shapes = Vec::new();
        for one in got.iter_insns() {
            let Some(what) = one.what.as_ref() else { continue };
            shapes.push((what.name.clone().unwrap_or_default().to_string(), what.sources.clone()));
        }
        let mem = |at: usize| matches!(shapes[at].1.first(), Some(Loc::Mem(_)));
        assert!(
            shapes.len() == 4 && shapes[1].0 == "mov" && mem(1) && shapes[2].0 == "mov" && shapes[3].0 == "imul",
            "the load comes first, then the copy of what it loaded, then the imul by `a`: {shapes:?}"
        );
    }

    /// `convertInstTo3Addr`: the `lea` has the place the add had, and the maps
    /// forget the add's result as a source and its source as a destination.
    #[test]
    fn test_a_converted_instruction_keeps_its_distance_and_clears_its_map_entries() {
        let body = LirBody::new(
            "t",
            0,
            vec![LirBlock::new(0, vec![arrival(0, &[(7, RegId::EAX)]), sum_into(1, 8, 7, 7), read(2, 8)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let facts = crate::twoaddress::BodyFacts::of(&body);
        let mut walk = Commuter::new(&facts, 0);
        walk.pass(&body.blocks[0].insns[0]);
        walk.process_copy(&body.blocks[0].insns[0]);
        let at = walk.pass(&body.blocks[0].insns[1]);
        assert_eq!(walk.src_register(8), Some(RegId::EAX), "premise: the sum arrives as v7 did");
        let three = sum_into(1, 8, 7, 7);
        walk.converted_to_three_address(body.blocks[0].insns[1].id(), three.id(), 8, 7);
        assert_eq!(walk.distance_of(three.id()), Some(at));
        assert_eq!(walk.src_register(8), None, "SrcRegMap[regA] is erased");
    }
}
