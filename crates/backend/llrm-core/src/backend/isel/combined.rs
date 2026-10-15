//! Selections that need neighbouring instructions, made over each block once
//! every instruction is selected.
//!
//! # How it works
//! The patterns select one MIR instruction at a time. Some forms exist only
//! across instructions, and are cheaper chosen while values are still virtual:
//! after allocation the operands would sit in unrelated registers. In order,
//! over each block:
//!
//! 1. A far pointer's two word loads become one `les`-form load
//!    ([`crate::backend::farload`]).
//! 2. A comparison's one-use load becomes its memory operand
//!    ([`crate::backend::comparefold`]).
//! 3. A step of a compared value sets the compare's flags
//!    ([`crate::backend::stepflags`]).
//! 4. A load, update and store of one cell become one read-modify-write
//!    ([`crate::backend::rmw`]).
//! 5. The target's `peephole.peep` groups `memory_arguments` (a single-use load
//!    as a push's operand), `paired_pushes` and `immediate_arguments` (a
//!    constant as a push's operand) run over held values.
//! 6. A literal pushed several times is made again at each push, so no literal
//!    address is kept live across a call (`_rematerialized_arguments`).
//! 7. Two word loads of one dword's halves that are only pushed, high then low,
//!    become one dword load pushed whole (`dword_pairs`).
//!
//! ```text
//!  mov v1, [p]       ; low word            mov v3, dword [p]
//!  mov v2, [p+2]     ; high word    =>     push v3
//!  push v2
//!  push v1
//! ```
//!
//! Use counts are recomputed between steps, counting instruction reads, fixed
//! register inputs and phi inputs.
//!
//! # Example
//! The first three lines of the MIR below (a load, an add and a store of one
//! cell) leave `isel` as separate selections and step 4 makes them one:
//!
//! ```mir
//!   %3 = load i16, ptr addrspace(5) %1
//!   %4 = add i16 %3, 3
//!   store i16 %4, ptr addrspace(5) %1
//! ```
//!
//! ```lir
//!       0x0007  [[abs+0x0] v2@unplaced] := add [[abs+0x0] v2@unplaced], 0x3  ; inserted
//! ```
//!
//! No dump prints the body before this step, so the result is shown against the
//! MIR it came from.
//!
//! From `ISEL_DUMP=1` of a nib program (a struct method with a `while` loop,
//! called from `main`) at `-m16 -O2`, stage `FarIndirectCalls`.
//!
//! # Runs
//! Once per function, at the end of `Selector::body`, after unread wide halves
//! are dropped and before address promotion (`widen`).
//!
//! # Needs
//! The target's [`crate::backend::peep::Rules`] and the CPU profile.
//!
//! # See also
//! The phase map: [`crate::backend`].

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::Regs;

use crate::backend::cpu::Profile;
use crate::backend::peep::{self, walk::Facts};
use crate::backend::{comparefold, farload, rmw, stepflags};
use crate::model::ir::{self, Loc, Operation};
use crate::model::lir::{self, Block, Insn, LirBlock};
use crate::support::hash::IndexMap;

/// `blocks` with the selections made. No value of isel's is exposed: a
/// result leaves through a return's operands.
pub(super) fn combined(
    regs: Regs,
    bits: u32,
    blocks: Vec<Block>,
    cpu: &Profile,
    rules: &peep::Rules,
) -> Vec<Block> {
    let exposed = BTreeSet::new();
    let read_by_phis: BTreeSet<u32> = blocks
        .iter()
        .flat_map(|block| &block.phis)
        .flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value))
        .collect();
    let made: IndexMap<i64, Vec<Arc<Insn>>> = blocks.iter().map(|block| (block.at, block.insns.to_vec())).collect();
    // A far-pointer field is two language-visible word loads but one target
    // instruction.
    let selecting = farload::selectors(&made, &read_by_phis);
    let made: IndexMap<i64, Vec<Arc<Insn>>> =
        made.into_iter().map(|(at, insns)| (at, farload::selected(regs, &insns, &selecting))).collect();
    let uses = recount(&made, &blocks);
    // A one-use comparison load is a legal memory operand.
    let made: IndexMap<i64, Vec<Arc<Insn>>> =
        made.into_iter().map(|(at, insns)| (at, comparefold::selected(&insns, &uses, &exposed))).collect();
    let uses = recount(&made, &blocks);
    // A step of a compared value sets the compare's flags.
    let made: IndexMap<i64, Vec<Arc<Insn>>> =
        made.into_iter().map(|(at, insns)| (at, stepflags::selected(&insns))).collect();
    // x86 can express a C read-modify-write update in one memory operand.
    let made: IndexMap<i64, Vec<Arc<Insn>>> =
        made.into_iter().map(|(at, insns)| (at, rmw::selected(&insns, &uses))).collect();
    // The argument selections of peephole.peep, over held values.
    let facts = Facts::counted(&uses, bits, regs).with_cpu(cpu);
    let made: IndexMap<i64, Vec<Arc<Insn>>> = made
        .into_iter()
        .map(|(at, insns)| (at, peep::rewritten_insns(rules.memory_arguments, &insns, &facts)))
        .collect();
    let made: IndexMap<i64, Vec<Arc<Insn>>> =
        made.into_iter().map(|(at, insns)| (at, peep::rewritten_insns(rules.paired_pushes, &insns, &facts))).collect();
    let made: IndexMap<i64, Vec<Arc<Insn>>> = made
        .into_iter()
        .map(|(at, insns)| (at, peep::rewritten_insns(rules.immediate_arguments, &insns, &facts)))
        .collect();
    let made: IndexMap<i64, Vec<Arc<Insn>>> =
        made.into_iter().map(|(at, insns)| (at, _rematerialized_arguments(&insns, &uses, &exposed))).collect();
    let uses = recount(&made, &blocks);
    let mut made = dword_pairs(regs, made, &uses, &blocks);
    blocks
        .into_iter()
        .map(|block| LirBlock { insns: made.swap_remove(&block.at).expect("every block").into(), ..block.into_inner() })
        .collect()
}

/// How many times each value is read: by instructions, their fixed
/// inputs, and phis.
fn recount(
    made: &IndexMap<i64, Vec<Arc<Insn>>>,
    blocks: &[Block],
) -> IndexMap<u32, i64> {
    let mut uses: IndexMap<u32, i64> = IndexMap::default();
    for one in made.values().flatten() {
        for value in &one.uses {
            *uses.entry(*value).or_insert(0) += 1;
        }
    }
    for one in made.values().flatten() {
        for (held, _) in &one.requires {
            if !one.uses.contains(&held.value) {
                *uses.entry(held.value).or_insert(0) += 1;
            }
        }
    }
    for value in
        blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value))
    {
        *uses.entry(value).or_insert(0) += 1;
    }
    uses
}

/// Python's `Counter`: a missing key reads as zero.
fn count(
    counter: &IndexMap<u32, i64>,
    value: u32,
) -> i64 {
    counter.get(&value).copied().unwrap_or(0)
}

fn plain(one: &Insn) -> bool {
    !(!one.clobbers.is_empty() || !one.requires.is_empty() || !one.delivers.is_empty() || !one.spread.is_empty())
}

/// The value of a one-operand `push` of a held value.
fn pushed_value(what: &ir::Semantics) -> Option<(u32, u32)> {
    match (what.op, what.name.as_deref(), &what.dests[..], &what.sources[..]) {
        (Operation::Push, Some("push"), [], [Loc::Held(one)]) => Some((one.value, one.width)),
        _ => None,
    }
}

/// A `mov` of an immediate into a held value.
fn immediate_copy(what: &ir::Semantics) -> Option<(u32, u32, ir::Imm)> {
    match (what.op, what.name.as_deref(), &what.dests[..], &what.sources[..]) {
        (Operation::Move, Some("mov"), [Loc::Held(one)], [Loc::Imm(immediate)]) => {
            Some((one.value, one.width, immediate.clone()))
        }
        _ => None,
    }
}

/// A word load of a dword's low or high half, and the cell it reads.
fn word_load(
    regs: Regs,
    one: &Insn,
) -> Option<(u32, &ir::Mem)> {
    let what = one.what.as_ref()?;
    match (what.op, what.name.as_deref(), &what.dests[..], &what.sources[..]) {
        (Operation::Move, Some("mov"), [Loc::Held(dest)], [Loc::Mem(source)])
            if dest.width == 2
                && source.width == 2
                && one.defines == [dest.value]
                && !one.volatile
                && plain(one)
                && !regs.is_stack(source.through) =>
        {
            Some((dest.value, source))
        }
        _ => None,
    }
}

/// Two word loads of one dword's halves, every use of either a push of the
/// high then the low, as one dword load pushed whole: the old route's far
/// pointer, never taken apart, was one dword.
fn dword_pairs(
    regs: Regs,
    mut made: IndexMap<i64, Vec<Arc<Insn>>>,
    uses: &IndexMap<u32, i64>,
    blocks: &[Block],
) -> IndexMap<i64, Vec<Arc<Insn>>> {
    let loads: IndexMap<u32, (i64, usize, ir::Mem)> = made
        .iter()
        .flat_map(|(at, insns)| {
            insns
                .iter()
                .enumerate()
                .filter_map(
                    move |(index, one)| word_load(regs, one).map(|(value, cell)| (value, (*at, index, cell.clone()))),
                )
        })
        .collect();
    let pushed = |one: &Insn| {
        one.what
            .as_ref()
            .and_then(pushed_value)
            .filter(|&(_, width)| width == 2 && one.defines.is_empty() && plain(one))
            .map(|(value, _)| value)
    };
    // Each (high, low) pushed back to back, and how often.
    let mut pairs: IndexMap<(u32, u32), i64> = IndexMap::default();
    for insns in made.values() {
        for two in insns.windows(2) {
            if let (Some(high), Some(low)) = (pushed(&two[0]), pushed(&two[1])) {
                *pairs.entry((high, low)).or_insert(0) += 1;
            }
        }
    }
    let joined: Vec<(u32, u32)> = pairs
        .iter()
        .filter(|((high, low), times)| high != low && count(uses, *high) == **times && count(uses, *low) == **times)
        .filter(|((high, low), _)| {
            let (Some((high_at, high_index, high_cell)), Some((low_at, low_index, low_cell))) =
                (loads.get(high), loads.get(low))
            else {
                return false;
            };
            if high_at != low_at {
                return false;
            }
            // Loaded together, nothing between may change either half.
            let between = &made[high_at][(*high_index).min(*low_index) + 1..(*high_index).max(*low_index)];
            high_cell.word_above(low_cell)
                && (high_cell.through, high_cell.index_through) == (low_cell.through, low_cell.index_through)
                && between.iter().all(|one| {
                    one.what.as_ref().is_some_and(|what| {
                        !what.dests.iter().any(|dest| matches!(dest, Loc::Mem(_) | Loc::Reg(_)))
                            && !matches!(
                                what.op,
                                Operation::Barrier | Operation::Call | Operation::Return
                            )
                    }) && one.clobbers.is_empty()
                })
        })
        .map(|(pair, _)| *pair)
        .collect();
    // A half joins one pair.
    let mut taken: BTreeSet<u32> = BTreeSet::new();
    let joined: Vec<(u32, u32)> = joined
        .into_iter()
        .filter(|(high, low)| {
            !taken.contains(high) && !taken.contains(low) && taken.insert(*high) && taken.insert(*low)
        })
        .collect();
    if joined.is_empty() {
        return made;
    }
    let mut fresh = made
        .values()
        .flatten()
        .flat_map(|one| one.defines.iter().chain(&one.uses))
        .copied()
        .chain(
            blocks
                .iter()
                .flat_map(|block| &block.phis)
                .flat_map(|phi| std::iter::once(phi.result).chain(phi.incoming.iter().map(|(_, value)| *value))),
        )
        .max()
        .unwrap_or(0);
    for (high, low) in joined {
        fresh += 1;
        let whole = ir::Held { value: fresh, width: 4 };
        let (at, _, _) = loads[&high].clone();
        let (_, _, cell) = loads[&low].clone();
        let insns = made.get_mut(&at).expect("the loads' block");
        let place = |value: u32| {
            insns
                .iter()
                .position(|one| word_load(regs, one).is_some_and(|(loaded, _)| loaded == value))
                .expect("the load")
        };
        let (high_index, low_index) = (place(high), place(low));
        let first = high_index.min(low_index);
        let mut load = (*insns[low_index]).clone();
        let mut what = load.what.take().expect("a load");
        what.dests = vec![Loc::Held(whole)];
        what.sources = vec![Loc::Mem(ir::Mem { width: 4, ..cell })];
        load.what = Some(what);
        load.defines = vec![fresh];
        let (dropped, kept) = (Arc::clone(&insns[high_index.max(low_index)]), Arc::new(load));
        insns[first] = kept;
        insns.retain(|one| !Arc::ptr_eq(one, &dropped));
        for insns in made.values_mut() {
            let mut out = Vec::with_capacity(insns.len());
            let mut index = 0;
            while index < insns.len() {
                if index + 1 < insns.len()
                    && pushed(&insns[index]) == Some(high)
                    && pushed(&insns[index + 1]) == Some(low)
                {
                    let mut push = (*insns[index + 1]).clone();
                    let mut what = push.what.take().expect("a push");
                    what.sources = vec![Loc::Held(whole)];
                    push.what = Some(what);
                    push.uses = vec![fresh];
                    out.push(Arc::new(push));
                    index += 2;
                } else {
                    out.push(Arc::clone(&insns[index]));
                    index += 1;
                }
            }
            *insns = out;
        }
    }
    made
}

/// Select immediate pushes without keeping literal addresses live across calls.
fn _rematerialized_arguments(
    insns: &[Arc<Insn>],
    uses: &IndexMap<u32, i64>,
    exposed: &BTreeSet<u32>,
) -> Vec<Arc<Insn>> {
    let mut literals: IndexMap<u32, (Arc<Insn>, ir::Imm)> = IndexMap::default();
    // Only values whose every known use is an eligible PUSH may be recreated
    // independently of an ordered setup sequence.
    let mut pushed: IndexMap<u32, i64> = IndexMap::default();
    for one in insns {
        if let Some(what) = &one.what {
            if what.op == Operation::Push
                && what.sources.len() == 1
                && matches!(
                    &what.sources[0],
                    Loc::Held(held) if one.uses.contains(&held.value)
                )
            {
                let Loc::Held(held) = &what.sources[0] else { unreachable!() };
                for value in &one.uses {
                    if *value == held.value {
                        *pushed.entry(*value).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    let mut consumed: IndexMap<u32, i64> = IndexMap::default();
    let mut out = Vec::new();
    for one in insns {
        let mut one = Arc::clone(one);
        if let Some((value, width)) = one.what.as_ref().and_then(pushed_value) {
            if literals.contains_key(&value)
                && count(uses, value) == count(&pushed, value)
                && plain(&one)
                && one.defines.is_empty()
                && one.uses == [value]
            {
                let (definition, immediate) = literals[&value].clone();
                if width == immediate.width {
                    let mut rewritten = (*one).clone();
                    let mut what = rewritten.what.clone().unwrap();
                    what.sources = vec![Loc::Imm(immediate.clone())];
                    rewritten.what = Some(what);
                    rewritten.uses = vec![];
                    rewritten.call = definition.call.clone();
                    rewritten.symbol = Some(immediate.address.is_some());
                    rewritten.rematerialized = true;
                    one = Arc::new(rewritten);
                    *consumed.entry(value).or_insert(0) += 1;
                }
            }
        }
        for value in &one.defines {
            literals.shift_remove(value);
        }
        if let Some((value, width, immediate)) = one.what.as_ref().and_then(immediate_copy) {
            if width == immediate.width
                && matches!(width, 2 | 4)
                && one.defines == [value]
                && one.uses.is_empty()
                && plain(&one)
            {
                literals.insert(value, (Arc::clone(&one), immediate));
            }
        }
        out.push(one);
    }
    let dead: Vec<Arc<Insn>> = literals
        .iter()
        .filter(|(value, _)| {
            count(&consumed, **value) == count(uses, **value)
                && count(&consumed, **value) != 0
                && !exposed.contains(value)
        })
        .map(|(_, (definition, _))| Arc::clone(definition))
        .collect();
    lir::without(&out, |one| dead.iter().any(|dead| Arc::ptr_eq(dead, one)), None::<fn(&Arc<Insn>) -> Arc<Insn>>)
}
