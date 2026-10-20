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
//! - Commuting: for `add`, `and`, `or`, `xor` and `imul` the sources are
//!   swapped when that lets the destination reuse a source that dies. A swap is
//!   chosen when the second source is the destination already, when only the
//!   second source dies, or when neither lives on and the second is less
//!   blocked (by interference with the destination's copy neighbours) and
//!   closer by copies. Interference is computed only among the values a swap
//!   asks about. Before those, a sum that ends in a register (`requires`) keeps
//!   the operand delivered in it as the tied one (LLVM's physical-register rule
//!   in `isProfitableToCommute`), so `a + b` returned in EAX is `add eax, edx`.
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
//! [`TwoAddressInstructionPass`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp).

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::analysis::intervals as ranges;
use crate::backend::allocate::LiveAt;
use crate::backend::{allocate, coalesce, spiller};
use crate::model::ir::{self, Held, Loc, Mem, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{IndexMap, IndexSet};

/// Operations that read their destination.
pub const _TIED: [Operation; 3] = [Operation::Binary, Operation::Unary, Operation::Funnel];

pub struct TwoAddress;

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
        Ok(tied(&body))
    }
}

/// `body` with every tied instruction reading what it writes.
pub fn tied(body: &LirBody) -> LirBody {
    let reused = _reused(body);
    let body = &reused;
    let mut changed = false;
    let mut counter = spiller::_next_value(body);
    let mut mint = || {
        counter += 1;
        counter - 1
    };

    // What is live at each block's end is read from the rows of bits and what
    // the block's own instructions change laid over them: a set of every live
    // value copied for each block was quadratic in the values live at once.
    let rows = allocate::live_rows(body);
    let copies = _copy_destinations(body);
    let fixed = _fixed(body);
    // Of each swap's two sources, those live after it: all `_commuted` reads of
    // liveness.
    let lives: Vec<IndexMap<usize, BTreeSet<u32>>> = body
        .blocks
        .iter()
        .map(|block| {
            let mut changed: crate::support::hash::HashMap<u32, bool> = Default::default();
            let alive = |changed: &crate::support::hash::HashMap<u32, bool>, value: u32| {
                changed.get(&value).copied().unwrap_or_else(|| rows.live_out(block.at, value))
            };
            let mut live_after: IndexMap<usize, BTreeSet<u32>> = IndexMap::default();
            for one in block.insns.iter().rev() {
                if let Some((_, first, second)) = _swappable(one) {
                    live_after.insert(
                        ranges::key(one),
                        [first, second].into_iter().filter(|value| alive(&changed, *value)).collect(),
                    );
                }
                for value in &one.defines {
                    changed.insert(*value, false);
                }
                for value in &one.uses {
                    changed.insert(*value, true);
                }
            }
            live_after
        })
        .collect();
    // `_commuted` compares a swap's two sources with the copy neighbours of its
    // destination, when neither source lives on: the interference among those
    // values is all it asks of, not every pair live together (quadratic in the
    // values live at once), and none where no swap asks.
    let asked: BTreeSet<u32> = body
        .blocks
        .iter()
        .zip(&lives)
        .flat_map(|(block, live_after)| block.insns.iter().map(move |one| (one, live_after.get(&ranges::key(one)))))
        .filter(|(_, alive)| alive.is_some_and(BTreeSet::is_empty))
        .filter_map(|(one, _)| _swappable(one))
        .filter_map(|(into, first, second)| {
            let neighbours = copies.get(&into)?;
            Some(neighbours.iter().copied().chain([into, first, second]))
        })
        .flatten()
        .collect();
    let interference =
        if asked.is_empty() { IndexMap::default() } else { coalesce::_interference_among(body, Some(&asked)) };
    if llrm_support::env_flag!("LLRM_CHECK_TWOADDR") {
        let whole = coalesce::_interference(body);
        for &one in &asked {
            for &other in &asked {
                assert_eq!(
                    interference.get(&one).is_some_and(|found| found.contains(&other)),
                    whole.get(&one).is_some_and(|found| found.contains(&other)),
                    "LLRM_CHECK_TWOADDR: values {one} and {other} interfere differently among the asked values than in the whole graph"
                );
            }
        }
    }
    let mut blocks = Vec::new();
    for (block, live_after) in body.blocks.iter().zip(&lives) {
        let nothing = BTreeSet::new();
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        for one in &block.insns {
            let mut one = Arc::clone(one);
            if let Some(chosen) = _commuted(
                &one,
                live_after.get(&ranges::key(&one)).unwrap_or(&nothing),
                Some(&copies),
                Some(&interference),
                &fixed,
            ) {
                changed = true;
                one = chosen;
            }
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
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        for value in &one.defines {
            *defined.entry(*value).or_insert(0) += 1;
        }
    }
    let rows = allocate::live_rows(body);
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
            if defined.get(&into.value) != Some(&1)
                || rows.live_out(block.at, into.value)
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

/// Which values each value is copied to or from.
fn _copy_destinations(body: &LirBody) -> IndexMap<u32, BTreeSet<u32>> {
    let mut adjacent: IndexMap<u32, BTreeSet<u32>> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            let Some(what) = &one.what else {
                continue;
            };
            if what.op == Operation::Move {
                if let ([Loc::Held(dest)], [Loc::Held(source)]) = (what.dests.as_slice(), what.sources.as_slice()) {
                    if dest.width == source.width {
                        adjacent.entry(source.value).or_default().insert(dest.value);
                        adjacent.entry(dest.value).or_default().insert(source.value);
                    }
                }
            }
        }
    }
    adjacent
}

/// How many copies apart two values are; infinite where none joins them within
/// `limit`.
fn _distance(
    copies: &IndexMap<u32, BTreeSet<u32>>,
    start: u32,
    goal: u32,
    limit: i64,
) -> f64 {
    let mut seen: BTreeSet<u32> = BTreeSet::from([start]);
    let mut frontier: BTreeSet<u32> = BTreeSet::from([start]);
    let mut steps = 0;
    while !frontier.is_empty() && steps <= limit {
        if frontier.contains(&goal) {
            return steps as f64;
        }
        frontier = frontier
            .iter()
            .flat_map(|one| copies.get(one).into_iter().flatten().copied())
            .filter(|other| !seen.contains(other))
            .collect();
        seen.extend(frontier.iter().copied());
        steps += 1;
    }
    f64::INFINITY
}

/// The values a commutative instruction writes and reads, where `_commuted`
/// may swap its sources.
fn _swappable(one: &Insn) -> Option<(u32, u32, u32)> {
    _swappable_held(one).map(|(into, first, second)| (into.value, first.value, second.value))
}

fn _swappable_held(one: &Insn) -> Option<(&Held, &Held, &Held)> {
    let what = one.what.as_ref()?;
    let commutative = (what.op == Operation::Binary
        && matches!(what.name.as_deref(), Some("add" | "and" | "or" | "xor")))
        || (what.op == Operation::Multiply && what.name.as_deref() == Some("imul"));
    if !commutative
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
struct Fixed {
    from: IndexMap<u32, RegId>,
    to: IndexMap<u32, RegId>,
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

/// The instruction with its commutative sources swapped, or None where it
/// is returned as it was.
fn _commuted(
    one: &Insn,
    alive: &BTreeSet<u32>,
    copies: Option<&IndexMap<u32, BTreeSet<u32>>>,
    interference: Option<&coalesce::Graph>,
    fixed: &Fixed,
) -> Option<Arc<Insn>> {
    let what = one.what.as_ref()?;
    let (into, first, second) = _swappable_held(one)?;
    let no_copies = IndexMap::default();
    let copies = copies.unwrap_or(&no_copies);
    let empty = BTreeSet::new();
    let affinities = copies.get(&into.value).unwrap_or(&empty);

    // The copy into the destination of a tied source that interferes with it
    // stays, so that source counts as blocked too.
    let blocked = |source: u32| -> usize {
        let found = interference.and_then(|graph| graph.get(&source));
        affinities.iter().filter(|other| found.is_some_and(|found| found.contains(other))).count()
            + usize::from(found.is_some_and(|found| found.contains(&into.value)))
    };

    // Where the sum ends in a register and one operand arrives in it, that
    // one is the tied operand (`isProfitableToCommute`, the physical-register
    // rule): the other dying here is all the swap needs.
    if second.value != into.value
        && !alive.contains(&second.value)
        && let Some(to) = fixed.to.get(&into.value)
    {
        let fits = |value: u32| fixed.from.get(&value).map(|register| ir::root(*register) == ir::root(*to));
        let (from_first, from_second) = (fits(first.value), fits(second.value));
        let swap = matches!(
            (from_first, from_second),
            (None, Some(true)) | (Some(false), None | Some(true))
        );
        let keep = matches!(
            (from_second, from_first),
            (None, Some(true)) | (Some(false), None | Some(true))
        );
        if swap || keep {
            if !swap {
                return None;
            }
            let mut made = one.clone();
            made.what = Some(Semantics { sources: vec![Loc::Held(*second), Loc::Held(*first)], ..what.clone() });
            return Some(Arc::new(made));
        }
    }
    let reusable = !alive.contains(&first.value)
        && !alive.contains(&second.value)
        && _less(
            (blocked(second.value), _distance(copies, into.value, second.value, 8)),
            (blocked(first.value), _distance(copies, into.value, first.value, 8)),
        );
    if second.value == into.value || alive.contains(&first.value) && !alive.contains(&second.value) || reusable {
        let mut made = one.clone();
        made.what = Some(Semantics { sources: vec![Loc::Held(*second), Loc::Held(*first)], ..what.clone() });
        return Some(Arc::new(made));
    }
    None
}

/// Python's tuple `<` over `(int, float)`.
fn _less(
    one: (usize, f64),
    other: (usize, f64),
) -> bool {
    one.0 < other.0 || (one.0 == other.0 && one.1 < other.1)
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
    Semantics {
        name: Some("mov".to_owned()),
        dests: vec![into],
        sources: vec![out_of],
        ..Semantics::new(Operation::Move)
    }
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
    let movement =
        _inserted(one, _move(Loc::Held(*into), first.clone()), vec![into.value], first_value.into_iter().collect());
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

    use super::{_commuted, _untied, Fixed, tied};
    use crate::model::ir::{Held, Imm, Loc, Operation, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn held(
        value: u32,
        width: u32,
    ) -> Loc {
        Loc::Held(Held { value, width })
    }

    fn addition(name: &str) -> Insn {
        let what = Semantics {
            name: Some(name.to_owned()),
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

    fn graph(pairs: &[(u32, &[u32])]) -> IndexMap<u32, BTreeSet<u32>> {
        pairs.iter().map(|(value, others)| (*value, others.iter().copied().collect())).collect()
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
                name: Some("add".to_owned()),
                dests: vec![held(into, 4)],
                sources: vec![held(first, 4), held(second, 4)],
                ..Semantics::new(Operation::Binary)
            };
            Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![into], vec![first, second]))
        };
        let nothing = |at: i64| {
            let what = Semantics { name: Some(String::new()), ..Semantics::new(Operation::Nothing) };
            Insn::new(at, Some((at, at)), Some(what), vec![], vec![])
        };
        let mut arrival = nothing(0);
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
        let got = tied(&input);
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
            let chosen = _commuted(&one, &BTreeSet::from([1, 3]), None, None, &Fixed::default()).expect(name);
            assert_eq!(sources(&chosen), reversed(&one), "{name}");
            assert!(chosen.uses == one.uses && chosen.defines == one.defines, "{name}");
            assert_eq!(chosen.covers, one.covers, "{name}");
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
        let mut one = addition("imul");
        one.what.as_mut().unwrap().op = Operation::Multiply;

        let chosen = _commuted(&one, &BTreeSet::from([1, 3]), None, None, &Fixed::default()).expect("swapped");

        assert_eq!(sources(&chosen), reversed(&one));
    }

    /// Nib fixed multiply shifted its multiplier instead of its product.
    #[test]
    fn test_funnel_shift_copies_its_low_source_into_the_destructive_destination() {
        let what = Semantics {
            name: Some("shrd".to_owned()),
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
            Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) })
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

        let got = tied(&input);

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
            Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) })
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

        let got = tied(&input);

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
            let one = addition(name);
            assert!(_commuted(&one, &BTreeSet::from([1, 3]), None, None, &Fixed::default()).is_none(), "{name}");
        }
    }

    #[test]
    fn test_live_operands_and_grouped_operations_keep_their_order() {
        let one = addition("add");
        assert!(_commuted(&one, &BTreeSet::from([1, 2, 3]), None, None, &Fixed::default()).is_none());
        let grouped = Insn { group: Some(1), ..one };
        assert!(_commuted(&grouped, &BTreeSet::from([1, 3]), None, None, &Fixed::default()).is_none());
    }

    /// LOCALP's backedge copy favors its accumulator only when its old value
    /// can be overwritten.
    #[test]
    fn test_result_copy_affinity_does_not_override_liveness() {
        for (alive, swapped) in [(vec![3], true), (vec![2, 3], false), (vec![1, 2, 3], false)] {
            let one = addition("add");
            let copies = graph(&[(3, &[2])]);
            let chosen = _commuted(&one, &alive.iter().copied().collect(), Some(&copies), None, &Fixed::default());
            let got = chosen.map_or_else(|| sources(&one), |chosen| sources(&chosen));
            assert_eq!(got, if swapped { reversed(&one) } else { sources(&one) }, "{alive:?}");
        }
    }

    /// CRC32 tied XOR to its shifted temporary, then copied the result around
    /// the backedge.
    #[test]
    fn test_crc32_ties_the_operand_that_can_join_its_loop_phi() {
        let one = addition("xor");
        let copies = graph(&[(3, &[4]), (4, &[3])]);
        let interference: crate::backend::coalesce::Graph = graph(&[(1, &[4]), (4, &[1])])
            .into_iter()
            .map(|(value, near)| (value, near.into_iter().collect()))
            .collect();

        let chosen = _commuted(&one, &BTreeSet::from([3]), Some(&copies), Some(&interference), &Fixed::default())
            .expect("swapped");

        assert_eq!(sources(&chosen), reversed(&one));
    }

    /// CRC's `(x >> 1) ^ t` tied the shifted copy, which interferes with the
    /// loop phi it joins, and copied the result into place every iteration.
    #[test]
    fn test_a_source_that_interferes_with_the_destination_is_not_the_one_tied() {
        let one = addition("xor");
        let copies = graph(&[(3, &[1, 2])]);
        let interference: crate::backend::coalesce::Graph = graph(&[(1, &[2, 3]), (2, &[1]), (3, &[1])])
            .into_iter()
            .map(|(value, near)| (value, near.into_iter().collect()))
            .collect();

        let chosen =
            _commuted(&one, &BTreeSet::new(), Some(&copies), Some(&interference), &Fixed::default()).expect("swapped");

        assert_eq!(sources(&chosen), reversed(&one));
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
            name: Some("add".to_owned()),
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
            name: Some("imul".to_owned()),
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
            name: Some("add".to_owned()),
            dests: vec![result],
            sources: vec![source, imm(21, 2)],
            ..Semantics::new(Operation::Binary)
        };
        let insn = Insn::new(0, Some((0, 3)), Some(what), vec![900], vec![901]);
        let fixed = fixed(&insn).expect("untied");
        assert_eq!(fixed[1].uses, [900]);
    }
}
