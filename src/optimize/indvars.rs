//! Replace a proved counted-loop control value with a zero-ending recurrence.
//!
//! Direct port of `qbopt/optimize/indvars.py:symbolically_zeroed`,
//! `_before_leaving`, and `_SeedBuilder`.  The proof belongs to
//! `analysis::induction`; this module only reconstructs the exact MIR rewrite.

use std::collections::{BTreeMap, BTreeSet};

use num_bigint::{BigInt, Sign};

use crate::analysis::loops;
use crate::analysis::ssa::SubstitutionError;
use crate::analysis::{consts, induction, occurrence, ssa};
use crate::model::mir::{
    self, Arg, Const, Held, Kind, MirBlock, MirBody, Op, OrderedMap, Phi, Value,
};

use super::rotate;

/// Recover a Python operation object's snapshot-local occurrence key.
fn op_occurrence(
    body: &MirBody,
    block_index: usize,
    operation_index: usize,
) -> occurrence::OpOccurrence {
    occurrence::operations(body)
        .find(|(at, _, _)| {
            at.block_index() == block_index && at.operation_index() == operation_index
        })
        .map(|(at, _, _)| at)
        .expect("operation ordinal belongs to the immutable MIR snapshot")
}

/// Recover a Python phi object's snapshot-local occurrence key.
fn phi_occurrence(
    body: &MirBody,
    block_index: usize,
    phi_index: usize,
) -> occurrence::PhiOccurrence {
    occurrence::phis(body)
        .find(|(at, _, _)| at.block_index() == block_index && at.phi_index() == phi_index)
        .map(|(at, _, _)| at)
        .expect("phi ordinal belongs to the immutable MIR snapshot")
}

/// Python's `_before_leaving`.
fn before_leaving(ops: &mut Vec<Op>, inserted: Vec<Op>) {
    let cut = ops.len()
        - usize::from(
            ops.last()
                .is_some_and(|op| matches!(op.kind, Kind::Jump | Kind::Branch)),
        );
    ops.splice(cut..cut, inserted);
}

/// Python's `_SeedBuilder`.
struct SeedBuilder {
    serial: u32,
    variable: u32,
    at: i64,
    width: u32,
    ops: Vec<Op>,
}

impl SeedBuilder {
    /// Python's `_SeedBuilder.computed`.
    fn computed(&mut self, kind: Kind, args: Vec<Arg>) -> Held {
        let value = Value {
            id: self.serial,
            at: self.at,
            flags: false,
            variable: self.variable,
            version: 0,
        };
        self.serial += 1;
        self.variable += 1;
        self.ops
            .push(mir::computed(self.at, kind, value, args, self.width));
        Held {
            value,
            width: self.width,
        }
    }
}

type Offset = (
    occurrence::OpOccurrence,
    Option<usize>,
    BigInt,
    Option<(Value, Value)>,
    BigInt,
);

/// Convert Python's arbitrary-precision shift count at Rust's allocation boundary.
///
/// `num_bigint` ultimately indexes its backing allocation with `usize`; this
/// retains every nonnegative Python count representable by that allocation
/// instead of imposing the narrower `u32` limit.
fn shift_count(value: &BigInt) -> usize {
    let (sign, digits) = value.to_u64_digits();
    assert!(sign != Sign::Minus, "negative shift count");
    match digits.as_slice() {
        [] => 0,
        [one] => usize::try_from(*one).expect("shift count exceeds allocation capacity"),
        _ => panic!("shift count exceeds allocation capacity"),
    }
}

/// Python's `_offsets`.
///
/// `operation` is the snapshot-local equivalent of Python's operation object
/// identity.  The tuple otherwise retains Python's add/address/equality form.
fn offsets(
    counter: Value,
    readers: &BTreeMap<Value, Vec<occurrence::OpOccurrence>>,
    placed: &BTreeMap<occurrence::OpOccurrence, i64>,
    home: &BTreeMap<Value, i64>,
    inside: &BTreeSet<i64>,
    own: &BTreeSet<occurrence::OpOccurrence>,
    body: &MirBody,
    address_offsets: bool,
) -> Option<Vec<Offset>> {
    let operation =
        |at: occurrence::OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let flagless = |op: &Op| {
        !op.defines
            .iter()
            .any(|value| value.flags && readers.contains_key(value))
    };
    let plain = |op: &Op, kind| {
        op.kind == kind
            && op.loads.is_empty()
            && op.stores.is_empty()
            && op.merges.is_empty()
            && !op.barrier()
            && op.results.len() == 1
            && matches!(op.results[0], Arg::Held(_))
            && flagless(op)
    };
    let added =
        |at: occurrence::OpOccurrence, value: Value, multiplier: BigInt| -> Option<Offset> {
            let op = operation(at);
            if !plain(op, Kind::Add) || op.args.len() != 2 {
                return None;
            }
            let accepted = |arg: &Arg| match arg {
                Arg::Held(_) => true,
                Arg::Const(_) => address_offsets,
                _ => false,
            };
            let width = match op.results[0] {
                Arg::Held(held) => held.width,
                _ => unreachable!("plain checked the sole result"),
            };
            if !op.args.iter().all(|arg| {
                accepted(arg)
                    && match arg {
                        Arg::Held(held) => held.width == width,
                        Arg::Const(constant) => constant.width == width,
                        _ => false,
                    }
            }) {
                return None;
            }
            let counted = op
                .args
                .iter()
                .enumerate()
                .filter_map(|(index, arg)| match arg {
                    Arg::Held(held) if held.value == value => Some(index),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if counted.len() != 1 {
                return None;
            }
            let position = 1 - counted[0];
            if let Arg::Held(invariant) = op.args[position] {
                if home
                    .get(&invariant.value)
                    .is_some_and(|at| inside.contains(at))
                {
                    return None;
                }
            }
            Some((at, Some(position), multiplier, None, BigInt::from(0_u8)))
        };
    let addressed = |at: occurrence::OpOccurrence,
                     value: Value,
                     multiplier: BigInt,
                     replacement: Option<Value>,
                     extra: BigInt|
     -> Option<Offset> {
        let op = operation(at);
        let refs = op
            .loads
            .iter()
            .chain(&op.stores)
            .chain(op.memory_values.iter().map(|(reference, _)| reference))
            .collect::<Vec<_>>();
        let found = refs
            .iter()
            .copied()
            .filter(|reference| reference.base == Some(value))
            .collect::<Vec<_>>();
        if found.is_empty()
            || found
                .iter()
                .any(|reference| reference.addr.is_none() || reference.symbolic.is_some())
            || op
                .args
                .iter()
                .any(|arg| matches!(arg, Arg::Held(held) if held.value == value))
            || refs
                .iter()
                .any(|reference| reference.segment == Some(value))
            || refs
                .iter()
                .any(|reference| reference.base == Some(value) && !found.contains(reference))
        {
            return None;
        }
        Some((
            at,
            None,
            multiplier,
            Some((value, replacement.unwrap_or(value))),
            extra,
        ))
    };
    let derived = |at: occurrence::OpOccurrence, value: Value, multiplier: BigInt| {
        let form = added(at, value, multiplier.clone());
        if form.is_some() || !address_offsets {
            form
        } else {
            addressed(at, value, multiplier, None, BigInt::from(0_u8))
        }
    };
    let equality = |at: occurrence::OpOccurrence, value: Value| -> Option<Offset> {
        let op = operation(at);
        if !address_offsets
            || op.kind != Kind::Sub
            || !op.results.is_empty()
            || !op.loads.is_empty()
            || !op.stores.is_empty()
            || op.barrier()
            || !op.merges.is_empty()
            || op.args.len() != 2
            || op.defines.len() != 1
            || !op.defines[0].flags
            || readers
                .get(&op.defines[0])
                .is_none_or(|users| users.is_empty())
            || readers[&op.defines[0]].iter().any(|reader| {
                let reader = operation(*reader);
                reader.kind != Kind::Branch || !matches!(reader.test, Some(Kind::Eq | Kind::Ne))
            })
        {
            return None;
        }
        let positions = op
            .args
            .iter()
            .enumerate()
            .filter_map(|(index, arg)| match arg {
                Arg::Held(held) if held.value == value => Some(index),
                _ => None,
            })
            .collect::<Vec<_>>();
        if positions.len() != 1 {
            return None;
        }
        let position = 1 - positions[0];
        let other = &op.args[position];
        let counter_width = match op.args[positions[0]] {
            Arg::Held(held) => held.width,
            _ => unreachable!("position was selected from Held"),
        };
        match other {
            Arg::Held(held)
                if held.width == counter_width
                    && !home.get(&held.value).is_some_and(|at| inside.contains(at)) => {}
            Arg::Const(constant) if constant.width == counter_width => {}
            _ => return None,
        }
        Some((
            at,
            Some(position),
            BigInt::from(-1_i8),
            None,
            BigInt::from(0_u8),
        ))
    };

    let mut out = Vec::new();
    for at in readers.get(&counter).into_iter().flatten().copied() {
        if own.contains(&at) || !inside.contains(&placed[&at]) {
            continue;
        }
        let op = operation(at);
        let mut form = address_offsets
            .then(|| addressed(at, counter, BigInt::from(1_u8), None, BigInt::from(0_u8)))
            .flatten();
        let added_form = added(at, counter, BigInt::from(1_u8));
        if address_offsets {
            if let Some((_, Some(position), _, _, _)) = &added_form {
                if let Arg::Const(invariant) = &op.args[*position] {
                    let result = match op.results[0] {
                        Arg::Held(held) => held.value,
                        _ => unreachable!("added checked held result"),
                    };
                    let constant = induction::_signed(
                        &Arg::Const(invariant.clone()),
                        &indexmap::IndexMap::new(),
                        invariant.width,
                    )
                    .expect("a constant has its signed form");
                    let forms = readers
                        .get(&result)
                        .into_iter()
                        .flatten()
                        .copied()
                        .map(|reader| {
                            addressed(
                                reader,
                                result,
                                BigInt::from(1_u8),
                                Some(counter),
                                constant.clone(),
                            )
                        })
                        .collect::<Vec<_>>();
                    if !forms.is_empty() && forms.iter().all(Option::is_some) {
                        out.extend(forms.into_iter().flatten());
                        continue;
                    }
                }
            }
        }
        if form.is_none() {
            form = added_form;
        }
        if form.is_none() {
            form = equality(at, counter);
        }
        let scale = if plain(op, Kind::Shl) && op.args.len() == 2 {
            match (&op.args[0], &op.args[1], &op.results[0]) {
                (Arg::Held(held), Arg::Const(shift), Arg::Held(result))
                    if held.value == counter && held.width == result.width =>
                {
                    Some(BigInt::from(1_u8) << shift_count(&shift.n))
                }
                _ => None,
            }
        } else if address_offsets && plain(op, Kind::Mul) && op.args.len() == 2 {
            let constants = op
                .args
                .iter()
                .filter_map(|arg| match arg {
                    Arg::Const(value) => Some(value),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let held = op
                .args
                .iter()
                .filter_map(|arg| match arg {
                    Arg::Held(value) if value.value == counter => Some(value),
                    _ => None,
                })
                .collect::<Vec<_>>();
            match (&constants[..], &held[..], &op.results[0]) {
                ([constant], [value], Arg::Held(result))
                    if constant.width == value.width && value.width == result.width =>
                {
                    induction::_signed(
                        &Arg::Const((*constant).clone()),
                        &indexmap::IndexMap::new(),
                        constant.width,
                    )
                }
                _ => None,
            }
        } else {
            None
        };
        if form.is_none() {
            if let Some(scale) = scale {
                let shifted = match op.results[0] {
                    Arg::Held(held) => held.value,
                    _ => unreachable!("plain checked held result"),
                };
                let forms = readers
                    .get(&shifted)
                    .into_iter()
                    .flatten()
                    .copied()
                    .map(|reader| derived(reader, shifted, scale.clone()))
                    .collect::<Vec<_>>();
                if !forms.is_empty() && forms.iter().all(Option::is_some) {
                    out.extend(forms.into_iter().flatten());
                    continue;
                }
            }
        }
        let form = form?;
        out.push(form);
    }
    Some(out)
}

/// Use a bounded affine data recurrence as the loop's sole control.
///
/// Direct port of `qbopt/optimize/indvars.py:symbolically_zeroed`.
pub(crate) fn symbolically_zeroed(body: &MirBody) -> Result<MirBody, SubstitutionError> {
    let facts = consts::known(body, None, None, None, None);
    let blocks = body
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.at, index))
        .collect::<BTreeMap<_, _>>();
    let made = occurrence::operations(body)
        .flat_map(|(at, _, op)| op.defines.iter().map(move |value| (*value, at)))
        .collect::<BTreeMap<_, _>>();
    let mut readers = BTreeMap::<Value, Vec<occurrence::OpOccurrence>>::new();
    for (at, _, op) in occurrence::operations(body) {
        for value in &op.uses {
            readers.entry(*value).or_default().push(at);
        }
    }
    let placed = occurrence::operations(body)
        .map(|(at, block, _)| (at, block.at))
        .collect::<BTreeMap<_, _>>();
    let mut home = occurrence::operations(body)
        .flat_map(|(_, block, op)| op.defines.iter().map(move |value| (*value, block.at)))
        .collect::<BTreeMap<_, _>>();
    home.extend(
        body.blocks
            .iter()
            .flat_map(|block| block.phis.iter().map(move |phi| (phi.result, block.at))),
    );
    let values = ssa::values(body).collect::<Vec<_>>();

    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let proofs = induction::counted(body, &loop_, Some(&facts));
        if proofs.len() != 1 {
            continue;
        }
        let proof = &proofs[0];
        let header_index = blocks[&loop_.header];
        let header = &body.blocks[header_index];
        let inside = loop_.body.clone();
        for candidate in induction::basics(body, &loop_).values() {
            let Some(symbolic) = induction::zero_terminating_control(
                body, &loop_, proof, candidate, Some(&facts),
            ) else {
                continue;
            };
            let control = &symbolic.replacement;
            let width = match &symbolic.candidate.start {
                induction::AffineOperand::Held(held) => held.width,
                induction::AffineOperand::Const(constant) => constant.width,
            };
            let Some((phi_index, phi)) = header
                .phis
                .iter()
                .enumerate()
                .find(|(_, phi)| phi.result.id == candidate.value)
            else {
                continue;
            };
            let phi_at = phi_occurrence(body, header_index, phi_index);
            let incoming = phi.incoming.keys().copied().collect::<BTreeSet<_>>();
            if incoming != BTreeSet::from([proof.preheader, proof.latch]) {
                continue;
            }
            let initial = *phi
                .incoming
                .get(&proof.preheader)
                .expect("checked preheader input");
            let update = *phi.incoming.get(&proof.latch).expect("checked latch input");
            let seed = made
                .get(&initial)
                .copied()
                .map(|at| &body.blocks[at.block_index()].ops[at.operation_index()]);
            let stepping = made.get(&update).copied();
            let Some(seed) = seed else {
                continue;
            };
            let Some(stepping_at) = stepping else {
                continue;
            };
            let stepping =
                &body.blocks[stepping_at.block_index()].ops[stepping_at.operation_index()];
            if seed.kind != Kind::Copy
                || seed.args.len() != 1
                || stepping.results.is_empty()
                || !matches!(stepping.results[0], Arg::Held(_))
                || !stepping.loads.is_empty()
                || !stepping.stores.is_empty()
                || stepping.barrier()
                || !stepping.merges.is_empty()
            {
                continue;
            }
            let offsets = offsets(
                phi.result,
                &readers,
                &placed,
                &home,
                &inside,
                &BTreeSet::from([stepping_at]),
                body,
                true,
            );
            let Some(offsets) = offsets else {
                continue;
            };
            if offsets.is_empty()
                || offsets
                    .iter()
                    .any(|(_, position, _, _, _)| position.is_none())
            {
                continue;
            }
            let read = body
                .blocks
                .iter()
                .flat_map(|block| &block.ops)
                .flat_map(|op| op.uses.iter().copied())
                .collect::<BTreeSet<_>>();
            if read.contains(&initial)
                || read.contains(&update)
                || stepping
                    .defines
                    .iter()
                    .any(|value| value.flags && read.contains(value))
            {
                continue;
            }

            let ending = body.blocks[blocks[&proof.preheader]].ops.last().unwrap_or(
                &body.blocks[proof.compare.block_index()].ops[proof.compare.operation_index()],
            );
            let mut builder = SeedBuilder {
                serial: values.iter().map(|value| value.id).max().unwrap_or(0) + 1,
                variable: values.iter().map(|value| value.variable).max().unwrap_or(0) + 1,
                at: ending.at,
                width,
                ops: Vec::new(),
            };
            let count = match &proof.bound {
                induction::AffineOperand::Held(held) => Arg::Held(*held),
                induction::AffineOperand::Const(constant) => Arg::Const(constant.clone()),
            };
            let distance = if symbolic.step == BigInt::from(1_u8) {
                count.clone()
            } else {
                Arg::Held(builder.computed(
                    Kind::Mul,
                    vec![
                        count.clone(),
                        Arg::Const(Const::new(consts::masked(&symbolic.step, width), width)),
                    ],
                ))
            };
            let mut rebased = BTreeMap::<occurrence::OpOccurrence, Op>::new();
            for (at, position, multiplier, _, _) in offsets {
                let position = position.expect("symbolically_zeroed refused address-only offsets");
                let op = &body.blocks[at.block_index()].ops[at.operation_index()];
                let base = op.args[position].clone();
                let delta = if multiplier == BigInt::from(1_u8) {
                    distance.clone()
                } else {
                    Arg::Held(builder.computed(
                        Kind::Mul,
                        vec![
                            distance.clone(),
                            Arg::Const(Const::new(consts::masked(&multiplier, width), width)),
                        ],
                    ))
                };
                let adjusted = builder.computed(Kind::Add, vec![base.clone(), delta]);
                let mut replacement = op.clone();
                replacement.args = op
                    .args
                    .iter()
                    .enumerate()
                    .map(|(index, arg)| {
                        if index == position {
                            Arg::Held(adjusted)
                        } else {
                            arg.clone()
                        }
                    })
                    .collect();
                if !matches!(base, Arg::Held(_) | Arg::Const(_)) {
                    unreachable!("offsets only returns held or constants");
                }
                replacement.uses = op
                    .uses
                    .iter()
                    .map(|value| match &base {
                        Arg::Held(base) if *value == base.value => adjusted.value,
                        _ => *value,
                    })
                    .collect();
                replacement.source_backed = false;
                replacement.raised = None;
                rebased.insert(at, replacement);
            }
            let source = seed.args[0].clone();
            let begun = builder.computed(Kind::Sub, vec![source, distance.clone()]);
            let step_flags = Value {
                id: builder.serial,
                at: stepping.at,
                flags: true,
                variable: builder.variable,
                version: 1,
            };
            let guard_flags = Value {
                id: builder.serial + 1,
                at: ending.at,
                flags: true,
                variable: builder.variable + 1,
                version: 1,
            };
            let mut decrement = stepping.clone();
            decrement.name.clear();
            decrement.defines = stepping
                .defines
                .iter()
                .copied()
                .filter(|value| !value.flags)
                .chain(std::iter::once(step_flags))
                .collect();
            decrement.source_backed = false;
            decrement.raised = None;
            decrement.symbol = Some(false);
            let compare =
                &body.blocks[proof.compare.block_index()].ops[proof.compare.operation_index()];
            let mut guard_compare = compare.clone();
            guard_compare.at = ending.at;
            guard_compare.defines = vec![guard_flags];
            guard_compare.uses = match &proof.bound {
                induction::AffineOperand::Held(held) => vec![held.value],
                induction::AffineOperand::Const(_) => vec![],
            };
            guard_compare.source_backed = false;
            guard_compare.args = vec![count.clone(), Arg::Const(Const::new(0, width))];
            guard_compare.raised = None;
            guard_compare.absorbed.clear();
            guard_compare.symbol = Some(false);
            let branch =
                &body.blocks[proof.branch.block_index()].ops[proof.branch.operation_index()];
            let mut guard_branch = branch.clone();
            guard_branch.at = ending.at;
            guard_branch.name.clear();
            guard_branch.defines.clear();
            guard_branch.uses = vec![guard_flags];
            guard_branch.source_backed = false;
            guard_branch.test = Some(Kind::Eq);
            guard_branch.target = Some(proof.exit);
            guard_branch.raised = None;
            guard_branch.absorbed.clear();
            guard_branch.symbol = Some(false);

            let private = [
                initial,
                *body.blocks[proof.phi.block_index()].phis[proof.phi.phi_index()]
                    .incoming
                    .get(&proof.preheader)
                    .expect("counted proof phi has preheader input"),
            ]
            .into_iter()
            .filter_map(|value| {
                let definition = made.get(&value).copied()?;
                (!body
                    .blocks
                    .iter()
                    .flat_map(|block| &block.ops)
                    .any(|op| op.uses.contains(&value))
                    && !occurrence::phis(body).any(|(other, _, phi)| {
                        other != phi_at
                            && other != proof.phi
                            && phi.incoming.values().any(|incoming| *incoming == value)
                    }))
                .then_some(definition)
            })
            .collect::<Vec<_>>();
            let mut entry_ops = body.blocks[blocks[&proof.preheader]]
                .ops
                .iter()
                .enumerate()
                .map(|(index, op)| {
                    let at = op_occurrence(body, blocks[&proof.preheader], index);
                    if private.contains(&at) {
                        mir::cleared(op)
                    } else {
                        op.clone()
                    }
                })
                .collect::<Vec<_>>();
            if let Some(last) = entry_ops.last_mut() {
                if last.kind == Kind::Jump {
                    *last = mir::cleared(last);
                } else if last.kind == Kind::Branch {
                    continue;
                }
            }
            before_leaving(&mut entry_ops, builder.ops);
            entry_ops.extend([guard_compare, guard_branch]);

            let rewritten = body
                .blocks
                .iter()
                .enumerate()
                .map(|(block_index, block)| {
                    let mut ops = Vec::new();
                    for (operation_index, op) in block.ops.iter().enumerate() {
                        let at = op_occurrence(body, block_index, operation_index);
                        if at == stepping_at || at == control.stepping {
                            continue;
                        }
                        let op = if at == proof.compare || private.contains(&at) {
                            mir::cleared(op)
                        } else if at == proof.branch {
                            let mut branch = op.clone();
                            branch.name.clear();
                            branch.uses = vec![step_flags];
                            branch.source_backed = false;
                            branch.test = Some(Kind::Ne);
                            branch.target = Some(proof.latch);
                            branch.raised = None;
                            branch.symbol = Some(false);
                            branch
                        } else {
                            rebased.get(&at).cloned().unwrap_or_else(|| op.clone())
                        };
                        ops.push(op);
                    }
                    if block.at == proof.latch {
                        let cut = ops.len()
                            - usize::from(ops.last().is_some_and(|op| op.kind == Kind::Jump));
                        ops.insert(cut, decrement.clone());
                    }
                    let phis = if block.at == loop_.header {
                        block
                            .phis
                            .iter()
                            .enumerate()
                            .filter_map(|(index, other)| {
                                let at = phi_occurrence(body, block_index, index);
                                if at == proof.phi {
                                    None
                                } else if at == phi_at {
                                    Some(Phi {
                                        result: other.result,
                                        incoming: OrderedMap::from_iter([
                                            (proof.preheader, begun.value),
                                            (proof.latch, update),
                                        ]),
                                    })
                                } else {
                                    Some(other.clone())
                                }
                            })
                            .collect()
                    } else {
                        block.phis.clone()
                    };
                    MirBlock {
                        at: block.at,
                        phis,
                        ops,
                        succ: block.succ.clone(),
                    }
                })
                .collect::<Vec<_>>();
            let changed = MirBody {
                blocks: rewritten,
                ..body.clone()
            };
            let changed_header = changed
                .block(loop_.header)
                .expect("symbolic header remains in reconstructed body");
            let changed_latch = changed
                .block(proof.latch)
                .expect("symbolic latch remains in reconstructed body");
            let rotated = rotate::at_body(
                &changed,
                &loop_,
                proof.preheader,
                changed_header,
                changed_latch,
                &entry_ops,
                Some(&[proof.latch, proof.exit]),
            )?;
            return symbolically_zeroed(&rotated);
        }
    }
    Ok(body.clone())
}

#[cfg(test)]
mod tests {
    use num_bigint::BigInt;

    use crate::analysis::induction;
    use crate::analysis::loops;
    use crate::model::mir::{
        Arg, Cell, Const, Held, IntegerRange, Kind, MemRef, MirBlock, MirBody, Op, OrderedMap, Phi,
        Value,
    };
    use crate::objectfile::module::Space;

    use super::{shift_count, symbolically_zeroed};

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn shift_counts_are_not_narrowed_to_u32() {
        // Regression for the first Rust port: Python accepts arbitrary-size
        // integer counts, but the port rejected every value above u32::MAX.
        let beyond_u32 = BigInt::from(u64::from(u32::MAX) + 1);
        assert_eq!(shift_count(&beyond_u32), u32::MAX as usize + 1);
    }

    /// Direct port of `tests/test_indvars.py:_symbolic_control_body`.
    fn symbolic_control_body(candidate_start: i64) -> MirBody {
        let bound = Value {
            id: 1,
            at: 0,
            flags: false,
            variable: 1,
            version: 1,
        };
        let control_seed = Value {
            id: 2,
            at: 0,
            flags: false,
            variable: 2,
            version: 1,
        };
        let candidate_seed = Value {
            id: 3,
            at: 0,
            flags: false,
            variable: 3,
            version: 1,
        };
        let control = Value {
            id: 4,
            at: 1,
            flags: false,
            variable: 2,
            version: 2,
        };
        let candidate = Value {
            id: 5,
            at: 1,
            flags: false,
            variable: 3,
            version: 2,
        };
        let flags = Value {
            id: 6,
            at: 1,
            flags: true,
            variable: 4,
            version: 1,
        };
        let control_next = Value {
            id: 7,
            at: 2,
            flags: false,
            variable: 2,
            version: 3,
        };
        let candidate_next = Value {
            id: 8,
            at: 2,
            flags: false,
            variable: 3,
            version: 3,
        };
        let offset = Value {
            id: 9,
            at: 2,
            flags: false,
            variable: 5,
            version: 1,
        };
        let mut source = MemRef::new(None, 2);
        source.space = Some(Space::Frame);

        let copy = |at, result, number| {
            let mut op = Op::new(at, None, "", vec![result], vec![]);
            op.kind = Kind::Copy;
            op.args = vec![Arg::Const(Const::new(number, 2))];
            op.results = vec![Arg::Held(Held {
                value: result,
                width: 2,
            })];
            op
        };
        let add = |at, result, left, right| {
            let mut op = Op::new(at, None, "", vec![result], vec![left]);
            op.kind = Kind::Add;
            op.args = vec![
                Arg::Held(Held {
                    value: left,
                    width: 2,
                }),
                Arg::Const(Const::new(right, 2)),
            ];
            op.results = vec![Arg::Held(Held {
                value: result,
                width: 2,
            })];
            op
        };
        let mut load = Op::new(0, None, "", vec![bound], vec![]);
        load.loads = vec![source.clone()];
        load.kind = Kind::Load;
        load.args = vec![Arg::Cell(Cell { r#ref: source })];
        load.results = vec![Arg::Held(Held {
            value: bound,
            width: 2,
        })];

        let mut compare = Op::new(1, None, "cmp", vec![flags], vec![control, bound]);
        compare.kind = Kind::Sub;
        compare.args = vec![
            Arg::Held(Held {
                value: control,
                width: 2,
            }),
            Arg::Held(Held {
                value: bound,
                width: 2,
            }),
        ];
        let mut branch = Op::new(1, None, "", vec![], vec![flags]);
        branch.kind = Kind::Branch;
        branch.test = Some(Kind::AboveEq);
        branch.target = Some(3);
        let mut jump = Op::new(2, None, "", vec![], vec![]);
        jump.kind = Kind::Jump;
        jump.target = Some(1);
        let mut returned = Op::new(3, None, "", vec![], vec![]);
        returned.kind = Kind::Return;

        let mut body = MirBody::new(
            0,
            vec![
                MirBlock::new(
                    0,
                    vec![],
                    vec![
                        load,
                        copy(0, control_seed, 0),
                        copy(0, candidate_seed, candidate_start),
                    ],
                    vec![1],
                ),
                MirBlock::new(
                    1,
                    vec![
                        Phi {
                            result: control,
                            incoming: OrderedMap::from_iter([(0, control_seed), (2, control_next)]),
                        },
                        Phi {
                            result: candidate,
                            incoming: OrderedMap::from_iter([
                                (0, candidate_seed),
                                (2, candidate_next),
                            ]),
                        },
                    ],
                    vec![compare, branch],
                    vec![2, 3],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![
                        add(2, offset, candidate, 100),
                        add(2, control_next, control, 1),
                        add(2, candidate_next, candidate, 1),
                        jump,
                    ],
                    vec![1],
                ),
                MirBlock::new(3, vec![], vec![returned], vec![]),
            ],
        );
        body.integer_ranges = OrderedMap::from_iter([(bound, IntegerRange::new(0, 7, 2))]);
        body
    }

    #[test]
    fn symbolic_control_refuses_a_nonzero_terminal_recurrence() {
        // `tests/test_indvars.py:test_symbolic_control_refuses_a_nonzero_terminal_recurrence`:
        // a seed of 5 would run the replacement until wraparound.
        let body = symbolic_control_body(5);
        let mut found = loops::loops(&body.blocks, Some(body.entry));
        let loop_ = found.remove(0);
        let proof = induction::counted(&body, &loop_, None).remove(0);
        let basics = induction::basics(&body, &loop_);
        let candidate = basics
            .values()
            .find(|candidate| *candidate != &proof.counter)
            .expect("the second affine recurrence exists");

        assert!(induction::zero_terminating_control(&body, &loop_, &proof, candidate, None).is_none());
        assert_eq!(symbolically_zeroed(&body).unwrap(), body);
    }

    #[test]
    fn symbolic_control_proves_a_zero_terminal_recurrence() {
        // `tests/test_indvars.py:test_symbolic_control_proves_a_zero_terminal_recurrence`.
        let body = symbolic_control_body(0);
        let mut found = loops::loops(&body.blocks, Some(body.entry));
        let loop_ = found.remove(0);
        let proof = induction::counted(&body, &loop_, None).remove(0);
        let basics = induction::basics(&body, &loop_);
        let candidate = basics
            .values()
            .find(|candidate| *candidate != &proof.counter)
            .expect("the second affine recurrence exists");

        let proven = induction::zero_terminating_control(&body, &loop_, &proof, candidate, None)
            .expect("zero-seeded recurrence has the proven final zero");
        assert_eq!(proven.replacement.counted, &proof);
        assert_eq!(proven.candidate, *candidate);
        assert_eq!(proven.step, 1.into());
        assert_eq!(proven.maximum, 7.into());
        assert_eq!(proven.period, 65_536.into());
        let changed = symbolically_zeroed(&body).unwrap();
        assert_ne!(changed, body);

        // The preheader receives the symbolic seed and a zero-trip guard,
        // then `at_body` enters straight at the old latch.  The old header
        // test becomes the backedge driven by the candidate step flags.
        let preheader = changed.block(0).expect("preheader remains");
        assert_eq!(preheader.succ, vec![2, 3]);
        assert_eq!(preheader.ops[preheader.ops.len() - 2].kind, Kind::Sub);
        let guard = preheader.ops.last().expect("guard branch follows compare");
        assert_eq!(guard.kind, Kind::Branch);
        assert_eq!(guard.test, Some(Kind::Eq));
        assert_eq!(guard.target, Some(3));

        let header = changed.block(1).expect("old header remains as latch test");
        assert!(header.phis.is_empty());
        let backedge = header.ops.last().expect("old branch remains as backedge");
        assert_eq!(backedge.kind, Kind::Branch);
        assert_eq!(backedge.test, Some(Kind::Ne));
        assert_eq!(backedge.target, Some(2));

        let latch = changed.block(2).expect("old latch receives moved phis");
        assert_eq!(latch.phis.len(), 1);
        let step = latch
            .ops
            .iter()
            .find(|op| op.defines.iter().any(|value| value.flags))
            .expect("candidate step now defines the backedge flags");
        let step_flags = step
            .defines
            .iter()
            .copied()
            .find(|value| value.flags)
            .expect("step flags exist");
        assert_eq!(backedge.uses, vec![step_flags]);
    }
}
