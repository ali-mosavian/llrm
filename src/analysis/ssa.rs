//! SSA use relations over source-neutral MIR.
//!
//! Direct port of `qbopt.analysis.ssa:use_index`.  Python returns operation
//! objects, whose identity distinguishes otherwise equal operations.  Rust
//! returns snapshot-local [`OpOccurrence`] keys for that same relation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::occurrence::{OpOccurrence, operations};
use crate::model::ir::Operation;
use crate::model::mir::{
    Arg, Cell, Held, MemRef, MirBody, Op, OpCode, OrderedMap, Value, consumed as operation_consumed,
};

/// A substitution followed an id-keyed cycle.
///
/// Direct port of `qbopt.analysis.ssa:provider`'s
/// `ValueError("cyclic value substitution")`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SubstitutionError {
    CyclicValueSubstitution,
}

impl fmt::Display for SubstitutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CyclicValueSubstitution => formatter.write_str("cyclic value substitution"),
        }
    }
}

impl std::error::Error for SubstitutionError {}

/// SSA reconstruction could not rebuild the selected variable namespace.
///
/// Direct port of Python `ssa.constructed` propagating `mir.resolved`'s
/// `Unraisable` failure and `ssa.substituted`'s cyclic-substitution failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ConstructionError {
    Resolution(String),
    Substitution(SubstitutionError),
}

impl fmt::Display for ConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolution(message) => formatter.write_str(message),
            Self::Substitution(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ConstructionError {}

impl From<SubstitutionError> for ConstructionError {
    fn from(error: SubstitutionError) -> Self {
        Self::Substitution(error)
    }
}

/// Each value's operation users, built in one body traversal.
///
/// `Op.uses` includes carried merge inputs; [`operation_consumed`] is the
/// narrower set an operation actually reads.  Callers choose the relation
/// they mean, while construction and body/block/operation ordering stay one
/// shared mechanism.  Each operation occurs at most once in a value's list.
///
/// Direct port of `qbopt.analysis.ssa:use_index`.
pub(crate) fn use_index(
    body: &MirBody,
    values: Option<&BTreeSet<Value>>,
    consumed: bool,
) -> BTreeMap<Value, Vec<OpOccurrence>> {
    let mut users = BTreeMap::<Value, Vec<OpOccurrence>>::new();
    for (occurrence, _, operation) in operations(body) {
        let mut read = if consumed {
            operation_consumed(operation)
        } else {
            operation.uses.iter().copied().collect()
        };
        if let Some(wanted) = values {
            read.retain(|value| wanted.contains(value));
        }
        for value in read {
            users.entry(value).or_default().push(occurrence);
        }
    }
    users
}

/// Follow an id-keyed substitution until its provider is unchanged.
///
/// Direct port of `qbopt.analysis.ssa:provider`.
pub(crate) fn provider(
    value: Value,
    swap: &BTreeMap<u32, Value>,
) -> Result<Value, SubstitutionError> {
    let mut value = value;
    let mut seen = BTreeSet::new();
    while let Some(replacement) = swap.get(&value.id) {
        if *replacement == value {
            break;
        }
        if !seen.insert(value.id) {
            return Err(SubstitutionError::CyclicValueSubstitution);
        }
        value = *replacement;
    }
    Ok(value)
}

/// Replace operation reads and memory-address dependencies through `swap`.
///
/// Definitions and held results are deliberately not rewritten.  Merge
/// targets remain untouched: only their source keys are ordinary uses.
/// Direct port of `qbopt.analysis.ssa:substituted`.
pub(crate) fn substituted(op: &Op, swap: &BTreeMap<u32, Value>) -> Result<Op, SubstitutionError> {
    if swap.is_empty() {
        return Ok(op.clone());
    }

    fn reference(
        reference: &MemRef,
        swap: &BTreeMap<u32, Value>,
    ) -> Result<MemRef, SubstitutionError> {
        let mut reference = reference.clone();
        reference.base = reference
            .base
            .map(|value| provider(value, swap))
            .transpose()?;
        reference.segment = reference
            .segment
            .map(|value| provider(value, swap))
            .transpose()?;
        Ok(reference)
    }

    fn operand(argument: &Arg, swap: &BTreeMap<u32, Value>) -> Result<Arg, SubstitutionError> {
        match argument {
            Arg::Held(Held { value, width }) => Ok(Arg::Held(Held {
                value: provider(*value, swap)?,
                width: *width,
            })),
            Arg::Cell(Cell { r#ref }) => Ok(Arg::Cell(Cell {
                r#ref: reference(r#ref, swap)?,
            })),
            _ => Ok(argument.clone()),
        }
    }

    let mut substituted = op.clone();
    substituted.uses = op
        .uses
        .iter()
        .copied()
        .map(|value| provider(value, swap))
        .collect::<Result<_, _>>()?;
    substituted.exits = op
        .exits
        .iter()
        .copied()
        .map(|value| provider(value, swap))
        .collect::<Result<_, _>>()?;
    substituted.args = op
        .args
        .iter()
        .map(|argument| operand(argument, swap))
        .collect::<Result<_, _>>()?;
    substituted.results = op
        .results
        .iter()
        .map(|result| match result {
            Arg::Cell(_) => operand(result, swap),
            _ => Ok(result.clone()),
        })
        .collect::<Result<_, _>>()?;
    substituted.loads = op
        .loads
        .iter()
        .map(|one| reference(one, swap))
        .collect::<Result<_, _>>()?;
    substituted.stores = op
        .stores
        .iter()
        .map(|one| reference(one, swap))
        .collect::<Result<_, _>>()?;
    substituted.memory_values = op
        .memory_values
        .iter()
        .map(|(one, known)| Ok((reference(one, swap)?, known.clone())))
        .collect::<Result<_, _>>()?;
    substituted.merges = op
        .merges
        .iter()
        .map(|(source, target)| Ok((provider(*source, swap)?, *target)))
        .collect::<Result<_, _>>()?;
    Ok(substituted)
}

/// Every value mentioned by a body, in its source declaration order.
///
/// Direct port of `qbopt.analysis.ssa:values`.
pub(crate) fn values(body: &MirBody) -> impl Iterator<Item = Value> + '_ {
    body.blocks.iter().flat_map(|block| {
        block
            .ops
            .iter()
            .flat_map(|op| op.defines.iter().chain(&op.uses).chain(&op.exits).copied())
            .chain(
                block.phis.iter().flat_map(|phi| {
                    std::iter::once(phi.result).chain(phi.incoming.values().copied())
                }),
            )
    })
}

/// Drop phis nothing needs, including cycles only other dead phis read.
///
/// Direct port of `qbopt.analysis.ssa:pruned_phis`.
pub(crate) fn pruned_phis(body: &MirBody, roots: &BTreeSet<Value>) -> MirBody {
    let mut needed: BTreeSet<Value> = roots | &crate::model::mir::exposed(body);
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        needed.extend(operation_consumed(op));
        for reference in op.loads.iter().chain(&op.stores) {
            needed.extend(reference.base.iter().chain(&reference.segment).copied());
        }
    }
    let phis: BTreeMap<Value, &crate::model::mir::Phi> =
        body.blocks.iter().flat_map(|block| &block.phis).map(|phi| (phi.result, phi)).collect();
    let mut pending: Vec<Value> = needed.iter().filter(|value| phis.contains_key(value)).copied().collect();
    while let Some(next) = pending.pop() {
        let incoming: BTreeSet<Value> =
            phis[&next].incoming.values().filter(|value| !needed.contains(value)).copied().collect();
        needed.extend(incoming.iter().copied());
        pending.extend(incoming.into_iter().filter(|value| phis.contains_key(value)));
    }
    let removed: BTreeSet<Value> = phis.keys().filter(|value| !needed.contains(value)).copied().collect();
    if removed.is_empty() {
        return body.clone();
    }
    let mut body = body.clone();
    for block in &mut body.blocks {
        block.phis.retain(|phi| !removed.contains(&phi.result));
        for op in &mut block.ops {
            op.uses.retain(|value| !removed.contains(value));
            op.merges = op.merges.iter().filter(|(source, _)| !removed.contains(source)).map(|(&s, &t)| (s, t)).collect();
        }
    }
    body
}

/// Reconstruct SSA for only the supplied variable names.
///
/// This is the direct Rust port of `qbopt.analysis.ssa:constructed`.  The
/// isolated skeleton lets the shared MIR renamer place phis and versions;
/// the second half merges those names back into the original operations.
/// Unreachable byte-owning blocks remain byte-for-byte present.
pub(crate) fn constructed(
    body: &MirBody,
    variables: &BTreeSet<u32>,
) -> Result<MirBody, ConstructionError> {
    let owned = |values: &[Value]| {
        values
            .iter()
            .copied()
            .filter(|value| variables.contains(&value.variable))
            .collect::<Vec<_>>()
    };

    let mut edges = BTreeMap::<i64, OrderedMap<u32, Value>>::new();
    for block in &body.blocks {
        for phi in &block.phis {
            for (predecessor, value) in phi.incoming.iter() {
                if variables.contains(&value.variable) {
                    edges
                        .entry(*predecessor)
                        .or_default()
                        .insert(value.variable, *value);
                }
            }
        }
    }

    // Phi inputs are reads at the predecessor's end, not at the merge block.
    let probes = edges
        .iter()
        .map(|(at, names)| {
            (
                *at,
                Op::new(
                    *at,
                    OpCode::Operation(Operation::Move),
                    "",
                    Vec::new(),
                    names.values().copied().collect(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut skeleton = body.clone();
    for block in &mut skeleton.blocks {
        block.phis.clear();
        for operation in &mut block.ops {
            operation.defines = owned(&operation.defines);
            operation.uses = owned(&operation.uses);
            operation.exits = owned(&operation.exits);
            operation.args.clear();
            operation.results.clear();
            operation.loads.clear();
            operation.stores.clear();
            operation.merges = OrderedMap::new();
            operation.raised = None;
        }
        if let Some(probe) = probes.get(&block.at) {
            block.ops.push(probe.clone());
        }
    }

    let mut repaired =
        crate::model::mir::resolved(&skeleton, None).map_err(ConstructionError::Resolution)?;

    // The isolated renamer starts ids at zero; keep its namespace disjoint.
    let offset = values(body).map(|value| value.id).max().unwrap_or(0) + 1;
    let mapping = values(&repaired)
        .map(|value| {
            (
                value,
                Value {
                    id: value.id + offset,
                    ..value
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    for block in &mut repaired.blocks {
        for phi in &mut block.phis {
            phi.result = mapping[&phi.result];
            phi.incoming = phi
                .incoming
                .iter()
                .map(|(at, value)| (*at, mapping[value]))
                .collect();
        }
        for operation in &mut block.ops {
            operation.uses = operation.uses.iter().map(|value| mapping[value]).collect();
            operation.exits = operation.exits.iter().map(|value| mapping[value]).collect();
            operation.defines = operation
                .defines
                .iter()
                .map(|value| mapping[value])
                .collect();
        }
    }

    let outgoing = repaired
        .blocks
        .iter()
        .filter(|block| probes.contains_key(&block.at))
        .filter_map(|block| {
            block.ops.last().map(|probe| {
                (
                    block.at,
                    probe
                        .uses
                        .iter()
                        .map(|value| (value.variable, *value))
                        .collect::<BTreeMap<_, _>>(),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let repaired_by_at = repaired
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();

    let mut result = body.clone();
    for block in &mut result.blocks {
        let Some(fixed) = repaired_by_at.get(&block.at) else {
            continue;
        };
        for phi in &mut block.phis {
            phi.incoming = phi
                .incoming
                .iter()
                .map(|(at, value)| {
                    let replacement = outgoing
                        .get(at)
                        .and_then(|names| names.get(&value.variable))
                        .copied()
                        .unwrap_or(*value);
                    (*at, replacement)
                })
                .collect();
        }
        block.phis.extend(fixed.phis.iter().cloned());

        for (operation, renamed) in block.ops.iter_mut().zip(&fixed.ops) {
            let uses = renamed
                .uses
                .iter()
                .map(|value| (value.variable, *value))
                .collect::<BTreeMap<_, _>>();
            let defines = renamed
                .defines
                .iter()
                .map(|value| (value.variable, *value))
                .collect::<BTreeMap<_, _>>();
            let swaps = operation
                .uses
                .iter()
                .filter_map(|value| {
                    uses.get(&value.variable)
                        .map(|replacement| (value.id, *replacement))
                })
                .collect::<BTreeMap<_, _>>();
            let mut changed = substituted(operation, &swaps)?;
            changed.uses = operation
                .uses
                .iter()
                .map(|value| uses.get(&value.variable).copied().unwrap_or(*value))
                .collect();
            changed.defines = operation
                .defines
                .iter()
                .map(|value| defines.get(&value.variable).copied().unwrap_or(*value))
                .collect();
            changed.results = changed
                .results
                .iter()
                .map(|result| match result {
                    Arg::Held(held) => defines
                        .get(&held.value.variable)
                        .map(|value| {
                            Arg::Held(Held {
                                value: *value,
                                width: held.width,
                            })
                        })
                        .unwrap_or_else(|| result.clone()),
                    _ => result.clone(),
                })
                .collect();
            *operation = changed;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::model::ir::Operation;
    use crate::model::mir::{
        Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value,
    };

    use super::{
        SubstitutionError, constructed, operations, provider, pruned_phis, substituted, use_index, values,
    };

    #[test]
    fn test_unused_phi_cycles_are_pruned_but_real_dependencies_survive() {
        for reader in ["none", "argument", "memory", "root"] {
            let (seed, first, second) = (Value::new(1, 0), Value::new(2, 1), Value::new(3, 1));
            let phis = vec![
                Phi { result: first, incoming: [(0, seed), (1, second)].into_iter().collect() },
                Phi { result: second, incoming: [(0, seed), (1, first)].into_iter().collect() },
            ];
            let mut ops = vec![];
            if reader == "argument" {
                let mut op = Op::new(2, OpCode::Operation(Operation::Push), "push", vec![], vec![first]);
                op.kind = Kind::Arg;
                op.args = vec![Arg::Held(Held { value: first, width: 2 })];
                ops.push(op);
            }
            if reader == "memory" {
                let reference = MemRef { base: Some(first), ..MemRef::new(None, 2) };
                let mut op = Op::new(2, OpCode::Operation(Operation::Move), "mov", vec![], vec![]);
                op.kind = Kind::Load;
                op.loads = vec![reference.clone()];
                op.args = vec![Arg::Cell(Cell { r#ref: reference })];
                ops.push(op);
            }
            let body = MirBody::new(
                0,
                vec![MirBlock::new(0, vec![], vec![], vec![1]), MirBlock::new(1, phis.clone(), ops, vec![1])],
            );
            let roots = if reader == "root" { BTreeSet::from([first]) } else { BTreeSet::new() };
            let result = pruned_phis(&body, &roots);
            assert_eq!(result.blocks[1].phis, if reader == "none" { vec![] } else { phis }, "{reader}");
        }
    }

    fn value(id: u32, at: i64, variable: u32, version: u32) -> Value {
        Value {
            id,
            at,
            flags: false,
            variable,
            version,
        }
    }

    fn copy(at: i64, result: Value, argument: Arg) -> Op {
        let uses = match argument {
            Arg::Held(held) => vec![held.value],
            _ => Vec::new(),
        };
        let mut operation = Op::new(
            at,
            OpCode::Operation(Operation::Move),
            "",
            vec![result],
            uses,
        );
        operation.kind = Kind::Copy;
        operation.args = vec![argument];
        operation.results = vec![Arg::Held(Held {
            value: result,
            width: 2,
        })];
        operation
    }

    /// FPDEEP crashed in SSA repair after CFG cleanup left an unreachable
    /// byte owner.  Direct port of
    /// `tests/test_ssa_unreachable.py:test_reconstruction_matches_blocks_by_identity_not_position`.
    #[test]
    fn constructed_matches_blocks_by_identity_not_position() {
        let original = value(1, 0, 1, 1);
        let define = copy(0, original, Arg::Const(Const::new(7, 2)));
        let mut dead = Op::new(
            5,
            OpCode::Operation(Operation::Nothing),
            "",
            Vec::new(),
            Vec::new(),
        );
        dead.kind = Kind::Nothing;
        let mut read = Op::new(
            10,
            OpCode::Operation(Operation::Push),
            "push",
            Vec::new(),
            vec![original],
        );
        read.kind = Kind::Arg;
        read.args = vec![Arg::Held(Held {
            value: original,
            width: 2,
        })];
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, Vec::new(), vec![define], vec![10]),
                MirBlock::new(5, Vec::new(), vec![dead], Vec::new()),
                MirBlock::new(10, Vec::new(), vec![read], Vec::new()),
            ],
        );

        let result = constructed(&body, &BTreeSet::from([1])).expect("SSA reconstruction");

        assert_eq!(
            result
                .blocks
                .iter()
                .map(|block| block.at)
                .collect::<Vec<_>>(),
            vec![0, 5, 10]
        );
        assert_eq!(result.block(5), body.block(5));
        assert_eq!(
            result.block(10).unwrap().ops[0].uses,
            result.block(0).unwrap().ops[0].defines
        );
        assert_eq!(
            result.block(10).unwrap().ops[0].args[0],
            Arg::Held(Held {
                value: result.block(0).unwrap().ops[0].defines[0],
                width: 2,
            })
        );
    }

    /// Existing phi arms must name the reconstructed predecessor versions,
    /// not the value that happened to be present before reconstruction.
    /// Direct port of `tests/test_induction_identity.py`'s SSA regression.
    #[test]
    fn constructed_repairs_existing_phi_inputs_by_predecessor() {
        let initial = value(10, 0, 7, 0);
        let updated = value(11, 1, 7, 0);
        let joined = value(12, 2, 8, 0);
        let define = copy(0, initial, Arg::Const(Const::new(1, 2)));
        let mut step = copy(
            1,
            updated,
            Arg::Held(Held {
                value: initial,
                width: 2,
            }),
        );
        step.kind = Kind::Add;
        step.args.push(Arg::Const(Const::new(1, 2)));
        let mut incoming = OrderedMap::new();
        incoming.insert(0, initial);
        incoming.insert(1, initial);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, Vec::new(), vec![define], vec![1, 2]),
                MirBlock::new(1, Vec::new(), vec![step], vec![2]),
                MirBlock::new(
                    2,
                    vec![Phi {
                        result: joined,
                        incoming,
                    }],
                    Vec::new(),
                    Vec::new(),
                ),
            ],
        );

        let result = constructed(&body, &BTreeSet::from([7])).expect("SSA reconstruction");
        let phi = &result.blocks[2].phis[0];

        assert_eq!(phi.result, joined);
        assert_eq!(
            phi.incoming.get(&0),
            Some(&result.blocks[0].ops[0].defines[0])
        );
        assert_eq!(
            phi.incoming.get(&1),
            Some(&result.blocks[1].ops[0].defines[0])
        );
        assert_eq!(
            result
                .blocks
                .iter()
                .map(|block| block.ops.len())
                .collect::<Vec<_>>(),
            vec![1, 1, 0]
        );
    }

    /// A carried high half is a raw use but not an operation the instruction
    /// consumes.  Direct Rust port of
    /// `tests/test_sccp.py:test_ssa_use_index_preserves_modes_filtering_and_operation_order`.
    #[test]
    fn use_index_preserves_modes_filtering_and_operation_order() {
        let source = Value {
            variable: 1,
            ..Value::new(1, 0)
        };
        let carried = Value {
            variable: 2,
            ..Value::new(2, 0)
        };
        let first_result = Value {
            variable: 3,
            ..Value::new(3, 0)
        };
        let second_result = Value {
            variable: 4,
            ..Value::new(4, 1)
        };
        let ignored = Value {
            variable: 5,
            ..Value::new(5, 1)
        };
        let mut first = Op::new(0, None, "", vec![first_result], vec![source, carried]);
        first.kind = Kind::Copy;
        first.args = vec![Arg::Held(Held {
            value: source,
            width: 2,
        })];
        first.results = vec![Arg::Held(Held {
            value: first_result,
            width: 2,
        })];
        first.merges.insert(carried, first_result);

        let mut second = Op::new(1, None, "", vec![second_result], vec![source, ignored]);
        second.kind = Kind::Copy;
        second.args = vec![
            Arg::Held(Held {
                value: source,
                width: 2,
            }),
            Arg::Held(Held {
                value: ignored,
                width: 2,
            }),
        ];
        second.results = vec![Arg::Held(Held {
            value: second_result,
            width: 2,
        })];

        let body = MirBody::new(
            0,
            vec![MirBlock::new(0, vec![], vec![first, second], vec![])],
        );
        let occurrences = operations(&body)
            .map(|(occurrence, _, _)| occurrence)
            .collect::<Vec<_>>();
        let wanted = BTreeSet::from([source, carried]);

        let raw = use_index(&body, Some(&wanted), false);
        let consumed = use_index(&body, Some(&wanted), true);

        assert_eq!(
            raw,
            BTreeMap::from([
                (source, vec![occurrences[0], occurrences[1]]),
                (carried, vec![occurrences[0]]),
            ])
        );
        assert_eq!(
            consumed,
            BTreeMap::from([(source, vec![occurrences[0], occurrences[1]])])
        );
    }

    /// Direct Rust port of `tests/test_pointer_memory.py`'s two substitution
    /// regressions: a whole-pointer load and CALL-known memory both retain
    /// the replacement address dependency.
    #[test]
    fn substituted_rewrites_whole_pointer_and_known_memory_cells() {
        let pointer = Value::new(1, 0);
        let replacement = Value::new(9, 4);
        let value = Value::new(2, 0);
        let mut reference = MemRef::new(None, 2);
        reference.base = Some(pointer);
        reference.pointer = true;
        let mut op = Op::new(0, None, "mov", vec![value], vec![pointer]);
        op.kind = Kind::Load;
        op.args = vec![Arg::Cell(Cell {
            r#ref: reference.clone(),
        })];
        op.results = vec![Arg::Held(Held { value, width: 2 })];
        op.loads = vec![reference.clone()];
        op.memory_values = vec![(reference, Const::new(7, 2))];

        let changed = substituted(&op, &BTreeMap::from([(pointer.id, replacement)])).unwrap();
        assert!(changed.loads[0].pointer);
        assert_eq!(changed.loads[0].base, Some(replacement));
        assert_eq!(
            changed.args[0],
            Arg::Cell(Cell {
                r#ref: changed.loads[0].clone()
            })
        );
        assert_eq!(changed.uses, vec![replacement]);
        assert_eq!(changed.loads[0].segment, None);
        assert_eq!(changed.memory_values[0].0.base, Some(replacement));
    }

    #[test]
    fn substituted_rewrites_exactly_python_ssa_uses() {
        let old = Value::new(1, 0);
        let middle = Value::new(2, 0);
        let replacement = Value::new(3, 0);
        let defined = Value::new(4, 0);
        let merge_target = Value::new(5, 0);
        let mut reference = MemRef::new(None, 2);
        reference.base = Some(old);
        reference.segment = Some(middle);
        let mut op = Op::new(0, None, "op", vec![defined], vec![old]);
        op.exits = vec![middle];
        op.args = vec![
            Arg::Held(Held {
                value: old,
                width: 2,
            }),
            Arg::Cell(Cell {
                r#ref: reference.clone(),
            }),
        ];
        op.results = vec![
            Arg::Held(Held {
                value: old,
                width: 2,
            }),
            Arg::Cell(Cell {
                r#ref: reference.clone(),
            }),
        ];
        op.loads = vec![reference.clone()];
        op.stores = vec![reference.clone()];
        op.memory_values = vec![(reference, Const::new(7, 2))];
        op.merges.insert(old, merge_target);
        let swaps = BTreeMap::from([
            (old.id, middle),
            (middle.id, replacement),
            (merge_target.id, defined),
        ]);

        let changed = substituted(&op, &swaps).unwrap();
        assert_eq!(changed.defines, vec![defined]);
        assert_eq!(changed.uses, vec![replacement]);
        assert_eq!(changed.exits, vec![replacement]);
        assert_eq!(
            changed.args[0],
            Arg::Held(Held {
                value: replacement,
                width: 2
            })
        );
        assert_eq!(
            changed.results[0],
            Arg::Held(Held {
                value: old,
                width: 2
            })
        );
        let argument_cell = match &changed.args[1] {
            Arg::Cell(cell) => cell,
            _ => panic!("argument must remain a cell"),
        };
        assert_eq!(argument_cell.r#ref.base, Some(replacement));
        assert_eq!(argument_cell.r#ref.segment, Some(replacement));
        for reference in changed
            .loads
            .iter()
            .chain(&changed.stores)
            .chain(std::iter::once(&changed.memory_values[0].0))
        {
            assert_eq!(reference.base, Some(replacement));
            assert_eq!(reference.segment, Some(replacement));
        }
        let result_cell = match &changed.results[1] {
            Arg::Cell(cell) => cell,
            _ => panic!("result must remain a cell"),
        };
        assert_eq!(result_cell.r#ref.base, Some(replacement));
        assert_eq!(result_cell.r#ref.segment, Some(replacement));
        assert_eq!(
            changed.merges.iter().collect::<Vec<_>>(),
            vec![(&replacement, &merge_target)]
        );
        assert_eq!(substituted(&op, &BTreeMap::new()).unwrap(), op);
    }

    #[test]
    fn provider_follows_transitively_and_reports_cycles() {
        let first = Value::new(1, 0);
        let second = Value::new(2, 0);
        let third = Value::new(3, 0);
        assert_eq!(
            provider(
                first,
                &BTreeMap::from([(first.id, second), (second.id, third)])
            )
            .unwrap(),
            third
        );
        assert_eq!(
            provider(
                first,
                &BTreeMap::from([(first.id, second), (second.id, first)])
            ),
            Err(SubstitutionError::CyclicValueSubstitution)
        );
        assert_eq!(
            provider(first, &BTreeMap::from([(first.id, first)])).unwrap(),
            first
        );
    }

    #[test]
    fn values_preserves_body_order_and_duplicates() {
        let defined = Value::new(1, 0);
        let used = Value::new(2, 0);
        let exited = Value::new(3, 0);
        let phi_result = Value::new(4, 1);
        let mut first = Op::new(0, None, "first", vec![defined], vec![used]);
        first.exits = vec![exited, used];
        let second = Op::new(1, None, "second", vec![used], vec![defined]);
        let mut phi = Phi::new(phi_result);
        phi.incoming.insert(9, used);
        phi.incoming.insert(7, defined);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![phi], vec![first, second], vec![]),
                MirBlock::new(
                    1,
                    vec![],
                    vec![Op::new(2, None, "third", vec![exited], vec![used])],
                    vec![],
                ),
            ],
        );

        assert_eq!(
            values(&body).collect::<Vec<_>>(),
            vec![
                defined, used, exited, used, used, defined, phi_result, used, defined, exited,
                used,
            ]
        );
    }
}

// ---- early port (agent E) ----

/// Carry semantic pointer facts onto fresh SSA definitions.
pub(crate) fn cloned_pointer_metadata<'a>(
    body: &MirBody,
    mappings: impl Iterator<Item = &'a indexmap::IndexMap<u32, Value>>,
) -> (
    BTreeSet<Value>,
    crate::model::mir::OrderedMap<Value, crate::model::memory::Provenance>,
) {
    let pointer_ids = body.pointer_values.iter().map(|value| value.id).collect::<BTreeSet<u32>>();
    let seeds = body
        .pointer_seeds
        .iter()
        .map(|(value, provenance)| (value.id, provenance.clone()))
        .collect::<indexmap::IndexMap<u32, _>>();
    let mut pointer_values = body.pointer_values.clone();
    let mut pointer_seeds = body.pointer_seeds.clone();
    for mapping in mappings {
        for (original, cloned) in mapping {
            if pointer_ids.contains(original) {
                pointer_values.insert(*cloned);
            }
            if let Some(seed) = seeds.get(original) {
                pointer_seeds.insert(*cloned, seed.clone());
            }
        }
    }
    (pointer_values, pointer_seeds)
}

/// Carry frontend integer facts onto structurally cloned values.
pub(crate) fn cloned_integer_ranges<'a>(
    body: &MirBody,
    mappings: impl Iterator<Item = &'a indexmap::IndexMap<u32, Value>>,
) -> crate::model::mir::OrderedMap<Value, crate::model::mir::IntegerRange> {
    let ranges = body
        .integer_ranges
        .iter()
        .map(|(value, interval)| (value.id, interval.clone()))
        .collect::<indexmap::IndexMap<u32, _>>();
    let mut cloned_ranges = body.integer_ranges.clone();
    for mapping in mappings {
        for (original, cloned) in mapping {
            if let Some(interval) = ranges.get(original) {
                cloned_ranges.insert(*cloned, interval.clone());
            }
        }
    }
    cloned_ranges
}
