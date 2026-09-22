//! Strength reduction: a multiply in a loop becomes an add.
//!
//! Port of `qbopt/optimize/strength.py`. Python's `id(op)` is the
//! [`OpOccurrence`] of the input body; functions that read `Derived.op`
//! therefore take that body.
//!
//! Not ported yet: `Strength` (the MIRTransform), which composes
//! `ivshare.shared`, `transform.dead`, `exitsink.sunk`, `loopexit.evaluated`
//! and four `indvars` rewrites that have no Rust port.

use indexmap::IndexMap;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::sync::LazyLock;

use num_bigint::BigInt;

use crate::analysis::consts::{self, Known, masked};
use crate::analysis::induction::{self, Affine, AffineMap, AffineOperand, Derived};
use crate::analysis::liveness;
use crate::analysis::loops::{self as loopy, Loop};
use crate::analysis::occurrence::{OpOccurrence, operations};
use crate::analysis::regions::{RegionError, RegionLayout};
use crate::analysis::ssa::{self, ConstructionError, SubstitutionError};
use crate::model::ir::Operation;
use crate::model::mir::{
    Arg, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Value,
};
use crate::model::passes::{AddressForm, OperationCosts};
use crate::objectfile::module::Space;

use super::transform;

#[cfg_attr(not(test), allow(dead_code))]
static _DEFAULT_COSTS: LazyLock<OperationCosts> = LazyLock::new(OperationCosts::default);

/// What Python raises through `reduced`, from the analyses it composes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StrengthError {
    Region(RegionError),
    Substitution(SubstitutionError),
    Construction(ConstructionError),
}

impl fmt::Display for StrengthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Region(error) => write!(formatter, "{error:?}"),
            Self::Substitution(error) => error.fmt(formatter),
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

impl From<SubstitutionError> for StrengthError {
    fn from(error: SubstitutionError) -> Self {
        Self::Substitution(error)
    }
}

impl From<ConstructionError> for StrengthError {
    fn from(error: ConstructionError) -> Self {
        Self::Construction(error)
    }
}

/// `body` with every multiply of a counter by an invariant made an add.
///
/// `layout` stands for Python's `dgroup` and `bounds`: it is what the Rust
/// `induction.of` takes for them.
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)] // `Strength.transform` is its caller once indvars is ported.
pub(crate) fn reduced(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    layout: Option<&RegionLayout>,
    registers: i64,
    scales: &BTreeSet<i64>,
    call_registers: i64,
    costs: &OperationCosts,
    address_forms: &[AddressForm],
    control_recurrences: bool,
) -> Result<MirBody, StrengthError> {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let found = induction::of(body, dgroup, layout)?;
    if found.is_empty() {
        return Ok(body.clone());
    }

    let mut candidate_groups = BTreeMap::<i64, Vec<Derived>>::new();
    for (loop_, _basics, derived) in &found {
        candidate_groups.insert(loop_.header, _candidates(body, derived, scales));
    }
    let replacement_credits = if control_recurrences {
        &_replacement_credits(body, &found, &candidate_groups, costs)
            | &_control_credits(body, &found, &candidate_groups, costs)
    } else {
        BTreeSet::new()
    };

    let live = (registers != 0).then(|| liveness::live(body));
    let at_of = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    let mut references = BTreeMap::<u32, i64>::new();
    for block in &body.blocks {
        for op in &block.ops {
            for value in &op.uses {
                *references.entry(value.id).or_insert(0) += 1;
            }
        }
    }
    let mut taken = ssa::values(body).map(|one| one.variable).max().unwrap_or(0);
    let first = taken + 1;
    let mut ahead = BTreeMap::<i64, Vec<Op>>::new();
    let mut behind = BTreeMap::<i64, Vec<Op>>::new();
    let mut replacements = BTreeMap::<OpOccurrence, Vec<Op>>::new();
    // A carried pointer replaces an address expression with a copy from the
    // loop phi.  Its immediately-following memory use should name that phi
    // directly, rather than spend a copy merely to use it as a cell base.
    // Bindings are certified once all formula choices are known below: a
    // value used on another block or on a loop exit must retain its copy.
    let mut pointer_bindings = Vec::<(OpOccurrence, Value, Value)>::new();
    let mut wide = BTreeSet::<Value>::new();
    let facts = if !scales.is_empty() || !address_forms.is_empty() {
        consts::known(body, None, None, None, None)
    } else {
        IndexMap::new()
    };
    for (loop_, _basics, _derived) in &found {
        let preheader = transform::_preheader(body, loop_);
        let latches = loop_
            .latches
            .iter()
            .copied()
            .filter(|at| at_of.contains_key(at))
            .collect::<Vec<_>>();
        let Some(preheader) = preheader else {
            continue; // two ways in or out is a bigger change than this
        };
        if at_of[&preheader].succ != [loop_.header] || latches.len() != 1 {
            continue;
        }
        let mut candidates = candidate_groups[&loop_.header].clone();
        // Priced against the recurrences the loop drives, not against
        // pressure; see strength.py for the measurements.
        let leaves = _formula_set(
            body,
            &candidates,
            None,
            &BTreeSet::new(),
            &BTreeSet::new(),
            costs,
            None,
        );
        let mut widened = BTreeMap::<u32, Option<Vec<(OpOccurrence, Op)>>>::new();
        let mut native_forms = address_forms
            .iter()
            .filter(|form| !form.secondary)
            .cloned()
            .collect::<Vec<_>>();
        if native_forms.is_empty() {
            native_forms = vec![
                AddressForm::new(2, scales.clone(), 0, 0, 0, false, None)
                    .expect("a form with no fallback agrees with itself"),
            ];
        }
        let secondary_forms = address_forms
            .iter()
            .filter(|form| form.secondary)
            .cloned()
            .collect::<Vec<_>>();
        let mut native = BTreeMap::<OpOccurrence, (i64, AddressForm)>::new();
        for one in &leaves {
            if let Some(indexed) = native_forms
                .iter()
                .find_map(|form| _legal_form(body, loop_, one, form, &facts, &mut widened))
            {
                native.insert(one.op, indexed);
            }
        }
        let mut secondary = BTreeMap::<OpOccurrence, (i64, AddressForm)>::new();
        for one in &leaves {
            if native.contains_key(&one.op) {
                continue;
            }
            if let Some(indexed) = secondary_forms
                .iter()
                .find_map(|form| _legal_form(body, loop_, one, form, &facts, &mut widened))
            {
                secondary.insert(one.op, indexed);
            }
        }
        let mut room = candidates.len() as i64;
        let mut capacity = registers;
        if call_registers != 0
            && loop_
                .body
                .iter()
                .any(|at| at_of[at].ops.iter().any(|op| op.kind == Kind::Call))
        {
            capacity = if capacity != 0 {
                capacity.min(call_registers)
            } else {
                call_registers
            };
        }
        if capacity != 0 {
            // The fixed reserve is an upper bound; actual MIR liveness may
            // expose a tighter loop.
            room =
                0.max((capacity - _recurrences(body, loop_) - _RESERVE).min(
                    capacity - liveness::pressure(body, live.as_ref(), Some(&loop_.body)) as i64,
                ));
        }
        let native_keys = native.keys().copied().collect::<BTreeSet<_>>();
        let secondary_indexes = _secondary_indexes(
            body,
            &leaves,
            room,
            &native_keys,
            &secondary,
            Some(&references),
        );
        let free = &native_keys | &secondary_indexes;
        // A derived recurrence which can replace its source loop counter does
        // not consume another recurrence slot.
        candidates = _formula_set(
            body,
            &candidates,
            Some(room),
            &free,
            &replacement_credits,
            costs,
            Some(&references),
        );
        let mut indexes = BTreeMap::<OpOccurrence, (i64, AddressForm)>::new();
        for one in &candidates {
            if native.contains_key(&one.op) || secondary_indexes.contains(&one.op) {
                let indexed = secondary
                    .get(&one.op)
                    .or_else(|| native.get(&one.op))
                    .expect("an index has a form")
                    .clone();
                indexes.insert(one.op, indexed);
            }
        }
        // Only a counter every address indexes.
        let stepped = candidates
            .iter()
            .filter(|one| !indexes.contains_key(&one.op))
            .map(|one| one.of.value)
            .collect::<BTreeSet<_>>();
        indexes = candidates
            .iter()
            .filter(|one| indexes.contains_key(&one.op) && !stepped.contains(&one.of.value))
            .map(|one| (one.op, indexes[&one.op].clone()))
            .collect();
        let dword_counters = candidates
            .iter()
            .filter(|one| {
                indexes
                    .get(&one.op)
                    .is_some_and(|(_, form)| form.index_width > 2)
            })
            .map(|one| one.of.value)
            .collect::<BTreeSet<_>>();
        for value in dword_counters {
            for (op, widened_op) in widened[&value].iter().flatten() {
                replacements
                    .entry(*op)
                    .or_insert_with(|| vec![widened_op.clone()]);
            }
        }
        for one in &candidates {
            if !indexes.contains_key(&one.op) || replacements.contains_key(&one.op) {
                continue;
            }
            let (scale, form) = indexes[&one.op].clone();
            let op = op_at(one.op);
            let answer = _answer(body, one.op).expect("a candidate has an answer");
            let counter = at_of[&loop_.header]
                .phis
                .iter()
                .find(|phi| phi.result.id == one.of.value)
                .map(|phi| phi.result)
                .expect("a counter is a header phi");
            taken += 1;
            let base = Value {
                id: _next(body, taken),
                at: preheader,
                flags: false,
                variable: taken,
                version: 1,
            };
            ahead
                .entry(preheader)
                .or_default()
                .extend(_starts(body, base, one, preheader, false));
            taken += one.offsets.len() as u32 * 2;
            let address = |mut replaced: Op| {
                replaced.kind = Kind::Add;
                replaced.op = Some(OpCode::Operation(Operation::Binary));
                replaced.name = "add".to_owned();
                replaced.source_backed = false;
                replaced.defines = vec![answer];
                replaced.loads = Vec::new();
                replaced.stores = Vec::new();
                replaced.merges = OrderedMap::new();
                replaced.symbol = Some(false);
                replaced
            };
            let index_width = form.index_width as u32;
            if form.index_width == 2 {
                let mut replaced = op.clone();
                replaced.uses = vec![base, counter];
                replaced.args = vec![
                    Arg::Held(Held {
                        value: base,
                        width: 2,
                    }),
                    Arg::Held(Held {
                        value: counter,
                        width: 2,
                    }),
                ];
                replaced.results = vec![Arg::Held(Held {
                    value: answer,
                    width: 2,
                })];
                replacements.insert(one.op, vec![address(replaced)]);
                continue;
            }
            taken += 1;
            let extended = Value {
                id: _next(body, taken),
                at: preheader,
                flags: false,
                variable: taken,
                version: 1,
            };
            let mut extend = _made(
                Kind::ZeroExtend,
                "movzx",
                extended,
                vec![Arg::Held(Held {
                    value: base,
                    width: 2,
                })],
                preheader,
                op,
            );
            extend.results = vec![Arg::Held(Held {
                value: extended,
                width: index_width,
            })];
            ahead
                .get_mut(&preheader)
                .expect("set up above")
                .push(extend);
            taken += 1;
            let product = Value {
                id: _next(body, taken),
                at: op.at,
                flags: false,
                variable: taken,
                version: 1,
            };
            let shift = _made(
                Kind::Shl,
                "shl",
                product,
                vec![
                    Arg::Held(Held {
                        value: counter,
                        width: index_width,
                    }),
                    Arg::Const(Const::new(i64::from(64 - scale.leading_zeros()) - 1, 1)),
                ],
                op.at,
                op,
            );
            let mut replaced = op.clone();
            replaced.uses = vec![extended, product];
            replaced.args = vec![
                Arg::Held(Held {
                    value: extended,
                    width: index_width,
                }),
                Arg::Held(Held {
                    value: product,
                    width: index_width,
                }),
            ];
            replaced.results = vec![Arg::Held(Held {
                value: answer,
                width: index_width,
            })];
            replacements.insert(one.op, vec![shift, address(replaced)]);
            wide.insert(answer);
        }
        candidates.retain(|one| !indexes.contains_key(&one.op));
        // Every remaining formula is a value live around the loop; the
        // pressure-priced choices of `_formula_set` stand as made.
        let mut added = 0;
        // One counter an expression.
        let mut shared = HashMap::<
            (
                AffineOperand,
                AffineOperand,
                Arg,
                Vec<(Arg, BigInt)>,
                Option<Arg>,
                u32,
            ),
            Value,
        >::new();
        for one in &candidates {
            // Once each: a multiply in a nest is derived in every loop.
            let mut one = one.clone();
            let op = op_at(one.op);
            let answer = _answer(body, one.op);
            let Some(answer) = answer else {
                continue;
            };
            if replacements.contains_key(&one.op) {
                continue;
            }
            let width = _width(op);
            let key = (
                one.of.start.clone(),
                one.of.step.clone(),
                one.by.clone(),
                one.offsets.clone(),
                one.pointer.clone(),
                width,
            );
            if let Some(&counter) = shared.get(&key) {
                replacements.insert(one.op, vec![_copying(op, counter, answer, width)]);
                if one.pointer.is_some() {
                    pointer_bindings.push((one.op, answer, counter));
                }
                continue;
            }
            if _times(&one.of.step, &one.by, width).is_none() {
                continue;
            }
            if let Arg::Cell(cell) = &one.by {
                if op.loads != [cell.r#ref.clone()] || !op.args.contains(&one.by) {
                    continue;
                }
                taken += 1;
                let multiplier = Value {
                    id: _next(body, taken),
                    at: preheader,
                    flags: false,
                    variable: taken,
                    version: 1,
                };
                let mut load = _made(
                    Kind::Load,
                    "mov",
                    multiplier,
                    vec![one.by.clone()],
                    preheader,
                    op,
                );
                load.id = op.id;
                load.symbol = Some(true);
                ahead.entry(preheader).or_default().push(load);
                one.by = Arg::Held(Held {
                    value: multiplier,
                    width,
                });
            }
            let Some(stride) = _times(&one.of.step, &one.by, width) else {
                continue;
            };
            taken += 1;
            let start = Value {
                id: _next(body, taken),
                at: preheader,
                flags: false,
                variable: taken,
                version: 1,
            };
            let step = Value {
                id: start.id + 1,
                at: latches[0],
                flags: false,
                variable: taken,
                version: 2,
            };

            ahead
                .entry(preheader)
                .or_default()
                .extend(_starts(body, start, &one, preheader, true));
            taken += _start_temporary_count(&one, true);
            behind.entry(latches[0]).or_default().push(_made(
                if one.pointer.is_some() {
                    Kind::PtrOffset
                } else {
                    Kind::Add
                },
                if one.pointer.is_some() { "" } else { "add" },
                step,
                vec![
                    Arg::Held(Held {
                        value: start,
                        width,
                    }),
                    stride,
                ],
                latches[0],
                op,
            ));
            replacements.insert(one.op, vec![_copying(op, start, answer, width)]);
            if one.pointer.is_some() {
                pointer_bindings.push((one.op, answer, start));
            }
            shared.insert(key, start);
            added += 1;
        }
        let _ = added;
    }

    if replacements.is_empty() {
        return Ok(body.clone());
    }
    let pointer_rebases = _local_pointer_rebases(body, &pointer_bindings, &replacements);
    let mut changed = body.clone();
    let none = BTreeMap::new();
    for (index, block) in body.blocks.iter().enumerate() {
        let occurrences = operations(body)
            .filter(|(at, _, _)| at.block_index() == index)
            .map(|(at, _, op)| (at, op))
            .collect::<Vec<_>>();
        let mut ops = Vec::new();
        for (at, op) in _woven(
            block,
            &occurrences,
            ahead.get(&block.at).map_or(&[][..], Vec::as_slice),
            behind.get(&block.at).map_or(&[][..], Vec::as_slice),
            &replacements,
        ) {
            let swap = at.and_then(|at| pointer_rebases.get(&at)).unwrap_or(&none);
            ops.push(_rebased(&ssa::substituted(&op, swap)?, &wide));
        }
        changed.blocks[index].ops = ops;
    }
    Ok(ssa::constructed(&changed, &(first..=taken).collect())?)
}

fn _candidates(body: &MirBody, derived: &[Derived], scales: &BTreeSet<i64>) -> Vec<Derived> {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let candidates = derived
        .iter()
        .filter(|one| {
            let op = op_at(one.op);
            _answer(body, one.op).is_some()
                && (_multiplies(body, one, derived)
                    || one.pointer.is_some()
                    || op.kind == Kind::Divmod
                    || !one.offsets.is_empty() && op.kind == Kind::Shl
                    || one
                        .offsets
                        .iter()
                        .any(|(offset, _)| matches!(offset, Arg::Cell(_)))
                    || op.kind == Kind::Add
                        && one
                            .offsets
                            .iter()
                            .any(|(offset, _)| matches!(offset, Arg::Held(_))))
        })
        .cloned()
        .collect::<Vec<_>>();
    candidates
        .into_iter()
        .filter(|one| scales.is_empty() || !_indexed(body, one))
        .collect()
}

#[derive(Clone, Debug)]
struct _Replacement {
    loop_: Loop,
    root: Derived,
    mapping: AffineMap,
    domain: (BigInt, BigInt),
    aliases: BTreeSet<Value>,
    allowed: BTreeSet<OpOccurrence>,
    rank: (i64, i64, i64),
}

fn _formula_descendants(
    body: &MirBody,
    root: &Derived,
    formulas: &[Derived],
) -> BTreeSet<OpOccurrence> {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let held_result = |op: &Op| match op.results.first() {
        Some(Arg::Held(held)) => Some(held.value),
        _ => None,
    };
    let reads = |op: &Op, value: Value| {
        op.args
            .iter()
            .any(|arg| matches!(arg, Arg::Held(held) if held.value == value))
    };
    let mut pending = vec![held_result(op_at(root.op)).expect("a root has a result")];
    let mut seen = BTreeSet::<Value>::new();
    let mut operations = BTreeSet::from([root.op]);
    while let Some(value) = pending.pop() {
        if !seen.insert(value) {
            continue;
        }
        for formula in formulas {
            let op = op_at(formula.op);
            if let Some(result) = held_result(op) {
                if reads(op, value) {
                    operations.insert(formula.op);
                    pending.push(result);
                }
            }
        }
    }
    operations
}

type Found = Vec<(Loop, OrderedMap<u32, Affine>, Vec<Derived>)>;

/// Formulas whose source index is proved replaceable as loop control.
///
/// Separate from `_replacement_credits`: here the formula replaces every
/// data use of `i` and the counted-loop proof says the source is otherwise
/// control-only, so its net pressure cost is zero.
fn _control_credits(
    body: &MirBody,
    found: &Found,
    groups: &BTreeMap<i64, Vec<Derived>>,
    costs: &OperationCosts,
) -> BTreeSet<OpOccurrence> {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let held_result = |op: &Op| match op.results.first() {
        Some(Arg::Held(held)) => Some(held.value),
        _ => None,
    };
    let facts = consts::known(body, None, None, None, None);
    let mut selected = BTreeMap::<u32, ((i64, i64, i64), Derived)>::new();

    for (loop_, _basics, _derived) in found {
        let proofs = induction::counted(body, loop_, Some(&facts));
        if proofs.len() != 1 {
            continue;
        }
        let proof = &proofs[0];
        let candidates = groups[&loop_.header]
            .iter()
            .filter(|one| one.of == proof.counter)
            .cloned()
            .collect::<Vec<_>>();
        let results = candidates
            .iter()
            .filter_map(|one| held_result(op_at(one.op)))
            .collect::<BTreeSet<_>>();
        let roots = candidates.iter().filter(|one| {
            !op_at(one.op)
                .args
                .iter()
                .any(|arg| matches!(arg, Arg::Held(held) if results.contains(&held.value)))
        });
        for (order, root) in roots.enumerate() {
            let descendants = _formula_descendants(body, root, &candidates);
            if induction::control_replacement(body, loop_, proof, &descendants).is_none() {
                continue;
            }
            let rank = (
                descendants.len() as i64,
                _recompute_cost(body, root, costs),
                -(order as i64),
            );
            let previous = selected.get(&proof.counter.value);
            if previous.is_none_or(|previous| rank > previous.0) {
                selected.insert(proof.counter.value, (rank, root.clone()));
            }
        }
    }

    selected.values().map(|(_rank, root)| root.op).collect()
}

/// Root formulas proved to replace their source control recurrences.
///
/// The proof is relational: an equality with another counter is allowed
/// when that counter has a candidate with the identical injective map.
/// Candidate pairs are retained to a fixed point.
fn _replacement_credits(
    body: &MirBody,
    found: &Found,
    groups: &BTreeMap<i64, Vec<Derived>>,
    costs: &OperationCosts,
) -> BTreeSet<OpOccurrence> {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let held_result = |op: &Op| match op.results.first() {
        Some(Arg::Held(held)) => Some(held.value),
        _ => None,
    };
    let facts = consts::known(body, None, None, None, None);
    let blocks = body
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.at, index))
        .collect::<BTreeMap<_, _>>();
    // Python's `made` maps to the op object; Rust keeps its occurrence too.
    let mut made = BTreeMap::<u32, &Op>::new();
    let mut made_at = BTreeMap::<u32, OpOccurrence>::new();
    for (at, _, op) in operations(body) {
        for value in &op.defines {
            made.insert(value.id, op);
            made_at.insert(value.id, at);
        }
    }
    let flags = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| &op.defines)
        .filter(|value| value.flags)
        .copied()
        .collect::<BTreeSet<_>>();
    let flag_readers = ssa::use_index(body, Some(&flags), false);
    let mut nodes = Vec::<_Replacement>::new();

    for (loop_, _basics, _derived) in found {
        let count = induction::trip_count(body, loop_, &facts);
        let header_index = blocks[&loop_.header];
        let header = &body.blocks[header_index];
        let Some(count) = count else {
            continue;
        };
        if loop_.latches.len() != 1 || header.ops.is_empty() {
            continue;
        }
        let branch = header.ops.last().expect("checked non-empty");
        if branch.kind != Kind::Branch {
            continue;
        }
        let latch = *loop_.latches.iter().next().expect("one latch");
        let candidates = &groups[&loop_.header];
        // Python iterates a set of Affine here; the nodes of one source keep
        // their relative order either way, so first-appearance order is used.
        let mut affines = Vec::<&Affine>::new();
        for one in candidates {
            if !affines.contains(&&one.of) {
                affines.push(&one.of);
            }
        }
        for affine in affines {
            let phi = header.phis.iter().find(|one| one.result.id == affine.value);
            let domain = induction::domain(body, loop_, affine, &facts);
            let (Some(phi), Some(domain)) = (phi, domain) else {
                continue;
            };
            if !phi.incoming.contains_key(&latch) {
                continue;
            }
            let controls = operations(body)
                .filter(|(at, _, _)| {
                    at.block_index() == header_index && at.operation_index() + 1 < header.ops.len()
                })
                .filter(|(_, _, op)| {
                    induction::_counter_bound(op, branch, affine, affine.start.width(), Some(&made))
                        .is_some()
                })
                .map(|(at, _, _)| at)
                .collect::<BTreeSet<_>>();
            let update = made_at.get(&phi.incoming.get(&latch).expect("checked").id);
            let Some(&update) = update else {
                continue;
            };
            if controls.len() != 1 {
                continue;
            }

            let (aliases, copies) = induction::transparent_aliases(body, loop_, phi.result);

            let same = candidates
                .iter()
                .filter(|one| &one.of == affine)
                .cloned()
                .collect::<Vec<_>>();
            let results = same
                .iter()
                .filter_map(|one| held_result(op_at(one.op)))
                .collect::<BTreeSet<_>>();
            let roots = same.iter().filter(|one| {
                !op_at(one.op)
                    .args
                    .iter()
                    .any(|arg| matches!(arg, Arg::Held(held) if results.contains(&held.value)))
            });
            for (order, root) in roots.enumerate() {
                let mapping = induction::derived_map(root, &facts);
                let width = _width(op_at(root.op));
                let stride = _times(&root.of.step, &root.by, width);
                let signed = stride
                    .as_ref()
                    .and_then(|stride| induction::_signed(stride, &facts, width));
                let modulus = BigInt::from(1_u8) << (width * 8);
                let Some(mapping) = mapping else {
                    continue;
                };
                let Some(signed) = signed else {
                    continue;
                };
                if signed == BigInt::from(0_u8) {
                    continue;
                }
                let magnitude = BigInt::from(signed.magnitude().clone());
                if count >= &modulus / induction::gcd(magnitude, modulus.clone()) {
                    continue;
                }
                let descendants = _formula_descendants(body, root, &same);
                let mut allowed = &(&descendants | &controls) | &copies;
                allowed.insert(update);
                nodes.push(_Replacement {
                    loop_: loop_.clone(),
                    root: root.clone(),
                    mapping,
                    domain: domain.clone(),
                    aliases: aliases.clone(),
                    allowed,
                    rank: (
                        descendants.len() as i64,
                        _recompute_cost(body, root, costs),
                        -(order as i64),
                    ),
                });
            }
        }
    }

    let mut aliases = BTreeMap::<Value, BTreeSet<u32>>::new();
    let mut domains = BTreeMap::<u32, (BigInt, BigInt)>::new();
    let mut existing = BTreeMap::<u32, HashSet<AffineMap>>::new();
    for (loop_, basics, _derived) in found {
        let header = &body.blocks[blocks[&loop_.header]];
        let latch = if loop_.latches.len() == 1 {
            loop_.latches.iter().next().copied()
        } else {
            None
        };
        for affine in basics.values() {
            let domain = induction::domain(body, loop_, affine, &facts);
            let phi = header.phis.iter().find(|one| one.result.id == affine.value);
            let (Some(domain), Some(phi)) = (domain, phi) else {
                continue;
            };
            domains.insert(affine.value, domain);
            let (source_aliases, _copies) = induction::transparent_aliases(body, loop_, phi.result);
            for value in source_aliases {
                aliases.entry(value).or_default().insert(affine.value);
            }

            for alternative in basics.values() {
                if alternative.value == affine.value {
                    continue;
                }
                let relation = induction::relation(affine, alternative, &facts);
                let alternative_phi = header
                    .phis
                    .iter()
                    .find(|one| one.result.id == alternative.value);
                let (Some(relation), Some(alternative_phi), Some(latch)) =
                    (relation, alternative_phi, latch)
                else {
                    continue;
                };
                let update = alternative_phi.incoming.get(&latch);
                let Some(update) = update else {
                    continue;
                };
                if !body
                    .blocks
                    .iter()
                    .filter(|block| loop_.body.contains(&block.at))
                    .flat_map(|block| &block.ops)
                    .any(|op| {
                        op.uses.contains(&alternative_phi.result) && !op.defines.contains(update)
                    })
                {
                    continue;
                }
                existing.entry(affine.value).or_default().insert(relation);
            }
        }
    }

    for node in &nodes {
        for value in &node.aliases {
            aliases
                .entry(*value)
                .or_default()
                .insert(node.root.of.value);
        }
        domains.insert(node.root.of.value, node.domain.clone());
    }
    let mut by_source = BTreeMap::<u32, Vec<usize>>::new();
    for (index, node) in nodes.iter().enumerate() {
        by_source.entry(node.root.of.value).or_default().push(index);
    }

    let equality = |node: &_Replacement, op: &Op, active: &BTreeSet<usize>| -> bool {
        if op.kind != Kind::Sub
            || !op.results.is_empty()
            || !op.loads.is_empty()
            || !op.stores.is_empty()
            || op.barrier()
            || !op.merges.is_empty()
            || op.args.len() != 2
            || op.defines.len() != 1
            || !op.defines[0].flags
            || flag_readers.get(&op.defines[0]).is_none_or(Vec::is_empty)
            || flag_readers[&op.defines[0]].iter().any(|reader| {
                let reader = op_at(*reader);
                reader.kind != Kind::Branch
                    || !matches!(reader.test, Some(Kind::Eq) | Some(Kind::Ne))
            })
        {
            return false;
        }
        let positions = op
            .args
            .iter()
            .enumerate()
            .filter(|(_, arg)| {
                matches!(arg, Arg::Held(held)
                    if node.aliases.contains(&held.value) && held.width == node.mapping.width)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if positions.len() != 1 {
            return false;
        }
        let Arg::Held(other) = &op.args[1 - positions[0]] else {
            return false;
        };
        if other.width != node.mapping.width {
            return false;
        }
        let mut partners = aliases.get(&other.value).cloned().unwrap_or_default();
        partners.remove(&node.root.of.value);
        partners.iter().any(|partner| {
            domains.get(partner).is_some_and(|domain| {
                node.mapping.injective(
                    (&node.domain.0).min(&domain.0),
                    (&node.domain.1).max(&domain.1),
                )
            }) && (existing
                .get(partner)
                .is_some_and(|maps| maps.contains(&node.mapping))
                || by_source.get(partner).is_some_and(|indexes| {
                    indexes.iter().any(|candidate| {
                        active.contains(candidate) && nodes[*candidate].mapping == node.mapping
                    })
                }))
        })
    };

    let valid = |node: &_Replacement, active: &BTreeSet<usize>| -> bool {
        operations(body)
            .filter(|(_, block, _)| node.loop_.body.contains(&block.at))
            .all(|(at, _, op)| {
                !op.uses.iter().any(|value| node.aliases.contains(value))
                    || node.allowed.contains(&at)
                    || equality(node, op, active)
            })
    };

    let mut active = (0..nodes.len()).collect::<BTreeSet<_>>();
    loop {
        let rejected = active
            .iter()
            .copied()
            .filter(|index| !valid(&nodes[*index], &active))
            .collect::<BTreeSet<_>>();
        if rejected.is_empty() {
            break;
        }
        active = &active - &rejected;
    }

    let mut selected = BTreeMap::<u32, usize>::new();
    for index in active {
        let source = nodes[index].root.of.value;
        if selected
            .get(&source)
            .is_none_or(|chosen| nodes[index].rank > nodes[*chosen].rank)
        {
            selected.insert(source, index);
        }
    }
    // A final selection may choose a different map from the one which made an
    // equality viable.  Remove either side rather than taking speculative
    // pressure credit; the ordinary formula price remains available.
    loop {
        let chosen = selected.values().copied().collect::<BTreeSet<_>>();
        let rejected = selected
            .iter()
            .filter(|(_, index)| !valid(&nodes[**index], &chosen))
            .map(|(source, _)| *source)
            .collect::<Vec<_>>();
        if rejected.is_empty() {
            break;
        }
        for source in rejected {
            selected.remove(&source);
        }
    }
    selected
        .values()
        .map(|index| nodes[*index].root.op)
        .collect()
}

/// Choose whole recurrence formulas, collapsing sibling address forms.
///
/// Carry every address of a shared product, or carry the product and keep
/// the cheap additions; never some of each. `free` leaves are indexed
/// memory forms and consume no recurrence. `credited` formulas replace
/// their source control recurrence and add no net pressure.
fn _formula_set(
    body: &MirBody,
    candidates: &[Derived],
    room: Option<i64>,
    free: &BTreeSet<OpOccurrence>,
    credited: &BTreeSet<OpOccurrence>,
    costs: &OperationCosts,
    references: Option<&BTreeMap<u32, i64>>,
) -> Vec<Derived> {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let held_result = |op: &Op| match op.results.first() {
        Some(Arg::Held(held)) => Some(held.value),
        _ => None,
    };
    let reads = |op: &Op, value: Value| {
        op.args
            .iter()
            .any(|arg| matches!(arg, Arg::Held(held) if held.value == value))
    };
    let op = |one: &Derived| op_at(one.op);
    let made = candidates
        .iter()
        .filter_map(|one| held_result(op(one)))
        .collect::<BTreeSet<_>>();
    let consumed = candidates
        .iter()
        .flat_map(|one| &op(one).args)
        .filter_map(|arg| match arg {
            Arg::Held(held) if made.contains(&held.value) => Some(held.value),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut selected = candidates
        .iter()
        .filter(|one| held_result(op(one)).is_some_and(|value| !consumed.contains(&value)))
        .map(|one| one.op)
        .collect::<BTreeSet<_>>();
    // A credited root is the exact map which discharges the old counter's
    // uses; carry it and leave its invariant field additions in the loop.
    for root in candidates.iter().filter(|one| credited.contains(&one.op)) {
        let mut pending = vec![held_result(op(root)).expect("a root has a result")];
        while let Some(value) = pending.pop() {
            for child in candidates {
                if child.of != root.of || !reads(op(child), value) {
                    continue;
                }
                selected.remove(&child.op);
                if let Some(result) = held_result(op(child)) {
                    pending.push(result);
                }
            }
        }
        selected.insert(root.op);
    }
    let Some(room) = room else {
        return candidates
            .iter()
            .filter(|one| selected.contains(&one.op))
            .cloned()
            .collect();
    };

    let slots = |selected: &BTreeSet<OpOccurrence>| {
        selected
            .iter()
            .filter(|one| !free.contains(one) && !credited.contains(one))
            .count() as i64
    };

    let empty = BTreeMap::new();
    let references = references.unwrap_or(&empty);
    while slots(&selected) > room {
        let overflow = slots(&selected) - room;
        let mut choices = Vec::<(i64, i64, i64, &Derived, Vec<&Derived>)>::new();
        for (order, parent) in candidates.iter().enumerate() {
            let Some(result) = held_result(op(parent)) else {
                continue;
            };
            let children = candidates
                .iter()
                .filter(|child| {
                    selected.contains(&child.op)
                        && child.of == parent.of
                        && reads(op(child), result)
                })
                .collect::<Vec<_>>();
            // An indexed child is already the zero-recurrence formula. A
            // single child saves no pressure by replacing it with its parent.
            if children.len() < 2 || children.iter().any(|child| free.contains(&child.op)) {
                continue;
            }
            let gain = children.len() as i64 - if selected.contains(&parent.op) { 0 } else { 1 };
            if gain > 0 {
                let relief = gain.min(overflow);
                let mut cheapest = children
                    .iter()
                    .filter_map(|child| held_result(op(child)))
                    .map(|value| references.get(&value.id).copied().unwrap_or(1))
                    .collect::<Vec<_>>();
                cheapest.sort();
                cheapest.truncate(relief as usize);
                // Best case for retaining the leaves: allocation spills the
                // least-used children, each a latch update and a reload per
                // use. The extra ADD charges for exceeding the capacity.
                let spill = cheapest
                    .iter()
                    .map(|uses| costs.memory_update + uses * costs.load + costs.add)
                    .sum::<i64>();
                let leaf = children.len() as i64 * costs.add;
                let collapsed = costs.add + children.len() as i64 * costs.address;
                let benefit = spill - (collapsed - leaf);
                choices.push((benefit, gain, -(order as i64), parent, children));
            }
        }
        let Some((_benefit, _gain, _order, parent, children)) =
            choices.into_iter().reduce(|best, one| {
                if (one.0, one.1, one.2) > (best.0, best.1, best.2) {
                    one
                } else {
                    best
                }
            })
        else {
            break;
        };
        for child in children {
            selected.remove(&child.op);
        }
        selected.insert(parent.op);
    }

    // A lone formula over budget: compare the work it removes with the
    // memory traffic of carrying it as a spilled recurrence.
    while slots(&selected) > room {
        let overflow = candidates
            .iter()
            .filter(|one| {
                selected.contains(&one.op) && !free.contains(&one.op) && !credited.contains(&one.op)
            })
            .collect::<Vec<_>>();
        let mut priced = Vec::<(i64, i64, &Derived)>::new();
        for (order, one) in overflow.into_iter().enumerate() {
            let result = held_result(op(one));
            let uses = result.map_or(1, |result| references.get(&result.id).copied().unwrap_or(1));
            let spilled = costs.memory_update + uses * costs.load;
            priced.push((
                _recompute_cost(body, one, costs) - spilled,
                -(order as i64),
                one,
            ));
        }
        let Some((benefit, _order, loser)) = priced.into_iter().reduce(|best, one| {
            if (one.0, one.1) < (best.0, best.1) {
                one
            } else {
                best
            }
        }) else {
            break;
        };
        if benefit > 0 {
            break;
        }
        selected.remove(&loser.op);
    }

    candidates
        .iter()
        .filter(|one| selected.contains(&one.op))
        .cloned()
        .collect()
}

/// Activate costed address forms before overflowing recurrence storage.
///
/// A legal secondary address spends bytes and a per-use cost but creates no
/// loop-carried value. Only the remaining overflow reaches sibling collapse
/// and spill/recompute pricing.
fn _secondary_indexes(
    body: &MirBody,
    candidates: &[Derived],
    room: i64,
    native: &BTreeSet<OpOccurrence>,
    secondary: &BTreeMap<OpOccurrence, (i64, AddressForm)>,
    references: Option<&BTreeMap<u32, i64>>,
) -> BTreeSet<OpOccurrence> {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let held_result = |op: &Op| match op.results.first() {
        Some(Arg::Held(held)) => Some(held.value),
        _ => None,
    };
    let empty = BTreeMap::new();
    let references = references.unwrap_or(&empty);
    let slots = candidates
        .iter()
        .filter(|one| !native.contains(&one.op))
        .collect::<Vec<_>>();
    let overflow = 0.max(slots.len() as i64 - room);
    let mut choices = Vec::<(i64, i64, usize, OpOccurrence)>::new();
    for (order, one) in slots.into_iter().enumerate() {
        let Some((_scale, form)) = secondary.get(&one.op) else {
            continue;
        };
        let result = held_result(op_at(one.op));
        let uses = result.map_or(1, |result| references.get(&result.id).copied().unwrap_or(1));
        // Extension is loop setup; prefix cost is paid by each addressed use.
        choices.push((
            form.extension_cost + uses * form.use_cost,
            form.extra_bytes * uses,
            order,
            one.op,
        ));
    }
    choices.sort();
    choices
        .into_iter()
        .take(overflow as usize)
        .map(|choice| choice.3)
        .collect()
}

/// Target-neutral cost of rebuilding a complete affine formula.
///
/// `Derived.op` is only the formula's leaf; `by` and `offsets` are the
/// whole formula.
fn _recompute_cost(body: &MirBody, one: &Derived, costs: &OperationCosts) -> i64 {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let kind = op_at(one.op).kind;
    if matches!(kind, Kind::Div | Kind::Rem | Kind::Divmod) {
        return costs.divide;
    }
    let mut work = if let Arg::Const(constant) = &one.by {
        let scale = &constant.n;
        let zero = BigInt::from(0_u8);
        let one_ = BigInt::from(1_u8);
        if *scale == zero || *scale == one_ {
            0
        } else if *scale > zero && (scale & (scale - &one_)) == zero {
            costs.shift
        } else {
            costs.multiply
        }
    } else {
        costs.multiply
    };
    work += one.offsets.len() as i64 * costs.add;
    if one.pointer.is_some() || kind == Kind::PtrOffset {
        work += costs.address;
    }
    work
}

/// `op` made a copy of the counter that replaces it.
fn _copying(op: &Op, start: Value, answer: Value, width: u32) -> Op {
    let mut copy = op.clone();
    copy.kind = Kind::Copy;
    copy.name = String::new();
    copy.source_backed = false;
    copy.defines = vec![answer];
    copy.uses = vec![start];
    copy.args = vec![Arg::Held(Held {
        value: start,
        width,
    })];
    copy.results = vec![Arg::Held(Held {
        value: answer,
        width,
    })];
    copy.loads = Vec::new();
    copy.stores = Vec::new();
    copy.merges = OrderedMap::new();
    copy.symbol = Some(false);
    copy
}

// What a loop's body needs to compute with, beyond the recurrences it
// drives. Two is the knee of the suite sweep, and the operands of an
// expression.
const _RESERVE: i64 = 2;

/// How many recurrences the loop drives already, this pass's own included.
fn _recurrences(body: &MirBody, loop_: &Loop) -> i64 {
    induction::basics(body, loop_).len() as i64
}

/// Replace multiplication chains, not cheap shifts needing extra counters.
fn _multiplies(body: &MirBody, one: &Derived, derived: &[Derived]) -> bool {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let held_result = |op: &Op| match op.results.first() {
        Some(Arg::Held(held)) => Some(held.value),
        _ => None,
    };
    let mut producers = BTreeMap::<Value, OpOccurrence>::new();
    for item in derived {
        if item.of != one.of {
            continue;
        }
        if let Some(result) = held_result(op_at(item.op)) {
            producers.insert(result, item.op);
        }
    }
    let mut pending = vec![one.op];
    let mut seen = BTreeSet::new();
    while let Some(at) = pending.pop() {
        if !seen.insert(at) {
            continue;
        }
        let op = op_at(at);
        if op.kind == Kind::Mul {
            return true;
        }
        pending.extend(op.args.iter().filter_map(|arg| match arg {
            Arg::Held(held) => producers.get(&held.value).copied(),
            _ => None,
        }));
    }
    false
}

/// The counter's value on the way in: `start * by`, computed once.
///
/// A multiply by one is a copy.
fn _start(body: &MirBody, into: Value, one: &Derived, preheader: i64) -> Op {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let beside = op_at(one.op);
    if matches!(&one.by, Arg::Const(constant) if constant.n == BigInt::from(1_u8)) {
        return _made(
            Kind::Copy,
            "mov",
            into,
            vec![one.of.start.as_arg()],
            preheader,
            beside,
        );
    }
    _made(
        Kind::Mul,
        "imul",
        into,
        vec![one.of.start.as_arg(), one.by.clone()],
        preheader,
        beside,
    )
}

/// Initialize scale * start plus invariant offsets once, before the loop.
///
/// Not `counted`, the offsets alone: the base an index is added to.
fn _starts(body: &MirBody, into: Value, one: &Derived, preheader: i64, counted: bool) -> Vec<Op> {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let beside = op_at(one.op);
    let unit = matches!(&one.by, Arg::Const(constant) if constant.n == BigInt::from(1_u8));
    // A composed offset needs its initial `i * scale + invariant` built
    // before adding it to the pointer; the direct form would drop it.
    if unit && one.offsets.is_empty() && one.pointer.is_some() {
        let pointer = one.pointer.as_ref().expect("checked");
        return vec![_made(
            Kind::PtrOffset,
            "",
            into,
            vec![pointer.clone(), one.of.start.as_arg()],
            preheader,
            beside,
        )];
    }
    if one.pointer.is_none() && one.offsets.is_empty() {
        return vec![_start(body, into, one, preheader)];
    }
    let width = _width(beside);
    let count = _start_temporary_count(one, counted);
    let mut temporaries = (0..count).map(|number| Value {
        id: into.id + 2 + number,
        at: preheader,
        flags: false,
        variable: into.variable + 1 + number,
        version: 1,
    });
    let mut current = None;
    let mut operations = Vec::new();
    if counted {
        let temporary = temporaries.next().expect("counted");
        operations.push(_start(body, temporary, one, preheader));
        current = Some(temporary);
    }
    for (index, (offset, coefficient)) in one.offsets.iter().enumerate() {
        let mut offset = offset.clone();
        if *coefficient != BigInt::from(1_u8) {
            let product = temporaries.next().expect("counted");
            operations.push(_made(
                Kind::Mul,
                "imul",
                product,
                vec![
                    offset,
                    Arg::Const(Const::new(masked(coefficient, width), width)),
                ],
                preheader,
                beside,
            ));
            offset = Arg::Held(Held {
                value: product,
                width,
            });
        }
        let result = if one.pointer.is_some() || index != one.offsets.len() - 1 {
            temporaries.next().expect("counted")
        } else {
            into
        };
        match current {
            None => {
                let kind = if matches!(offset, Arg::Cell(_)) {
                    Kind::Load
                } else {
                    Kind::Copy
                };
                operations.push(_made(kind, "mov", result, vec![offset], preheader, beside));
            }
            Some(current) => operations.push(_made(
                Kind::Add,
                "add",
                result,
                vec![
                    Arg::Held(Held {
                        value: current,
                        width,
                    }),
                    offset,
                ],
                preheader,
                beside,
            )),
        }
        current = Some(result);
    }
    if let Some(pointer) = &one.pointer {
        let current = current.expect("a pointer formula has an offset");
        operations.push(_made(
            Kind::PtrOffset,
            "",
            into,
            vec![
                pointer.clone(),
                Arg::Held(Held {
                    value: current,
                    width,
                }),
            ],
            preheader,
            beside,
        ));
    }
    operations
}

/// Every temporary `_starts` needs for this complete affine formula.
///
/// A pointer result cannot reuse `into` for its final offset sum; each
/// product and intermediate sum is counted directly.
fn _start_temporary_count(one: &Derived, counted: bool) -> u32 {
    let unit = matches!(&one.by, Arg::Const(constant) if constant.n == BigInt::from(1_u8));
    let direct_pointer = one.pointer.is_some() && unit && one.offsets.is_empty();
    if direct_pointer || one.pointer.is_none() && one.offsets.is_empty() {
        return 0;
    }
    u32::from(counted)
        + one
            .offsets
            .iter()
            .enumerate()
            .map(|(index, (_offset, coefficient))| {
                u32::from(*coefficient != BigInt::from(1_u8))
                    + u32::from(one.pointer.is_some() || index != one.offsets.len() - 1)
            })
            .sum::<u32>()
}

/// The one value this multiply produces that anything reads, or None.
///
/// Where the high half or the flags are read too, the multiply is doing work
/// an add does not do and it stays.
fn _answer(body: &MirBody, op: OpOccurrence) -> Option<Value> {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let held_result = |op: &Op| match op.results.first() {
        Some(Arg::Held(held)) => Some(held.value),
        _ => None,
    };
    let mut read = operations(body)
        .filter(|(other, _, _)| *other != op)
        .flat_map(|(_, _, other)| other.uses.iter().map(|value| value.id))
        .collect::<BTreeSet<_>>();
    // A phi arm counts only where the phi's own result is read.
    let mut incoming = BTreeMap::<u32, Vec<Value>>::new();
    for block in &body.blocks {
        for phi in &block.phis {
            incoming.insert(phi.result.id, phi.incoming.values().copied().collect());
        }
    }
    let mut pending = read.iter().copied().collect::<Vec<_>>();
    while let Some(next) = pending.pop() {
        for value in incoming.get(&next).into_iter().flatten() {
            if read.insert(value.id) {
                pending.push(value.id);
            }
        }
    }
    let op = op_at(op);
    let wanted = op
        .defines
        .iter()
        .filter(|one| read.contains(&one.id))
        .collect::<Vec<_>>();
    if wanted.len() != 1 || wanted[0].flags || held_result(op) != Some(*wanted[0]) {
        return None;
    }
    Some(*wanted[0])
}

/// The block with the new counter set up and advanced, and the multiply out.
///
/// `ahead` goes at the end of the preheader, `behind` before whatever leaves
/// the latch, because a branch reads the flags something before it set.
/// Each kept operation carries its input occurrence, `None` if it is new.
fn _woven(
    block: &MirBlock,
    ops: &[(OpOccurrence, &Op)],
    ahead: &[Op],
    behind: &[Op],
    replacements: &BTreeMap<OpOccurrence, Vec<Op>>,
) -> Vec<(Option<OpOccurrence>, Op)> {
    let mut kept = Vec::new();
    for (at, op) in ops {
        match replacements.get(at) {
            Some(replacement) => {
                kept.extend(_replaced(replacement).into_iter().map(|made| (None, made)))
            }
            None => kept.push((Some(*at), (*op).clone())),
        }
    }
    if !ahead.is_empty() || !behind.is_empty() {
        let mut cut = kept.len();
        while cut > 0 && matches!(kept[cut - 1].1.kind, Kind::Jump | Kind::Branch) {
            cut -= 1;
        }
        let at = if cut < kept.len() {
            kept[cut].1.at
        } else if let Some((_, last)) = kept.last() {
            last.at
        } else {
            block.at
        };
        let inserted = ahead.iter().chain(behind).map(|op| {
            let mut op = op.clone();
            op.at = at;
            op.absorbed = Vec::new();
            (None, op)
        });
        kept.splice(cut..cut, inserted);
    }
    kept
}

/// The same-block memory users that may name a new pointer phi directly.
///
/// A direct use is safe where the definition dominates its block or precedes
/// it in the same block. Phi inputs retain the original copy.
fn _local_pointer_rebases(
    body: &MirBody,
    bindings: &[(OpOccurrence, Value, Value)],
    replacements: &BTreeMap<OpOccurrence, Vec<Op>>,
) -> BTreeMap<OpOccurrence, BTreeMap<u32, Value>> {
    let place = operations(body)
        .map(|(at, block, _)| (at, (block.at, at.operation_index())))
        .collect::<BTreeMap<_, _>>();
    let dominators = loopy::dominators(&body.blocks, Some(body.entry));
    let mut users = BTreeMap::<u32, Vec<OpOccurrence>>::new();
    for (at, _, op) in operations(body) {
        for value in &op.uses {
            users.entry(value.id).or_default().push(at);
        }
    }
    let mut rebases = BTreeMap::<OpOccurrence, BTreeMap<u32, Value>>::new();
    for (source, answer, carried) in bindings {
        let (source_at, source_index) = place[source];
        let uses = users
            .get(&answer.id)
            .into_iter()
            .flatten()
            .copied()
            .filter(|user| {
                let (user_at, user_index) = place[user];
                !replacements.contains_key(user)
                    && (user_at != source_at
                        && dominators
                            .get(&user_at)
                            .is_some_and(|dominated| dominated.contains(&source_at))
                        || user_at == source_at && user_index > source_index)
            })
            .collect::<Vec<_>>();
        if uses.is_empty() {
            continue;
        }
        // A consumer with two independent carried-pointer bases must retain
        // both identities unless they agree.
        if uses.iter().any(|user| {
            rebases
                .get(user)
                .and_then(|swap| swap.get(&answer.id))
                .is_some_and(|previous| previous != carried)
        }) {
            continue;
        }
        for user in uses {
            rebases.entry(user).or_default().insert(answer.id, *carried);
        }
    }
    rebases
}

fn _replaced(one: &[Op]) -> Vec<Op> {
    one.to_vec()
}

/// This derived address in one form, including exact-width proof.
fn _legal_form(
    body: &MirBody,
    loop_: &Loop,
    one: &Derived,
    form: &AddressForm,
    facts: &IndexMap<Value, Known>,
    widened: &mut BTreeMap<u32, Option<Vec<(OpOccurrence, Op)>>>,
) -> Option<(i64, AddressForm)> {
    let scale = _indexable(body, loop_, one, &form.scales, facts)?;
    if form.index_width > 2 {
        widened
            .entry(one.of.value)
            .or_insert_with(|| _widened(body, loop_, facts, Some(one.of.value)));
        widened[&one.of.value].as_ref()?;
    }
    Some((scale, form.clone()))
}

/// The scale this address is its counter times, where the counter can index it.
///
/// From zero by one, so the counter is the index. A scale above one needs
/// the counter as a dword, and the address is then exact only in bounds.
fn _indexable(
    body: &MirBody,
    _loop: &Loop,
    one: &Derived,
    scales: &BTreeSet<i64>,
    facts: &IndexMap<Value, Known>,
) -> Option<i64> {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let zero = Some(BigInt::from(0_u8));
    let unit = Some(BigInt::from(1_u8));
    let Arg::Const(by) = &one.by else {
        return None;
    };
    let scale = i64::try_from(&by.n).ok();
    if one.pointer.is_some()
        || one.offsets.is_empty()
        || _width(op_at(one.op)) != 2
        || !scale.is_some_and(|scale| scales.contains(&scale))
        || induction::_signed(&one.of.start.as_arg(), facts, 2) != zero
        || induction::_signed(&one.of.step.as_arg(), facts, 2) != unit
    {
        return None;
    }
    let scale = scale.expect("checked");
    let answer = _answer(body, one.op);
    let refs = answer.and_then(|answer| _addressed(body, answer));
    let refs = refs.filter(|refs| !refs.is_empty())?;
    if refs
        .iter()
        .any(|reference| reference.where_() != Some(Space::Far) || reference.base_width != 2)
    {
        return None;
    }
    if scale > 1 && refs.iter().any(|reference| reference.allocation.is_none()) {
        return None;
    }
    Some(scale)
}

/// Whether this is already a cell's base plus its counter, as lowering folds it.
///
/// Reducing it again would give the address back the recurrence the index
/// replaced. A word is folded only unscaled, `[bx+si]`.
fn _indexed(body: &MirBody, one: &Derived) -> bool {
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    let op = op_at(one.op);
    let answer = _answer(body, one.op);
    let Some(answer) = answer else {
        return false;
    };
    if op.kind != Kind::Add || op.args.len() != 2 {
        return false;
    }
    if !op
        .args
        .iter()
        .all(|arg| matches!(arg, Arg::Held(held) if held.width == _width(op)))
    {
        return false;
    }
    let mut made = BTreeMap::<Value, &Op>::new();
    for other in body.blocks.iter().flat_map(|block| &block.ops) {
        for value in &other.defines {
            made.insert(*value, other);
        }
    }
    for arg in &op.args {
        let Arg::Held(arg) = arg else {
            unreachable!("every argument is held");
        };
        let shift = made.get(&arg.value);
        let counted = arg.value.id == one.of.value
            || _width(op) == 4
                && shift.is_some_and(|shift| {
                    shift.kind == Kind::Shl
                        && matches!(shift.args.first(), Some(Arg::Held(held)) if held.value.id == one.of.value)
                });
        if counted {
            return _addressed(body, answer).is_some_and(|refs| !refs.is_empty());
        }
    }
    false
}

/// Every cell `value` is the address of, or None if anything else reads it.
fn _addressed(body: &MirBody, value: Value) -> Option<Vec<MemRef>> {
    let reads = |op: &Op, value: Value| {
        op.args
            .iter()
            .any(|arg| matches!(arg, Arg::Held(held) if held.value == value))
    };
    let fed = body
        .blocks
        .iter()
        .flat_map(|block| &block.phis)
        .filter(|phi| phi.incoming.values().any(|one| *one == value))
        .map(|phi| phi.result)
        .collect::<BTreeSet<_>>();
    let mut refs = Vec::new();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        if op.uses.iter().any(|one| fed.contains(one)) {
            return None;
        }
        if !op.uses.contains(&value) {
            continue;
        }
        let cells = op
            .args
            .iter()
            .chain(&op.results)
            .filter_map(|one| match one {
                Arg::Cell(cell) => Some(&cell.r#ref),
                _ => None,
            })
            .collect::<Vec<_>>();
        let found = cells
            .iter()
            .filter(|reference| reference.base == Some(value))
            .map(|reference| (*reference).clone())
            .collect::<Vec<_>>();
        if found.is_empty()
            || reads(op, value)
            || cells
                .iter()
                .any(|reference| reference.segment == Some(value))
        {
            return None;
        }
        refs.extend(found);
    }
    Some(refs)
}

/// The ops setting and advancing this loop's counter, rewritten as dwords.
///
/// Exact where the counter starts at a word constant no less than zero and
/// stops before it could wrap, and nothing reads the flags its step sets.
fn _widened(
    body: &MirBody,
    loop_: &Loop,
    facts: &IndexMap<Value, Known>,
    value: Option<u32>,
) -> Option<Vec<(OpOccurrence, Op)>> {
    let at_of = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    let header = at_of[&loop_.header];
    let mut made = BTreeMap::<Value, (OpOccurrence, &Op)>::new();
    for (at, _, op) in operations(body) {
        for defined in &op.defines {
            made.insert(*defined, (at, op));
        }
    }
    let mut read = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.uses.iter().copied())
        .collect::<BTreeSet<_>>();
    let phis = body
        .blocks
        .iter()
        .flat_map(|block| &block.phis)
        .collect::<Vec<_>>();
    loop {
        let grown = phis
            .iter()
            .filter(|phi| read.contains(&phi.result))
            .flat_map(|phi| phi.incoming.values().copied())
            .filter(|one| !read.contains(one))
            .collect::<BTreeSet<_>>();
        if grown.is_empty() {
            break;
        }
        read.extend(grown);
    }
    let zero = Some(BigInt::from(0_u8));
    let unit = Some(BigInt::from(1_u8));
    'affines: for affine in induction::basics(body, loop_).values() {
        if value.is_some_and(|value| affine.value != value) {
            continue;
        }
        if induction::_signed(&affine.start.as_arg(), facts, 2) != zero
            || induction::_signed(&affine.step.as_arg(), facts, 2) != unit
        {
            continue;
        }
        if induction::_last_counter(body, loop_, affine, facts, 2).is_none() {
            continue;
        }
        let phi = header.phis.iter().find(|phi| phi.result.id == affine.value);
        let Some(phi) = phi else {
            continue;
        };
        if phi.incoming.len() != 2 {
            continue;
        }
        let ops = phi
            .incoming
            .values()
            .map(|one| made.get(one).copied())
            .collect::<Vec<_>>();
        // A promoted slot the step also writes keeps its word: it reads the low half.
        if ops.iter().any(|op| {
            op.is_none_or(|(_, op)| {
                !op.loads.is_empty()
                    || !op.stores.is_empty()
                    || op.barrier()
                    || op.results.len() != 1
            })
        }) {
            continue;
        }
        let mut out = Vec::new();
        for (at, op) in ops.into_iter().flatten() {
            if op.defines.iter().any(|one| one.flags && read.contains(one)) {
                continue 'affines;
            }
            let args = match (op.kind, op.args.as_slice()) {
                (Kind::Copy, [Arg::Const(Const { n, width: 2 })]) if *n >= BigInt::from(0_u8) => {
                    vec![Arg::Const(Const::new(n.clone(), 4))]
                }
                (
                    Kind::Increment,
                    [
                        Arg::Held(Held {
                            value: counter,
                            width: 2,
                        }),
                    ],
                ) if *counter == phi.result => {
                    vec![Arg::Held(Held {
                        value: *counter,
                        width: 4,
                    })]
                }
                (
                    Kind::Add,
                    [
                        Arg::Held(Held {
                            value: counter,
                            width: 2,
                        }),
                        Arg::Const(one),
                    ],
                ) if one.n == BigInt::from(1_u8) && *counter == phi.result => {
                    vec![
                        Arg::Held(Held {
                            value: *counter,
                            width: 4,
                        }),
                        Arg::Const(Const::new(1, 4)),
                    ]
                }
                _ => continue 'affines,
            };
            let Arg::Held(result) = &op.results[0] else {
                continue 'affines;
            };
            let mut widened = op.clone();
            widened.args = args;
            widened.results = vec![Arg::Held(Held {
                value: result.value,
                width: 4,
            })];
            out.push((at, widened));
        }
        return Some(out);
    }
    None
}

/// `op` with every cell addressed through a widened value saying so.
fn _rebased(op: &Op, wide: &BTreeSet<Value>) -> Op {
    if wide.is_empty() || !op.uses.iter().any(|value| wide.contains(value)) {
        return op.clone();
    }

    let reference = |one: &MemRef| {
        let mut one = one.clone();
        if one.base.is_some_and(|base| wide.contains(&base)) {
            one.base_width = 4;
        }
        one
    };
    let arg = |one: &Arg| match one {
        Arg::Cell(cell) => {
            let mut cell = cell.clone();
            cell.r#ref = reference(&cell.r#ref);
            Arg::Cell(cell)
        }
        other => other.clone(),
    };

    let mut rebased = op.clone();
    rebased.args = op.args.iter().map(arg).collect();
    rebased.results = op.results.iter().map(arg).collect();
    rebased.loads = op.loads.iter().map(reference).collect();
    rebased.stores = op.stores.iter().map(reference).collect();
    rebased
}

/// One operation this pass invented, claiming none of BC's bytes.
pub(crate) fn _made(
    kind: Kind,
    name: &str,
    into: Value,
    args: Vec<Arg>,
    at: i64,
    beside: &Op,
) -> Op {
    let loads = args
        .iter()
        .filter_map(|one| match one {
            Arg::Cell(cell) => Some(cell.r#ref.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut uses = Vec::<Value>::new();
    let held = args.iter().filter_map(|one| match one {
        Arg::Held(held) => Some(held.value),
        _ => None,
    });
    let addressed = loads
        .iter()
        .flat_map(|reference| [reference.base, reference.segment])
        .flatten();
    for value in held.chain(addressed) {
        if !uses.contains(&value) {
            uses.push(value);
        }
    }
    let width = _widest(&args);
    let mut op = Op::new(at, beside.op, name, vec![into], uses);
    op.loads = loads;
    op.stores = Vec::new();
    op.source_backed = false;
    op.kind = kind;
    op.args = args;
    op.results = vec![Arg::Held(Held { value: into, width })];
    op
}

/// `step * by`, where that can be said without an operation.
///
/// A step of one gives the multiplier itself; two constants give their
/// product. Anything else needs a multiply of two invariants.
fn _times(step: &AffineOperand, by: &Arg, _width: u32) -> Option<Arg> {
    if let AffineOperand::Const(step) = step {
        if step.n == BigInt::from(1_u8) {
            return Some(by.clone());
        }
        if let Arg::Const(by) = by {
            return Some(Arg::Const(Const::new(
                &step.n * &by.n,
                step.width.max(by.width),
            )));
        }
    }
    None
}

fn _width(op: &Op) -> u32 {
    for one in &op.results {
        if let Arg::Held(held) = one {
            return held.width;
        }
    }
    2
}

pub(crate) fn _widest(args: &[Arg]) -> u32 {
    args.iter()
        .map(|one| match one {
            Arg::Held(held) => held.width,
            Arg::Const(constant) => constant.width,
            Arg::Symbol(symbol) => symbol.width,
            Arg::FrameAddress(address) => address.width,
            Arg::FrameSelector(selector) => selector.width,
            Arg::Cell(_) | Arg::Opaque(_) => 2,
        })
        .max()
        .unwrap_or(2)
}

/// An id nothing in this body uses.
fn _next(body: &MirBody, taken: u32) -> u32 {
    let mut seen = 0;
    for block in &body.blocks {
        for op in &block.ops {
            for one in op.defines.iter().chain(&op.uses) {
                seen = seen.max(one.id);
            }
        }
        for phi in &block.phis {
            seen = seen.max(phi.result.id);
        }
    }
    seen + 1 + taken * 2
}

#[cfg(test)]
#[path = "strength_tests.rs"]
mod tests;
