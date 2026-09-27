//! What the old route selects across instructions once each is selected,
//! in `lower::lowered`'s order: a far pointer's two words as `les`, a
//! one-use load as a comparison's operand, a load, update and store as one
//! read-modify-write, and a push's load or constant as its operand. The
//! argument selections are `lower.rs`'s, which goes with the old route.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::Register;

use crate::backend::{comparefold, farload, rmw};
use crate::model::ir::{self, Loc, Operation, Space};
use crate::model::lir::{self, Insn, LirBlock};
use crate::support::hash::IndexMap;

/// `blocks` with the selections made. No value of isel's is exposed: a
/// result leaves through a return's operands.
pub(super) fn combined(blocks: Vec<LirBlock>) -> Vec<LirBlock> {
    let exposed = BTreeSet::new();
    let read_by_phis: BTreeSet<u32> = blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value)).collect();
    let made: IndexMap<i64, Vec<Arc<Insn>>> = blocks.iter().map(|block| (block.at, block.insns.clone())).collect();
    // A far-pointer field is two language-visible word loads but one target instruction.
    let selecting = farload::selectors(&made, &read_by_phis);
    let made: IndexMap<i64, Vec<Arc<Insn>>> = made.into_iter().map(|(at, insns)| (at, farload::selected(&insns, &selecting))).collect();
    let uses = recount(&made, &blocks);
    // A one-use comparison load is a legal memory operand.
    let made: IndexMap<i64, Vec<Arc<Insn>>> = made.into_iter().map(|(at, insns)| (at, comparefold::selected(&insns, &uses, &exposed))).collect();
    let uses = recount(&made, &blocks);
    // x86 can express a C read-modify-write update in one memory operand.
    let made: IndexMap<i64, Vec<Arc<Insn>>> = made.into_iter().map(|(at, insns)| (at, rmw::selected(&insns, &uses))).collect();
    let made: IndexMap<i64, Vec<Arc<Insn>>> = made.into_iter().map(|(at, insns)| (at, _memory_arguments(&insns, &uses, &exposed))).collect();
    let made: IndexMap<i64, Vec<Arc<Insn>>> = made.into_iter().map(|(at, insns)| (at, paired_pushes(&insns, &uses, &exposed))).collect();
    let made: IndexMap<i64, Vec<Arc<Insn>>> = made.into_iter().map(|(at, insns)| (at, _immediate_arguments(&insns, &uses))).collect();
    let made: IndexMap<i64, Vec<Arc<Insn>>> =
        made.into_iter().map(|(at, insns)| (at, _rematerialized_arguments(&insns, &uses, &exposed))).collect();
    let uses = recount(&made, &blocks);
    let mut made = dword_pairs(made, &uses, &blocks);
    blocks.into_iter().map(|block| LirBlock { insns: made.shift_remove(&block.at).expect("every block"), ..block }).collect()
}

/// How many times each value is read: by instructions, their fixed
/// inputs, and phis.
fn recount(made: &IndexMap<i64, Vec<Arc<Insn>>>, blocks: &[LirBlock]) -> IndexMap<u32, i64> {
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
    for value in blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value)) {
        *uses.entry(value).or_insert(0) += 1;
    }
    uses
}

/// Python's `Counter`: a missing key reads as zero.
fn count(counter: &IndexMap<u32, i64>, value: u32) -> i64 {
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

/// The word a plain push reads from memory.
fn pushed_word(one: &Insn) -> Option<&ir::Mem> {
    let what = one.what.as_ref()?;
    match (what.op, what.name.as_deref(), &what.dests[..], &what.sources[..]) {
        (Operation::Push, Some("push"), [], [Loc::Mem(word)])
            if word.width == 2 && one.uses.is_empty() && one.defines.is_empty() && !one.volatile && plain(one) =>
        {
            Some(word)
        }
        _ => None,
    }
}

/// Two word pushes of one dword's halves, the high first, as one dword
/// push: both leave the same four bytes, the low at the lower address. A
/// half may be pushed from memory or from a one-use load nothing changes
/// before the push.
fn paired_pushes(insns: &[Arc<Insn>], uses: &IndexMap<u32, i64>, exposed: &BTreeSet<u32>) -> Vec<Arc<Insn>> {
    let mut loaded: IndexMap<u32, (Arc<Insn>, ir::Mem)> = IndexMap::default();
    let mut dead: Vec<Arc<Insn>> = Vec::new();
    let mut out: Vec<Arc<Insn>> = Vec::with_capacity(insns.len());
    let mut index = 0;
    while index < insns.len() {
        let one = &insns[index];
        // The memory a word push reads, and the load it replaces if any.
        let word = |push: &Insn| -> Option<(ir::Mem, Option<Arc<Insn>>)> {
            if let Some(cell) = pushed_word(push) {
                return Some((cell.clone(), None));
            }
            let (value, 2) = push.what.as_ref().and_then(pushed_value)? else { return None };
            let (load, cell) = loaded.get(&value)?;
            (count(uses, value) == 1 && !exposed.contains(&value) && push.defines.is_empty() && !push.volatile && plain(push))
                .then(|| (cell.clone(), Some(Arc::clone(load))))
        };
        let paired = insns.get(index + 1).and_then(|next| {
            let ((high, high_load), (low, low_load)) = (word(one)?, word(next)?);
            let above = ir::Mem {
                addr: low.addr.map(|addr| addr.plus(2)),
                offset: if low.addr.is_some() { low.offset } else { low.offset + 2 },
                ..low.clone()
            };
            let same_registers = (high.through, high.index_through) == (low.through, low.index_through);
            (high == above && same_registers && ir::root(low.through) != Register::ESP).then(|| {
                let mut pushed = (**next).clone();
                let mut what = pushed.what.take().expect("a push");
                what.sources = vec![Loc::Mem(ir::Mem { width: 4, ..low.clone() })];
                pushed.what = Some(what);
                if let Some(load) = &low_load {
                    pushed.uses = load.uses.clone();
                    pushed.op = load.op.clone();
                    pushed.symbol = load.symbol;
                }
                dead.extend(high_load.into_iter().chain(low_load));
                Arc::new(pushed)
            })
        });
        let taken = if paired.is_some() { 2 } else { 1 };
        for one in &insns[index..index + taken] {
            let what = one.what.as_ref();
            let writes_memory = what.is_none_or(|what| what.dests.iter().any(|dest| matches!(dest, Loc::Mem(_))));
            let writes_fixed = what.is_none_or(|what| what.dests.iter().any(|dest| matches!(dest, Loc::Reg(_))));
            let barrier = what.is_none_or(|what| matches!(what.op, Operation::Barrier | Operation::Call | Operation::Return));
            if writes_memory || writes_fixed || barrier || !one.clobbers.is_empty() {
                loaded.clear();
            }
            for value in &one.defines {
                loaded.shift_remove(value);
            }
            if let Some((Operation::Move, Some("mov"), [Loc::Held(dest)], [Loc::Mem(source)])) =
                what.map(|what| (what.op, what.name.as_deref(), &what.dests[..], &what.sources[..]))
            {
                if dest.width == 2 && source.width == 2 && one.defines == [dest.value] && !one.volatile && plain(one) && ir::root(source.through) != Register::ESP {
                    loaded.insert(dest.value, (Arc::clone(one), source.clone()));
                }
            }
        }
        match paired {
            Some(one) => out.push(one),
            None => out.push(Arc::clone(one)),
        }
        index += taken;
    }
    lir::without(&out, |one| dead.iter().any(|dead| Arc::ptr_eq(dead, one)), None::<fn(&Arc<Insn>) -> Arc<Insn>>)
}

/// A word load of a dword's low or high half, and the cell it reads.
fn word_load(one: &Insn) -> Option<(u32, &ir::Mem)> {
    let what = one.what.as_ref()?;
    match (what.op, what.name.as_deref(), &what.dests[..], &what.sources[..]) {
        (Operation::Move, Some("mov"), [Loc::Held(dest)], [Loc::Mem(source)])
            if dest.width == 2 && source.width == 2 && one.defines == [dest.value] && !one.volatile && plain(one) && ir::root(source.through) != Register::ESP =>
        {
            Some((dest.value, source))
        }
        _ => None,
    }
}

/// Two word loads of one dword's halves, every use of either a push of the
/// high then the low, as one dword load pushed whole: the old route's far
/// pointer, never taken apart, was one dword.
fn dword_pairs(mut made: IndexMap<i64, Vec<Arc<Insn>>>, uses: &IndexMap<u32, i64>, blocks: &[LirBlock]) -> IndexMap<i64, Vec<Arc<Insn>>> {
    let loads: IndexMap<u32, (i64, usize, ir::Mem)> = made
        .iter()
        .flat_map(|(at, insns)| insns.iter().enumerate().filter_map(move |(index, one)| word_load(one).map(|(value, cell)| (value, (*at, index, cell.clone())))))
        .collect();
    let pushed = |one: &Insn| one.what.as_ref().and_then(pushed_value).filter(|&(_, width)| width == 2 && one.defines.is_empty() && plain(one)).map(|(value, _)| value);
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
            let (Some((high_at, high_index, high_cell)), Some((low_at, low_index, low_cell))) = (loads.get(high), loads.get(low)) else { return false };
            let above = ir::Mem { addr: low_cell.addr.map(|addr| addr.plus(2)), offset: if low_cell.addr.is_some() { low_cell.offset } else { low_cell.offset + 2 }, ..low_cell.clone() };
            // Loaded together, nothing between may change either half.
            let between = &made[high_at][(*high_index).min(*low_index) + 1..(*high_index).max(*low_index)];
            high_at == low_at
                && *high_cell == above
                && (high_cell.through, high_cell.index_through) == (low_cell.through, low_cell.index_through)
                && between.iter().all(|one| {
                    one.what.as_ref().is_some_and(|what| {
                        !what.dests.iter().any(|dest| matches!(dest, Loc::Mem(_) | Loc::Reg(_))) && !matches!(what.op, Operation::Barrier | Operation::Call | Operation::Return)
                    }) && one.clobbers.is_empty()
                })
        })
        .map(|(pair, _)| *pair)
        .collect();
    // A half joins one pair.
    let mut taken: BTreeSet<u32> = BTreeSet::new();
    let joined: Vec<(u32, u32)> = joined.into_iter().filter(|(high, low)| !taken.contains(high) && !taken.contains(low) && taken.insert(*high) && taken.insert(*low)).collect();
    if joined.is_empty() {
        return made;
    }
    let mut fresh = made.values().flatten().flat_map(|one| one.defines.iter().chain(&one.uses)).copied()
        .chain(blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| std::iter::once(phi.result).chain(phi.incoming.iter().map(|(_, value)| *value))))
        .max().unwrap_or(0);
    for (high, low) in joined {
        fresh += 1;
        let whole = ir::Held { value: fresh, width: 4 };
        let (at, _, _) = loads[&high].clone();
        let (_, _, cell) = loads[&low].clone();
        let insns = made.get_mut(&at).expect("the loads' block");
        let place = |value: u32| insns.iter().position(|one| word_load(one).is_some_and(|(loaded, _)| loaded == value)).expect("the load");
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
                if index + 1 < insns.len() && pushed(&insns[index]) == Some(high) && pushed(&insns[index + 1]) == Some(low) {
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
                && matches!(&what.sources[0], Loc::Held(held) if one.uses.contains(&held.value))
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
                    rewritten.op = definition.op.clone();
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
            count(&consumed, **value) == count(uses, **value) && count(&consumed, **value) != 0 && !exposed.contains(value)
        })
        .map(|(_, (definition, _))| Arc::clone(definition))
        .collect();
    lir::without(&out, |one| dead.iter().any(|dead| Arc::ptr_eq(dead, one)), None::<fn(&Arc<Insn>) -> Arc<Insn>>)
}

/// Fold a single-use load into the PUSH that consumes it.
fn _memory_arguments(insns: &[Arc<Insn>], uses: &IndexMap<u32, i64>, exposed: &BTreeSet<u32>) -> Vec<Arc<Insn>> {
    let mut loaded: IndexMap<u32, (Arc<Insn>, ir::Mem)> = IndexMap::default();
    let mut consumed: IndexMap<u32, i64> = IndexMap::default();
    let mut out: Vec<Arc<Insn>> = Vec::new();
    for one in insns {
        let mut one = Arc::clone(one);
        let mut folded = false;
        if let Some((value, width)) = one.what.as_ref().and_then(pushed_value) {
            if let Some((definition, source)) = loaded.get(&value).cloned() {
                if width == source.width
                    && matches!(source.width, 2 | 4)
                    && count(uses, value) == 1
                    && !exposed.contains(&value)
                    && one.defines.is_empty()
                    && plain(&one)
                {
                    let mut rewritten = (*one).clone();
                    let mut what = rewritten.what.clone().unwrap();
                    what.sources = vec![Loc::Mem(source)];
                    rewritten.what = Some(what);
                    rewritten.uses = vec![];
                    rewritten.op = definition.op.clone();
                    rewritten.symbol = definition.symbol;
                    one = Arc::new(rewritten);
                    *consumed.entry(value).or_insert(0) += 1;
                    folded = true;
                }
            }
        }

        let what = one.what.as_ref();
        let writes_memory = what.is_none_or(|what| what.dests.iter().any(|dest| matches!(dest, Loc::Mem(_))));
        let writes_fixed = what.is_none_or(|what| what.dests.iter().any(|dest| matches!(dest, Loc::Reg(_))));
        let barrier =
            what.is_none_or(|what| matches!(what.op, Operation::Barrier | Operation::Call | Operation::Return));
        if writes_memory || writes_fixed || barrier || !one.clobbers.is_empty() {
            loaded.clear();
        }

        for value in &one.defines {
            loaded.shift_remove(value);
        }
        if !folded {
            if let Some(what) = what {
                if let (Operation::Move, Some("mov"), [Loc::Held(dest)], [Loc::Mem(source)]) =
                    (what.op, what.name.as_deref(), &what.dests[..], &what.sources[..])
                {
                    if dest.width == source.width
                        && matches!(source.width, 2 | 4)
                        && source.addr.is_some_and(|addr| addr.space == Space::Frame && addr.disp >= 4)
                        && ir::root(source.through) != Register::ESP
                        && ir::root(source.index_through) != Register::ESP
                        && !source.stack_argument
                        && one.defines == [dest.value]
                        && one.uses.is_empty()
                        && plain(&one)
                    {
                        loaded.insert(dest.value, (Arc::clone(&one), source.clone()));
                    }
                }
            }
        }
        out.push(one);
    }

    let dead: BTreeSet<u32> = consumed
        .keys()
        .filter(|value| {
            count(&consumed, **value) == count(uses, **value) && count(&consumed, **value) != 0 && !exposed.contains(value)
        })
        .copied()
        .collect();
    let mut previous: Option<Vec<*const Insn>> = None;
    loop {
        let now: Vec<*const Insn> = out.iter().map(Arc::as_ptr).collect();
        if previous.as_ref() == Some(&now) {
            break;
        }
        previous = Some(now);
        out = lir::without(
            &out,
            |one| one.defines.iter().any(|value| dead.contains(value)),
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
    }
    out
}

/// Select a direct push for an adjacent, single-use immediate definition.
fn _immediate_arguments(insns: &[Arc<Insn>], uses: &IndexMap<u32, i64>) -> Vec<Arc<Insn>> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < insns.len() {
        if index + 1 < insns.len() && plain(&insns[index]) && plain(&insns[index + 1]) {
            let (copy, push) = (&insns[index], &insns[index + 1]);
            let copied = copy.what.as_ref().and_then(immediate_copy);
            let pushed = push.what.as_ref().and_then(pushed_value);
            if let (Some((value, width, immediate)), Some((pushed, pushed_width))) = (copied, pushed) {
                if value == pushed
                    && width == pushed_width
                    && pushed_width == immediate.width
                    && matches!(width, 2 | 4)
                    && count(uses, value) == 1
                    && copy.uses.is_empty()
                    && copy.defines == [value]
                    && push.uses == [value]
                    && push.defines.is_empty()
                {
                    let mut combined = (**copy).clone();
                    let mut what = push.what.clone().unwrap();
                    what.sources = vec![Loc::Imm(immediate)];
                    combined.what = Some(what);
                    combined.defines = vec![];
                    combined.uses = vec![];
                    let folded = lir::without(
                        &[Arc::new(combined), Arc::clone(push)],
                        |one| Arc::ptr_eq(one, push),
                        None::<fn(&Arc<Insn>) -> Arc<Insn>>,
                    );
                    out.extend(folded);
                    index += 2;
                    continue;
                }
            }
        }
        out.push(Arc::clone(&insns[index]));
        index += 1;
    }
    out
}
