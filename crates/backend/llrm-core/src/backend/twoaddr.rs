//! Port of `qbopt/backend/twoaddr.py`: x86 writes into one of the registers
//! it reads.
//!
//! LLVM's `TwoAddressInstructionPass`: `c := a + b` becomes `c := a` then
//! `c := c + b`.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::analysis::intervals as ranges;
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

    let (_, leaving) = allocate::live(body);
    let copies = _copy_destinations(body);
    let interference = coalesce::_interference(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut alive = leaving[&block.at].clone();
        let mut live_after: IndexMap<usize, BTreeSet<u32>> = IndexMap::default();
        for one in block.insns.iter().rev() {
            live_after.insert(ranges::key(one), alive.clone());
            for value in &one.defines {
                alive.remove(value);
            }
            alive.extend(one.uses.iter().copied());
        }
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        for one in &block.insns {
            let mut one = Arc::clone(one);
            if let Some(chosen) = _commuted(&one, &live_after[&ranges::key(&one)], Some(&copies), Some(&interference)) {
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
    let (_, leaving) = allocate::live(body);
    let mut changed = false;
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
        let out = &leaving[&block.at];
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
                || out.contains(&into.value)
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

/// The instruction with its commutative sources swapped, or None where it
/// is returned as it was.
fn _commuted(
    one: &Insn,
    alive: &BTreeSet<u32>,
    copies: Option<&IndexMap<u32, BTreeSet<u32>>>,
    interference: Option<&coalesce::Graph>,
) -> Option<Arc<Insn>> {
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
    let no_copies = IndexMap::default();
    let copies = copies.unwrap_or(&no_copies);
    let empty = BTreeSet::new();
    let affinities = copies.get(&into.value).unwrap_or(&empty);

    let blocked = |source: u32| -> usize {
        affinities
            .iter()
            .filter(|other| {
                interference.and_then(|graph| graph.get(&source)).is_some_and(|found| found.contains(other))
            })
            .count()
    };

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

    use super::{_commuted, _untied, tied};
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

    /// LNGMXX copied its accumulator out and back each iteration to preserve
    /// the invariant addend.
    #[test]
    fn test_commutative_instruction_reuses_the_dying_operand() {
        for name in ["add", "and", "or", "xor"] {
            let one = addition(name);
            let chosen = _commuted(&one, &BTreeSet::from([1, 3]), None, None).expect(name);
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

        let chosen = _commuted(&one, &BTreeSet::from([1, 3]), None, None).expect("swapped");

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
            vec![LirBlock { succ: vec![0], ..LirBlock::new(0, vec![Arc::new(add), Arc::new(back)]) }],
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
            vec![LirBlock { succ: vec![0], ..LirBlock::new(0, vec![Arc::new(add), Arc::new(back), Arc::new(old)]) }],
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
            assert!(_commuted(&one, &BTreeSet::from([1, 3]), None, None).is_none(), "{name}");
        }
    }

    #[test]
    fn test_live_operands_and_grouped_operations_keep_their_order() {
        let one = addition("add");
        assert!(_commuted(&one, &BTreeSet::from([1, 2, 3]), None, None).is_none());
        let grouped = Insn { group: Some(1), ..one };
        assert!(_commuted(&grouped, &BTreeSet::from([1, 3]), None, None).is_none());
    }

    /// LOCALP's backedge copy favors its accumulator only when its old value
    /// can be overwritten.
    #[test]
    fn test_result_copy_affinity_does_not_override_liveness() {
        for (alive, swapped) in [(vec![3], true), (vec![2, 3], false), (vec![1, 2, 3], false)] {
            let one = addition("add");
            let copies = graph(&[(3, &[2])]);
            let chosen = _commuted(&one, &alive.iter().copied().collect(), Some(&copies), None);
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

        let chosen = _commuted(&one, &BTreeSet::from([3]), Some(&copies), Some(&interference)).expect("swapped");

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
