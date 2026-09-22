//! Port of `qbopt/backend/twoaddr.py`: x86 writes into one of the registers
//! it reads.
//!
//! LLVM's `TwoAddressInstructionPass`: `c := a + b` becomes `c := a` then
//! `c := c + b`.

use std::collections::BTreeSet;
use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};

use crate::analysis::intervals as ranges;
use crate::backend::{allocate, coalesce, spiller};
use crate::model::ir::{self, Held, Loc, Mem, Operation, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;

/// Operations that read their destination.
const _TIED: [Operation; 3] = [Operation::Binary, Operation::Unary, Operation::Funnel];

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

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        Ok(tied(&body))
    }
}

/// `body` with every tied instruction reading what it writes.
pub fn tied(body: &LirBody) -> LirBody {
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
        let mut live_after: IndexMap<usize, BTreeSet<u32>> = IndexMap::new();
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
        blocks.push(LirBlock { insns, ..block.clone() });
    }
    if changed { LirBody { blocks, ..body.clone() } } else { body.clone() }
}

/// Which values each value is copied to or from.
fn _copy_destinations(body: &LirBody) -> IndexMap<u32, BTreeSet<u32>> {
    let mut adjacent: IndexMap<u32, BTreeSet<u32>> = IndexMap::new();
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

/// How many copies apart two values are; infinite where none joins them within `limit`.
fn _distance(copies: &IndexMap<u32, BTreeSet<u32>>, start: u32, goal: u32, limit: i64) -> f64 {
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
    interference: Option<&IndexMap<u32, BTreeSet<u32>>>,
) -> Option<Arc<Insn>> {
    let what = one.what.as_ref()?;
    let commutative = (what.op == Operation::Binary && matches!(what.name.as_deref(), Some("add" | "and" | "or" | "xor")))
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
    let no_copies = IndexMap::new();
    let copies = copies.unwrap_or(&no_copies);
    let empty = BTreeSet::new();
    let affinities = copies.get(&into.value).unwrap_or(&empty);

    let blocked = |source: u32| -> usize {
        affinities
            .iter()
            .filter(|other| interference.and_then(|graph| graph.get(&source)).is_some_and(|found| found.contains(other)))
            .count()
    };

    let reusable = !alive.contains(&first.value)
        && !alive.contains(&second.value)
        && _less(
            (blocked(second.value), _distance(copies, into.value, second.value, 8)),
            (blocked(first.value), _distance(copies, into.value, first.value, 8)),
        );
    if second.value == into.value
        || alive.contains(&first.value) && !alive.contains(&second.value)
        || reusable
    {
        let mut made = one.clone();
        made.what = Some(Semantics { sources: vec![Loc::Held(*second), Loc::Held(*first)], ..what.clone() });
        return Some(Arc::new(made));
    }
    None
}

/// Python's tuple `<` over `(int, float)`.
fn _less(one: (usize, f64), other: (usize, f64)) -> bool {
    one.0 < other.0 || (one.0 == other.0 && one.1 < other.1)
}

/// An empty span at the neighbour's address: this claims no bytes.
fn _nothing(beside: &Insn) -> (i64, i64) {
    let at = beside.covers.map_or(beside.at, |covers| covers.0);
    (at, at)
}

fn _inserted(beside: &Insn, what: Semantics, defines: Vec<u32>, uses: Vec<u32>) -> Arc<Insn> {
    let mut made = Insn::new(beside.at, Some(_nothing(beside)), Some(what), defines, uses);
    made.op = beside.op.clone();
    Arc::new(made)
}

fn _move(into: Loc, out_of: Loc) -> Semantics {
    Semantics { name: Some("mov".to_owned()), dests: vec![into], sources: vec![out_of], ..Semantics::new(Operation::Move) }
}

/// The copy and the fixed instruction, or None where it is already tied.
fn _untied(one: &Insn, mint: &mut dyn FnMut() -> u32) -> Option<Vec<Arc<Insn>>> {
    let what = one.what.as_ref()?;
    if what.dests.is_empty() || what.sources.is_empty() {
        return None;
    }
    let multiply = what.op == Operation::Multiply && what.dests.len() == 1 && what.sources.len() == 2;
    if !_TIED.contains(&what.op) && !multiply {
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
    let movement = _inserted(one, _move(Loc::Held(*into), first.clone()), vec![into.value], first_value.into_iter().collect());
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
fn _through_register(one: &Insn, what: &Semantics, into: &Mem, mint: &mut dyn FnMut() -> u32) -> Option<Vec<Arc<Insn>>> {
    if what.dests.len() != 1 || what.sources[0] == Loc::Mem(into.clone()) || one.group.is_some() {
        return None;
    }
    if what.sources.iter().any(|source| matches!(source, Loc::Mem(_))) || !one.requires.is_empty() || !one.delivers.is_empty() {
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

    use indexmap::IndexMap;

    use super::{_commuted, _untied};
    use crate::model::ir::{Held, Imm, Loc, Operation, Semantics};
    use crate::model::lir::Insn;

    fn held(value: u32, width: u32) -> Loc {
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

    /// LNGMXX copied its accumulator out and back each iteration to preserve the invariant addend.
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

    /// Matmul tied ``imul`` to its live, spilled factor and reloaded it eight times.
    #[test]
    fn test_multiply_reuses_the_dying_operand() {
        let mut one = addition("imul");
        one.what.as_mut().unwrap().op = Operation::Multiply;

        let chosen = _commuted(&one, &BTreeSet::from([1, 3]), None, None).expect("swapped");

        assert_eq!(sources(&chosen), reversed(&one));
    }

    /// Modern fixed multiply shifted its multiplier instead of its product.
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

    /// LOCALP's backedge copy favors its accumulator only when its old value can be overwritten.
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

    /// CRC32 tied XOR to its shifted temporary, then copied the result around the backedge.
    #[test]
    fn test_crc32_ties_the_operand_that_can_join_its_loop_phi() {
        let one = addition("xor");
        let copies = graph(&[(3, &[4]), (4, &[3])]);
        let interference = graph(&[(1, &[4]), (4, &[1])]);

        let chosen = _commuted(&one, &BTreeSet::from([3]), Some(&copies), Some(&interference)).expect("swapped");

        assert_eq!(sources(&chosen), reversed(&one));
    }

    // ------------------------------------------------------ tests/test_lir.py

    fn imm(value: i64, width: u32) -> Loc {
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

    /// Experimental matrix setup emitted 20 * 20 for 0 * 20 without a destination copy.
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

    #[test]
    fn test_constant_multiply_lowers_without_a_destination_tie() {
        use crate::backend::lower::{self, Place, Placed};
        use crate::model::mir;

        let (source, result) = (mir::Value::new(900, 0), mir::Value::new(901, 1));
        let mut op = mir::Op::new(1, mir::OpCode::Operation(Operation::Multiply), "", vec![result], vec![source]);
        op.kind = mir::Kind::Mul;
        op.args =
            vec![mir::Arg::Held(mir::Held { value: source, width: 2 }), mir::Arg::Const(mir::Const::new(20, 2))];
        op.results = vec![mir::Arg::Held(mir::Held { value: result, width: 2 })];
        let made = lower::semantics(&op, None, Place::AsAValue).unwrap().expect("semantics");
        let located = |places: Vec<Placed>| -> Vec<Loc> {
            places
                .into_iter()
                .map(|place| match place {
                    Placed::Loc(one) => one,
                    other => panic!("not located: {other:?}"),
                })
                .collect()
        };
        let what = Semantics {
            name: made.name,
            dests: located(made.dests),
            sources: located(made.sources),
            target: made.target,
            indirect: made.indirect,
            ..Semantics::new(made.op)
        };
        assert_eq!(what.sources[1..], [held(source.id, 2), imm(20, 2)]);
        let insn = Insn::new(1, Some((1, 1)), Some(what), vec![result.id], vec![source.id]);
        assert!(fixed(&insn).is_none());
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
