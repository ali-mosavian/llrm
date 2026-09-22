//! Loop strength reduction for direct scalar affine formulas.
//!
//! Direct port of the scalar, unpriced path in
//! `qbopt/optimize/strength.py:reduced`.  The Python implementation has
//! broader formula selection for address forms, pointers, offsets, and
//! capacity/cost policy.  This first Rust slice intentionally accepts only a
//! direct, non-pointer, non-indexed multiply-derived formula.  It consumes
//! `analysis::induction::of`'s recurrence proof rather than recognizing a
//! recurrence again.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::analysis::induction::{self, AffineOperand, Derived};
use crate::analysis::occurrence::{self, OpOccurrence};
use crate::analysis::regions::{RegionError, RegionLayout};
use crate::analysis::ssa::{self, ConstructionError};
use crate::model::mir::{Arg, Const, Held, Kind, MirBody, Op, OrderedMap, Value};

use super::transform;

/// Failures forwarded from the two analyses this rewrite composes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StrengthError {
    Region(RegionError),
    Construction(ConstructionError),
}

impl fmt::Display for StrengthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Region(error) => write!(formatter, "{error:?}"),
            Self::Construction(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for StrengthError {}

impl From<RegionError> for StrengthError {
    fn from(error: RegionError) -> Self {
        Self::Region(error)
    }
}

impl From<ConstructionError> for StrengthError {
    fn from(error: ConstructionError) -> Self {
        Self::Construction(error)
    }
}

/// Replace direct scalar loop multiplications with loop-carried additions.
///
/// This deliberately partial API does not price formulas, share recurrence
/// formulas, award control-replacement credits, or select an addressing form.
/// It refuses pointers, cells, indexed forms, and composed invariant offsets.
/// `layout` is forwarded unchanged to induction, where memory facts are
/// established; no layout, alias, or bounds fact is reconstructed here.
pub(crate) fn reduced_direct(
    body: &MirBody,
    layout: Option<&RegionLayout>,
) -> Result<MirBody, StrengthError> {
    let found = induction::of(body, &std::collections::BTreeSet::new(), layout)?;
    if found.is_empty() {
        return Ok(body.clone());
    }

    // Python's `{block.at: block ...}` retains the final duplicate.  MIR
    // construction normally has unique block addresses, but retaining this
    // detail keeps the selection rules identical for a malformed snapshot.
    let blocks = body
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.at, index))
        .collect::<BTreeMap<_, _>>();
    let mut taken = ssa::values(body)
        .map(|value| value.variable)
        .max()
        .unwrap_or(0);
    let mut new_variables = BTreeSet::new();
    let mut ahead = BTreeMap::<i64, Vec<Op>>::new();
    let mut behind = BTreeMap::<i64, Vec<Op>>::new();
    let mut replacements = BTreeMap::<OpOccurrence, Op>::new();

    for (loop_, _basics, derived) in found {
        let Some(preheader) = transform::preheader(body, &loop_) else {
            continue;
        };
        let Some(&preheader_index) = blocks.get(&preheader) else {
            continue;
        };
        if body.blocks[preheader_index].succ.as_slice() != [loop_.header] {
            continue;
        }
        let latches = loop_
            .latches
            .iter()
            .filter(|at| blocks.contains_key(at))
            .copied()
            .collect::<Vec<_>>();
        let [latch] = latches.as_slice() else {
            continue;
        };

        // `_formula_set(candidates)` with no budget retains the leaves. It
        // must see the full candidate group before this direct slice rejects
        // representations it cannot emit: otherwise a selected parent could
        // duplicate a larger, unsupported derived formula.
        let candidates = derived
            .iter()
            .filter(|formula| candidate(body, formula, &derived))
            .collect::<Vec<_>>();
        for formula in formula_leaves(body, &candidates)
            .into_iter()
            .filter(|formula| direct_formula(body, formula, &derived))
        {
            if replacements.contains_key(&formula.op) {
                // An inner loop's rewrite owns this exact immutable source
                // occurrence; Python never reduces it a second time.
                continue;
            }
            let operation = operation(body, formula.op);
            let Some(answer) = answer(body, formula.op) else {
                continue;
            };
            let width = width(operation);
            let Some(stride) = times(&formula.of.step, &formula.by) else {
                continue;
            };

            // Python increments its variable namespace once per selected
            // formula and derives its source ids from the immutable input
            // body's maximum. The update is immediately after the seed.
            taken += 1;
            let seed = Value {
                id: next_id(body, taken),
                at: preheader,
                flags: false,
                variable: taken,
                version: 1,
            };
            let next = Value {
                id: seed.id + 1,
                at: *latch,
                flags: false,
                variable: taken,
                version: 2,
            };
            new_variables.insert(taken);

            ahead
                .entry(preheader)
                .or_default()
                .push(start(seed, formula, preheader, operation));
            behind.entry(*latch).or_default().push(made(
                Kind::Add,
                "add",
                next,
                vec![Arg::Held(Held { value: seed, width }), stride],
                *latch,
                operation,
            ));
            replacements.insert(formula.op, copying(operation, seed, answer, width));
        }
    }

    if replacements.is_empty() {
        return Ok(body.clone());
    }

    let mut changed = body.clone();
    for (block_index, block) in changed.blocks.iter_mut().enumerate() {
        let source = &body.blocks[block_index];
        // Python looks up edits by address for every block occurrence.  Do
        // not consume the entry: duplicate-address snapshots receive the
        // same insertion at each matching occurrence.
        let inserted_before = ahead.get(&block.at).cloned().unwrap_or_default();
        let inserted_after = behind.get(&block.at).cloned().unwrap_or_default();
        let mut operations = source
            .ops
            .iter()
            .enumerate()
            .map(|(operation_index, operation)| {
                let occurrence = occurrence_at(body, block_index, operation_index);
                replacements
                    .get(&occurrence)
                    .cloned()
                    .unwrap_or_else(|| operation.clone())
            })
            .collect::<Vec<_>>();
        weave(&mut operations, block.at, inserted_before, inserted_after);
        block.ops = operations;
    }

    Ok(ssa::constructed(&changed, &new_variables)?)
}

/// The operation named by Python's exact object identity.
fn operation(body: &MirBody, at: OpOccurrence) -> &Op {
    &body.blocks[at.block_index()].ops[at.operation_index()]
}

/// Recover a snapshot-local occurrence key; no source provenance participates.
fn occurrence_at(body: &MirBody, block_index: usize, operation_index: usize) -> OpOccurrence {
    occurrence::operations(body)
        .find(|(at, _, _)| {
            at.block_index() == block_index && at.operation_index() == operation_index
        })
        .map(|(at, _, _)| at)
        .expect("body operation ordinal belongs to its immutable snapshot")
}

/// Python `_candidates`, used only to retain its unpriced leaf selection.
fn candidate(body: &MirBody, formula: &Derived, derived: &[Derived]) -> bool {
    answer(body, formula.op).is_some()
        && (multiplies(body, formula, derived)
            || formula.pointer.is_some()
            || operation(body, formula.op).kind == Kind::Divmod
            || (!formula.offsets.is_empty() && operation(body, formula.op).kind == Kind::Shl)
            || formula
                .offsets
                .iter()
                .any(|(offset, _)| matches!(offset, Arg::Cell(_)))
            || (operation(body, formula.op).kind == Kind::Add
                && formula
                    .offsets
                    .iter()
                    .any(|(offset, _)| matches!(offset, Arg::Held(_)))))
}

/// The direct subset of a selected Python candidate.
fn direct_formula(body: &MirBody, formula: &Derived, derived: &[Derived]) -> bool {
    formula.pointer.is_none()
        && formula.offsets.is_empty()
        && matches!(&formula.by, Arg::Const(_) | Arg::Held(_))
        && multiplies(body, formula, derived)
}

/// Python `_formula_set(candidates)` without a budget: retain its leaves.
fn formula_leaves<'a>(body: &MirBody, candidates: &[&'a Derived]) -> Vec<&'a Derived> {
    // Python's dict keeps the last formula for a repeated result while
    // retaining source order for the later selected-list walk.
    let mut made = BTreeMap::<Value, OpOccurrence>::new();
    for candidate in candidates {
        if let Some(Arg::Held(held)) = operation(body, candidate.op).results.first() {
            made.insert(held.value, candidate.op);
        }
    }
    let consumed = candidates
        .iter()
        .flat_map(|candidate| operation(body, candidate.op).args.iter())
        .filter_map(|argument| match argument {
            Arg::Held(held) if made.contains_key(&held.value) => Some(held.value),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let selected = candidates
        .iter()
        .filter_map(|candidate| {
            operation(body, candidate.op)
                .results
                .first()
                .is_some_and(
                    |result| matches!(result, Arg::Held(held) if !consumed.contains(&held.value)),
                )
                .then_some(candidate.op)
        })
        .collect::<BTreeSet<_>>();
    candidates
        .iter()
        .copied()
        .filter(|candidate| selected.contains(&candidate.op))
        .collect()
}

/// Python `_multiplies`, retaining operation occurrence identity throughout.
fn multiplies(body: &MirBody, formula: &Derived, derived: &[Derived]) -> bool {
    let producers = derived
        .iter()
        .filter(|candidate| candidate.of == formula.of)
        .filter_map(|candidate| {
            operation(body, candidate.op)
                .results
                .first()
                .and_then(|result| match result {
                    Arg::Held(held) => Some((held.value, candidate.op)),
                    _ => None,
                })
        })
        .collect::<BTreeMap<_, _>>();
    let mut pending = vec![formula.op];
    let mut seen = BTreeSet::new();
    while let Some(at) = pending.pop() {
        if !seen.insert(at) {
            continue;
        }
        let operation = operation(body, at);
        if operation.kind == Kind::Mul {
            return true;
        }
        pending.extend(operation.args.iter().filter_map(|argument| match argument {
            Arg::Held(held) => producers.get(&held.value).copied(),
            _ => None,
        }));
    }
    false
}

/// Python `_answer`, including the phi-read closure for inactive phi arms.
fn answer(body: &MirBody, occurrence: OpOccurrence) -> Option<Value> {
    let mut read = occurrence::operations(body)
        .filter(|(other, _, _)| *other != occurrence)
        .flat_map(|(_, _, operation)| operation.uses.iter().map(|value| value.id))
        .collect::<BTreeSet<_>>();
    let incoming = body
        .blocks
        .iter()
        .flat_map(|block| &block.phis)
        .map(|phi| {
            (
                phi.result.id,
                phi.incoming.values().copied().collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut pending = read.iter().copied().collect::<Vec<_>>();
    while let Some(value) = pending.pop() {
        for input in incoming.get(&value).into_iter().flatten() {
            if read.insert(input.id) {
                pending.push(input.id);
            }
        }
    }
    let operation = operation(body, occurrence);
    let wanted = operation
        .defines
        .iter()
        .copied()
        .filter(|value| read.contains(&value.id))
        .collect::<Vec<_>>();
    let [answer] = wanted.as_slice() else {
        return None;
    };
    let Some(Arg::Held(result)) = operation.results.first() else {
        return None;
    };
    (!answer.flags && *answer == result.value).then_some(*answer)
}

/// Python `_times`, deliberately leaving a nonconstant product unformed.
fn times(step: &AffineOperand, by: &Arg) -> Option<Arg> {
    match (step, by) {
        (AffineOperand::Const(step), by) if step.n == 1.into() => Some(by.clone()),
        (AffineOperand::Const(step), Arg::Const(by)) => Some(Arg::Const(Const::new(
            &step.n * &by.n,
            step.width.max(by.width),
        ))),
        _ => None,
    }
}

/// Python `_start` for a formula with neither pointer nor offsets.
fn start(into: Value, formula: &Derived, at: i64, beside: &Op) -> Op {
    let copying = matches!(&formula.by, Arg::Const(constant) if constant.n == 1.into());
    let kind = copying.then_some(Kind::Copy).unwrap_or(Kind::Mul);
    let name = if kind == Kind::Copy { "mov" } else { "imul" };
    let args = if copying {
        vec![affine_arg(&formula.of.start)]
    } else {
        vec![affine_arg(&formula.of.start), formula.by.clone()]
    };
    made(kind, name, into, args, at, beside)
}

fn affine_arg(operand: &AffineOperand) -> Arg {
    match operand {
        AffineOperand::Held(held) => Arg::Held(*held),
        AffineOperand::Const(constant) => Arg::Const(constant.clone()),
    }
}

/// Python `_made`: a source-free semantic operation beside one source shape.
fn made(kind: Kind, name: &str, into: Value, args: Vec<Arg>, at: i64, beside: &Op) -> Op {
    let mut uses = Vec::new();
    let mut loads = Vec::new();
    for argument in &args {
        match argument {
            Arg::Held(held) if !uses.contains(&held.value) => uses.push(held.value),
            Arg::Cell(cell) => {
                loads.push(cell.r#ref.clone());
                for value in [cell.r#ref.base, cell.r#ref.segment].into_iter().flatten() {
                    if !uses.contains(&value) {
                        uses.push(value);
                    }
                }
            }
            _ => {}
        }
    }
    let width = args
        .iter()
        .map(|argument| match argument {
            Arg::Held(held) => held.width,
            Arg::Const(constant) => constant.width,
            _ => 2,
        })
        .max()
        .unwrap_or(2);
    let mut operation = Op::new(at, beside.op, name, vec![into], uses);
    operation.kind = kind;
    operation.loads = loads;
    operation.source_backed = false;
    operation.args = args;
    operation.results = vec![Arg::Held(Held { value: into, width })];
    operation
}

/// Python `_copying`, preserving every source field not named by `replace`.
fn copying(source: &Op, start: Value, answer: Value, width: u32) -> Op {
    let mut replacement = source.clone();
    replacement.kind = Kind::Copy;
    replacement.name.clear();
    replacement.source_backed = false;
    replacement.defines = vec![answer];
    replacement.uses = vec![start];
    replacement.args = vec![Arg::Held(Held {
        value: start,
        width,
    })];
    replacement.results = vec![Arg::Held(Held {
        value: answer,
        width,
    })];
    replacement.loads.clear();
    replacement.stores.clear();
    replacement.merges = OrderedMap::new();
    replacement.symbol = Some(false);
    replacement
}

fn width(operation: &Op) -> u32 {
    operation
        .results
        .iter()
        .find_map(|result| match result {
            Arg::Held(held) => Some(held.width),
            _ => None,
        })
        .unwrap_or(2)
}

/// Python `_next`, including its intentionally sparse source-id namespace.
fn next_id(body: &MirBody, taken: u32) -> u32 {
    let mut seen = BTreeSet::from([0]);
    for block in &body.blocks {
        for operation in &block.ops {
            seen.extend(operation.defines.iter().map(|value| value.id));
            seen.extend(operation.uses.iter().map(|value| value.id));
        }
        seen.extend(block.phis.iter().map(|phi| phi.result.id));
    }
    seen.last().copied().unwrap_or(0) + 1 + taken * 2
}

/// Python `_woven`: set up after preheader work and advance before a latch
/// terminator, with inserted source ownership left empty.
fn weave(operations: &mut Vec<Op>, block_at: i64, ahead: Vec<Op>, behind: Vec<Op>) {
    if ahead.is_empty() && behind.is_empty() {
        return;
    }
    let mut cut = operations.len();
    while cut > 0 && matches!(operations[cut - 1].kind, Kind::Jump | Kind::Branch) {
        cut -= 1;
    }
    let at = operations
        .get(cut)
        .or_else(|| operations.last())
        .map(|operation| operation.at)
        .unwrap_or(block_at);
    let inserted = ahead
        .into_iter()
        .chain(behind)
        .map(|mut operation| {
            operation.at = at;
            operation.absorbed.clear();
            operation
        })
        .collect::<Vec<_>>();
    operations.splice(cut..cut, inserted);
}

#[cfg(test)]
mod tests {
    use super::{answer, reduced_direct};
    use crate::model::mir::{
        Arg, Const, Held, Kind, MirBlock, MirBody, Op, OrderedMap, Phi, Value,
    };

    fn value(id: u32, at: i64, variable: u32) -> Value {
        Value {
            id,
            at,
            flags: false,
            variable,
            version: 1,
        }
    }

    fn held(value: Value) -> Arg {
        Arg::Held(Held { value, width: 2 })
    }

    fn direct_body() -> (MirBody, Value, Value) {
        let start = value(1, 0, 1);
        let counter = value(2, 1, 1);
        let next = value(3, 1, 1);
        let product = value(4, 1, 2);
        let mut incoming = OrderedMap::new();
        incoming.insert(0, start);
        incoming.insert(1, next);
        let mut step = Op::new(1, None, "", vec![next], vec![counter]);
        step.kind = Kind::Increment;
        step.args = vec![held(counter)];
        step.results = vec![held(next)];
        let mut multiply = Op::new(2, None, "", vec![product], vec![counter]);
        multiply.kind = Kind::Mul;
        multiply.args = vec![held(counter), Arg::Const(Const::new(3, 2))];
        multiply.results = vec![held(product)];
        let mut consume = Op::new(3, None, "", vec![], vec![product]);
        consume.kind = Kind::Arg;
        consume.args = vec![held(product)];
        (
            MirBody::new(
                0,
                vec![
                    MirBlock::new(0, vec![], vec![], vec![1]),
                    MirBlock::new(
                        1,
                        vec![Phi {
                            result: counter,
                            incoming,
                        }],
                        vec![step, multiply, consume],
                        vec![1, 2],
                    ),
                    MirBlock::new(2, vec![], vec![], vec![]),
                ],
            ),
            product,
            counter,
        )
    }

    #[test]
    fn direct_strength_reduces_multiply_to_preheader_seed_and_latch_add() {
        // Port of tests/test_flow.py:test_strength_reduction_replaces_a_loop_multiply_with_an_add.
        let (body, product, _counter) = direct_body();
        let result = reduced_direct(&body, None).expect("induction and construction");
        let preheader = &result.blocks[0];
        let header = &result.blocks[1];
        assert_eq!(preheader.ops.len(), 1);
        assert_eq!(preheader.ops[0].kind, Kind::Mul);
        let carried = header
            .phis
            .iter()
            .find(|phi| phi.result.variable != 1)
            .expect("new recurrence phi");
        let copy = header
            .ops
            .iter()
            .find(|op| op.defines.contains(&product))
            .expect("product copy");
        assert_eq!(copy.kind, Kind::Copy);
        assert_eq!(copy.args, vec![held(carried.result)]);
        let update = header
            .ops
            .iter()
            .find(|op| {
                op.defines
                    .contains(carried.incoming.get(&1).expect("latch phi input"))
            })
            .expect("latch update");
        assert_eq!(update.kind, Kind::Add);
        assert_eq!(update.args[0], held(carried.result));
        assert_eq!(
            header
                .ops
                .last()
                .expect("terminatorless header's final op")
                .kind,
            Kind::Add
        );
    }

    #[test]
    fn direct_strength_initializes_a_unit_multiplier_with_one_copy_operand() {
        // Python `_start` turns multiplication by one into a copy of the
        // recurrence start; retaining the literal one changes its MIR shape.
        let (mut body, _product, _counter) = direct_body();
        body.blocks[1].ops[1].args[1] = Arg::Const(Const::new(1, 2));

        let result = reduced_direct(&body, None).expect("analysis and construction");
        let seed = &result.blocks[0].ops[0];

        assert_eq!(seed.kind, Kind::Copy);
        assert_eq!(seed.args.len(), 1);
        assert!(matches!(seed.args[0], Arg::Held(_)));
    }

    #[test]
    fn direct_strength_refuses_a_live_high_product_result_through_phis() {
        // Port of tests/test_induction_identity.py:test_reduction_preserves_every_live_product_result.
        let (mut body, low, _counter) = direct_body();
        let high = value(20, 1, 9);
        let middle = value(21, 2, 9);
        let final_value = value(22, 3, 9);
        let multiply = &mut body.blocks[1].ops[1];
        multiply.defines.push(high);
        multiply.results.push(held(high));
        let mut middle_incoming = OrderedMap::new();
        middle_incoming.insert(1, high);
        let mut final_incoming = OrderedMap::new();
        final_incoming.insert(2, middle);
        let mut consume = Op::new(3, None, "", vec![], vec![final_value]);
        consume.kind = Kind::Arg;
        consume.args = vec![held(final_value)];
        body.blocks[2] = MirBlock::new(
            2,
            vec![Phi {
                result: middle,
                incoming: middle_incoming,
            }],
            vec![],
            vec![3],
        );
        body.blocks.push(MirBlock::new(
            3,
            vec![Phi {
                result: final_value,
                incoming: final_incoming,
            }],
            vec![consume],
            vec![],
        ));
        let occurrence = crate::analysis::occurrence::operations(&body)
            .nth(1)
            .expect("multiply")
            .0;
        assert_eq!(answer(&body, occurrence), None);
        assert_eq!(reduced_direct(&body, None).expect("analysis"), body);
        assert_ne!(low, high);
    }

    #[test]
    fn direct_strength_refuses_a_second_live_product_result() {
        // The multiply's low result and its second result are both live;
        // one recurrence copy cannot preserve the operation's full answer.
        let (mut body, low, _counter) = direct_body();
        let second = value(30, 1, 11);
        let multiply = &mut body.blocks[1].ops[1];
        multiply.defines.push(second);
        multiply.results.push(held(second));
        let mut consume = Op::new(4, None, "", vec![], vec![low, second]);
        consume.kind = Kind::Arg;
        consume.args = vec![held(low), held(second)];
        body.blocks[1].ops[2] = consume;
        assert_eq!(reduced_direct(&body, None).expect("analysis"), body);
    }

    #[test]
    fn direct_strength_selects_only_the_leaf_of_a_multiply_chain() {
        // Python `_formula_set(..., room=None)` carries the complete leaf
        // formula. Reducing both `i * 3` and `(i * 3) * 5` would invent two
        // counters for one result chain.
        let (mut body, product, _counter) = direct_body();
        let leaf = value(5, 1, 3);
        let mut chained = Op::new(3, None, "", vec![leaf], vec![product]);
        chained.kind = Kind::Mul;
        chained.args = vec![held(product), Arg::Const(Const::new(5, 2))];
        chained.results = vec![held(leaf)];
        let mut consume = Op::new(4, None, "", vec![], vec![leaf]);
        consume.kind = Kind::Arg;
        consume.args = vec![held(leaf)];
        let step = body.blocks[1].ops[0].clone();
        let parent = body.blocks[1].ops[1].clone();
        body.blocks[1].ops = vec![step, parent, chained, consume];

        let result = reduced_direct(&body, None).expect("analysis and construction");
        let header = &result.blocks[1];
        assert_eq!(
            header
                .ops
                .iter()
                .find(|op| op.defines.contains(&product))
                .expect("unselected parent")
                .kind,
            Kind::Mul
        );
        assert_eq!(
            header
                .ops
                .iter()
                .find(|op| op.defines.contains(&leaf))
                .expect("selected leaf")
                .kind,
            Kind::Copy
        );
        assert_eq!(
            header
                .phis
                .iter()
                .filter(|phi| phi.result.variable != 1)
                .count(),
            1
        );
    }

    #[test]
    fn direct_strength_refuses_a_preheader_bypass() {
        // Port of tests/test_induction_identity.py:test_reduction_does_not_speculate_on_a_loop_bypass.
        let (mut body, _product, _counter) = direct_body();
        body.blocks[0].succ = vec![1, 2];
        assert_eq!(reduced_direct(&body, None).expect("analysis"), body);
    }

    #[test]
    fn direct_strength_copies_the_current_iteration_value_for_an_exit_phi() {
        // Port of tests/test_induction_identity.py:test_reduced_product_keeps_the_current_iteration_on_exit.
        let (mut body, product, counter) = direct_body();
        body.blocks[1].ops.remove(2);
        let exit_value = value(40, 2, 10);
        let mut incoming = OrderedMap::new();
        incoming.insert(1, product);
        let mut consume = Op::new(5, None, "", vec![], vec![exit_value]);
        consume.kind = Kind::Arg;
        consume.args = vec![held(exit_value)];
        body.blocks[2] = MirBlock::new(
            2,
            vec![Phi {
                result: exit_value,
                incoming,
            }],
            vec![consume],
            vec![],
        );
        let result = reduced_direct(&body, None).expect("analysis and construction");
        let header = &result.blocks[1];
        let carried = header
            .phis
            .iter()
            .find(|phi| phi.result != counter)
            .expect("carried phi");
        let copy = header
            .ops
            .iter()
            .find(|op| op.defines.contains(&product))
            .expect("source answer remains available");
        assert_eq!(copy.kind, Kind::Copy);
        assert_eq!(copy.args, vec![held(carried.result)]);
        assert_eq!(result.blocks[2].phis[0].incoming.get(&1), Some(&product));
    }

    #[test]
    fn direct_strength_owns_insertions_at_the_preheader_and_latch_locations() {
        // Port of tests/test_induction_identity.py:test_inserted_counter_operations_own_their_insertion_location.
        let (body, _product, _counter) = direct_body();
        let result = reduced_direct(&body, None).expect("analysis and construction");
        assert_eq!(result.blocks[0].ops[0].at, 0);
        let update = result.blocks[1].ops.last().expect("latch update");
        assert_eq!(update.kind, Kind::Add);
        assert_eq!(update.at, 3);
        assert!(result.blocks[0].ops[0].absorbed.is_empty());
        assert!(update.absorbed.is_empty());
    }
}
