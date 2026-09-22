//! Port of `qbopt/backend/addressforms.py`: fold address arithmetic into
//! the memory operands that read it.

use std::collections::BTreeSet;
use crate::support::hash::HashMap;
use std::sync::{Arc, LazyLock};

use iced_x86::Register;
use crate::support::hash::{IndexMap, IndexSet};
use num_bigint::BigInt;

use crate::analysis::induction::mod_floor;
use crate::analysis::ranges;
use crate::model::ir::{self, Addr, Loc, Operation, Semantics, Space};
use crate::model::lir::{self, Insn};
use crate::model::mir::{self, Arg, Kind, MirBody, Op};
use crate::model::passes::{AddressForm, OperationCosts};

pub fn offsets(body: &MirBody) -> IndexMap<u32, (ir::Held, BigInt)> {
    let mut result = IndexMap::default();
    for block in &body.blocks {
        for op in &block.ops {
            if !op.loads.is_empty() || !op.stores.is_empty() || op.barrier() {
                continue;
            }
            match (op.kind, op.args.as_slice(), op.results.as_slice()) {
                (
                    Kind::Copy,
                    [Arg::Held(source @ mir::Held { width: 2, .. })],
                    [
                        Arg::Held(mir::Held {
                            value: dest,
                            width: 2,
                        }),
                    ],
                ) => {
                    result.insert(
                        dest.id,
                        (
                            ir::Held {
                                value: source.value.id,
                                width: 2,
                            },
                            BigInt::from(0),
                        ),
                    );
                }
                (
                    Kind::Add,
                    [
                        Arg::Held(source @ mir::Held { width: 2, .. }),
                        Arg::Const(mir::Const {
                            n: amount,
                            width: 2,
                        }),
                    ],
                    [
                        Arg::Held(mir::Held {
                            value: dest,
                            width: 2,
                        }),
                    ],
                ) => {
                    result.insert(
                        dest.id,
                        (
                            ir::Held {
                                value: source.value.id,
                                width: 2,
                            },
                            amount.clone(),
                        ),
                    );
                }
                _ => {}
            }
        }
    }
    result
}

pub fn selected(
    what: Option<&Semantics>,
    forms: &IndexMap<u32, (ir::Held, BigInt)>,
) -> Option<Semantics> {
    let what = what?;

    let operand = |arg: &Loc| -> Loc {
        let Loc::Mem(cell) = arg else {
            return arg.clone();
        };
        let Some(mut base) = cell.base.filter(|base| base.width == 2) else {
            return arg.clone();
        };
        if cell.index.is_some() {
            return arg.clone();
        }
        // A based FAR cell and a non-relocated LITERAL cell both encode the
        // arithmetic displacement beside their base register.  The latter is
        // how a local array reached through SS is represented after its frame
        // root has been folded.  SEGMENT/EXTERNAL/GROUP cells stay out: their
        // displacement is owned by a relocation rather than by this address
        // computation.
        let Some(addr) = cell
            .addr
            .filter(|addr| matches!(addr.space, Space::Far | Space::Literal))
        else {
            return arg.clone();
        };
        let mut offset = BigInt::from(0);
        let mut seen = BTreeSet::new();
        while forms.contains_key(&base.value) && !seen.contains(&base.value) {
            seen.insert(base.value);
            let (next, step) = &forms[&base.value];
            base = *next;
            offset += step;
        }
        if seen.contains(&base.value) {
            return arg.clone();
        }
        let displacement = mod_floor(
            &(BigInt::from(cell.offset) + &offset + 32768),
            &BigInt::from(65536),
        ) - 32768;
        let disp = mod_floor(
            &(BigInt::from(addr.disp) + &offset + 32768),
            &BigInt::from(65536),
        ) - 32768;
        if seen.is_empty() {
            return arg.clone();
        }
        let mut changed = cell.clone();
        changed.base = Some(base);
        changed.offset = i64::try_from(displacement).expect("a wrapped word fits");
        changed.disp_width = 2;
        changed.addr = Some(Addr {
            disp: i64::try_from(disp).expect("a wrapped word fits"),
            ..addr
        });
        Loc::Mem(changed)
    };

    Some(Semantics {
        dests: what.dests.iter().map(operand).collect(),
        sources: what.sources.iter().map(operand).collect(),
        ..what.clone()
    })
}

// Scales 32-bit addressing encodes; 16-bit `[bx+si]` has none but one.
static _SCALES: LazyLock<IndexMap<u32, Vec<i64>>> =
    LazyLock::new(|| [(4, vec![0, 1, 2, 3]), (2, vec![0])].into_iter().collect());

/// Python's `ir.Held | ir.Address`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IndexedBase {
    Held(ir::Held),
    Address(ir::Address),
}

pub type IndexedForm = (IndexedBase, ir::Held, i64);

/// Python's `IndexedForm | ir.Address`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FoldedForm {
    Indexed(IndexedForm),
    Address(ir::Address),
}

/// Based addresses `b + (c << k)` read only by cells, and what computes them.
///
/// The address becomes the cell's `[base+index*scale]` and the add and
/// shift that computed it become nothing. Only where no flag they set is
/// read and nothing but an encodable cell's base reads the address.  This
/// applies equally to far pointers and near pointers into local or global
/// objects; the address width below decides whether a scale is legal.
#[allow(clippy::type_complexity)]
pub fn indexed(
    body: &MirBody,
    exposed: &BTreeSet<u32>,
    address_forms: &[AddressForm],
    costs: Option<&OperationCosts>,
) -> Result<(IndexMap<u32, FoldedForm>, BTreeSet<u32>, BTreeSet<u32>), String> {
    let default = OperationCosts::default();
    let costs = costs.unwrap_or(&default);
    let made: IndexMap<u32, &Op> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().map(move |value| (value.id, op)))
        .collect();
    let block_of: HashMap<*const Op, i64> = body
        .blocks
        .iter()
        .flat_map(|block| block.ops.iter().map(move |op| (op as *const Op, block.at)))
        .collect();
    let constant_offsets = offsets(body);
    let mut frame_bases: IndexMap<u32, ir::Address> = IndexMap::default();
    for op in made.values() {
        if op.kind == Kind::Address
            && op.loads.is_empty()
            && op.stores.is_empty()
            && op.merges.is_empty()
            && !op.barrier()
            && op.args.len() == 1
            && op.results.len() == 1
        {
            if let (
                Arg::FrameAddress(source @ mir::FrameAddress { width: 2, .. }),
                Arg::Held(result @ mir::Held { width: 2, .. }),
            ) = (&op.args[0], &op.results[0])
            {
                frame_bases.insert(
                    result.value.id,
                    ir::Address {
                        through: Register::BP,
                        offset: source.offset,
                        disp_width: if (-128..=127).contains(&source.offset) {
                            1
                        } else {
                            2
                        },
                        ..ir::Address::new(Some(Addr::new(Space::Frame, source.offset)))
                    },
                );
            }
        }
    }
    // Strength reduction may choose a convenient fixed address and derive
    // several elements from it (for example `&x[4] - 16`).  They are all
    // the same encodable BP displacement.  Discover the complete pure
    // constant chain before use classification, so an intermediate address
    // is not mistaken for a value that needs a register merely because a
    // second address expression reads it.
    let mut fixed_candidates = frame_bases.clone();
    loop {
        let before = fixed_candidates.len();
        for op in made.values() {
            if !op.loads.is_empty()
                || !op.stores.is_empty()
                || !op.merges.is_empty()
                || op.barrier()
                || op.results.len() != 1
                || !matches!(op.results[0], Arg::Held(mir::Held { width: 2, .. }))
            {
                continue;
            }
            let mut source: Option<&mir::Held> = None;
            let mut amount = BigInt::from(0);
            if op.kind == Kind::Copy && op.args.len() == 1 {
                if let Arg::Held(held @ mir::Held { width: 2, .. }) = &op.args[0] {
                    source = Some(held);
                }
            } else if op.kind == Kind::Add && op.args.len() == 2 {
                let held = op
                    .args
                    .iter()
                    .filter_map(|one| match one {
                        Arg::Held(one @ mir::Held { width: 2, .. }) => Some(one),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let constants = op
                    .args
                    .iter()
                    .filter_map(|one| match one {
                        Arg::Const(one @ mir::Const { width: 2, .. }) => Some(one),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if held.len() == 1 && constants.len() == 1 {
                    (source, amount) = (Some(held[0]), constants[0].n.clone());
                }
            }
            let Some(source) =
                source.filter(|source| fixed_candidates.contains_key(&source.value.id))
            else {
                continue;
            };
            let fixed = &fixed_candidates[&source.value.id];
            let displacement = i64::try_from(
                mod_floor(
                    &(BigInt::from(fixed.offset) + &amount + 32768),
                    &BigInt::from(65536),
                ) - 32768,
            )
            .expect("a wrapped word fits");
            let Arg::Held(result) = &op.results[0] else {
                unreachable!("checked above")
            };
            let derived = ir::Address {
                addr: fixed.addr.map(|addr| Addr {
                    disp: displacement,
                    ..addr
                }),
                offset: displacement,
                disp_width: if (-128..=127).contains(&displacement) {
                    1
                } else {
                    2
                },
                ..fixed.clone()
            };
            fixed_candidates.insert(result.value.id, derived);
        }
        if fixed_candidates.len() == before {
            break;
        }
    }
    // Python's `Counter`-like dicts: a missing value counts zero.
    let mut bases: IndexMap<u32, i64> = IndexMap::default();
    let mut other: IndexMap<u32, i64> = IndexMap::default();
    let mut constant_bases: BTreeSet<u32> = BTreeSet::new();
    let mut unencodable_constant_bases: BTreeSet<u32> = BTreeSet::new();
    for block in &body.blocks {
        for phi in &block.phis {
            for value in phi.incoming.values() {
                *other.entry(value.id).or_insert(0) += 1;
            }
        }
        for op in &block.ops {
            let cells = op
                .args
                .iter()
                .chain(&op.results)
                .filter_map(|one| match one {
                    Arg::Cell(cell) if cell.r#ref.base.is_some() => Some(&cell.r#ref),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let based = cells
                .iter()
                .filter(|one| {
                    one.addr.is_some() && !matches!(one.where_(), Some(Space::Group | Space::Stack))
                })
                .map(|one| one.base.expect("filtered").id)
                .collect::<BTreeSet<u32>>();
            for cell in &cells {
                let value = cell.base.expect("filtered").id;
                if cell.base_width == 2
                    && cell
                        .addr
                        .is_some_and(|addr| matches!(addr.space, Space::Far | Space::Literal))
                {
                    constant_bases.insert(value);
                } else {
                    unencodable_constant_bases.insert(value);
                }
            }
            let held = op
                .args
                .iter()
                .filter_map(|one| match one {
                    Arg::Held(one) => Some(one.value.id),
                    _ => None,
                })
                .collect::<Vec<u32>>();
            for value in &op.uses {
                if based.contains(&value.id) && !held.contains(&value.id) {
                    *bases.entry(value.id).or_insert(0) += 1;
                } else {
                    *other.entry(value.id).or_insert(0) += 1;
                }
            }
        }
    }
    for &value in exposed {
        *other.entry(value).or_insert(0) += 1;
    }

    let plain = |op: &Op, kind: Kind| -> bool {
        op.kind == kind
            && op.loads.is_empty()
            && op.stores.is_empty()
            && !mir::partial(op)
            && !op.barrier()
            && op.results.len() == 1
            && matches!(op.results[0], Arg::Held(_))
            && !op.defines.iter().any(|one| {
                one.flags && (other.contains_key(&one.id) || bases.contains_key(&one.id))
            })
    };

    // A candidate whose arithmetic flags are observable is a value
    // computation, not merely an address spelling.  Admit derived fixed
    // addresses in dependency order only when deleting their operation is
    // legal; the use classification above is what makes that answer exact.
    let mut fixed_frames = frame_bases.clone();
    loop {
        let before = fixed_frames.len();
        for op in made.values() {
            let result = match op.results.as_slice() {
                [Arg::Held(result)] => Some(result),
                _ => None,
            };
            if !matches!(op.kind, Kind::Copy | Kind::Add)
                || !plain(op, op.kind)
                || op.results.len() != 1
                || !result.is_some_and(|result| fixed_candidates.contains_key(&result.value.id))
                || !op.args.iter().any(
                    |one| matches!(one, Arg::Held(one) if fixed_frames.contains_key(&one.value.id)),
                )
            {
                continue;
            }
            let id = result.expect("checked above").value.id;
            fixed_frames.insert(id, fixed_candidates[&id].clone());
        }
        if fixed_frames.len() == before {
            break;
        }
    }

    let mut forms: IndexMap<u32, FoldedForm> = IndexMap::default();
    // `selected` puts a copied-and-constant-adjusted 16-bit address directly
    // in every based cell.  When cells are the result's only readers, the
    // operation that produced that result is part of the same address fold.
    // Record it here so Lowering expands it to an ownership anchor rather
    // than emitting symbolic arithmetic which machine DCE must conservatively
    // retain.  `plain` is the flag-observability gate; `other` also includes
    // phis and every non-address read.
    let mut folded: BTreeSet<u32> = constant_offsets
        .keys()
        .copied()
        .filter(|value| {
            bases.contains_key(value)
                && constant_bases.contains(value)
                && !unencodable_constant_bases.contains(value)
                && !other.contains_key(value)
                && made.get(value).is_some_and(|defining| {
                    matches!(defining.kind, Kind::Copy | Kind::Add)
                        && plain(defining, defining.kind)
                })
        })
        .collect();
    for (&value, fixed) in &fixed_frames {
        if !bases.contains_key(&value) {
            continue;
        }
        // A frame address consumed directly as a cell base is already the
        // cell's BP displacement. Whether its ADDRESS operation is dead is
        // settled after all dependent address expressions have been folded.
        forms.insert(value, FoldedForm::Address(fixed.clone()));
    }
    for block in &body.blocks {
        for op in &block.ops {
            if !plain(op, Kind::Add) || op.args.len() != 2 {
                continue;
            }
            let Arg::Held(address) = &op.results[0] else {
                unreachable!("plain answers a held result")
            };
            if other.contains_key(&address.value.id)
                || !bases.contains_key(&address.value.id)
                || !_SCALES.contains_key(&address.width)
            {
                continue;
            }
            let held = op
                .args
                .iter()
                .filter(|one| matches!(one, Arg::Held(one) if one.width == address.width))
                .count();
            let constants = op
                .args
                .iter()
                .filter(|one| matches!(one, Arg::Const(one) if one.width == address.width))
                .count();
            if held == 1 && constants == 1 {
                if let Some(fixed) = fixed_frames.get(&address.value.id) {
                    // `&local[0] + 8` is an address spelling, not a value worth
                    // carrying.  Full unrolling exposes many of these with a
                    // literal subscript; keep the 16-bit wrapping arithmetic and
                    // put the result straight in the BP displacement.
                    forms.insert(address.value.id, FoldedForm::Address(fixed.clone()));
                    folded.insert(address.value.id);
                    continue;
                }
            }
            let [Arg::Held(base), Arg::Held(index)] = op.args.as_slice() else {
                continue;
            };
            if base.width != address.width || index.width != address.width {
                continue;
            }
            let fixed = fixed_frames.get(&base.value.id);
            let mut form = (
                fixed.map_or(
                    IndexedBase::Held(ir::Held {
                        value: base.value.id,
                        width: base.width,
                    }),
                    |fixed| IndexedBase::Address(fixed.clone()),
                ),
                ir::Held {
                    value: index.value.id,
                    width: index.width,
                },
                1,
            );
            for (base, index) in [(base, index), (index, base)] {
                let shift = made.get(&index.value.id);
                if let Some(shift) = shift {
                    if plain(shift, Kind::Shl) && shift.args.len() == 2 {
                        if let [Arg::Held(counter), Arg::Const(amount)] = shift.args.as_slice() {
                            if counter.width == address.width
                                && _SCALES[&address.width]
                                    .iter()
                                    .any(|scale| amount.n == BigInt::from(*scale))
                                && other.get(&index.value.id).copied().unwrap_or(0) == 1
                                && !bases.contains_key(&index.value.id)
                            {
                                let fixed = fixed_frames.get(&base.value.id);
                                let amount = i64::try_from(&amount.n).expect("one of _SCALES");
                                form = (
                                    fixed.map_or(
                                        IndexedBase::Held(ir::Held {
                                            value: base.value.id,
                                            width: base.width,
                                        }),
                                        |fixed| IndexedBase::Address(fixed.clone()),
                                    ),
                                    ir::Held {
                                        value: counter.value.id,
                                        width: counter.width,
                                    },
                                    1 << amount,
                                );
                                folded.insert(index.value.id);
                                break;
                            }
                        }
                    }
                }
            }
            forms.insert(address.value.id, FoldedForm::Indexed(form));
            folded.insert(address.value.id);
        }
    }

    // The native word form has no scale.  Before preserving a separately
    // computed `index * scale` (and eventually spilling another value),
    // try the target's explicitly priced secondary form.  Widen the original
    // index and each pointer base at the operations the fold removes, so no
    // additional live range is introduced.  The range gate is semantic: a
    // non-negative word product fitting in 16 bits names the same byte through
    // a 32-bit scaled address; a negative or wrapping product does not.
    let secondary = address_forms
        .iter()
        .find(|form| form.secondary && form.index_width == 4)
        .filter(|secondary| secondary.before_spill(costs));
    let mut promoted: BTreeSet<u32> = BTreeSet::new();
    if let Some(secondary) = secondary {
        let mut use_ops: IndexMap<u32, Vec<&Op>> = IndexMap::default();
        for block in &body.blocks {
            for op in &block.ops {
                for value in &op.uses {
                    use_ops.entry(value.id).or_default().push(op);
                }
            }
        }
        let scoped = ranges::dominated_edges(body)?;
        for product in made.values() {
            if !plain(product, product.kind) || !matches!(product.kind, Kind::Mul | Kind::Shl) {
                continue;
            }
            if product.args.len() != 2 || product.results.len() != 1 {
                continue;
            }
            let source = product.args.iter().find_map(|arg| match arg {
                Arg::Held(arg @ mir::Held { width: 2, .. }) => Some(arg),
                _ => None,
            });
            let amount = product.args.iter().find_map(|arg| match arg {
                Arg::Const(arg @ mir::Const { width: 2, .. }) => Some(arg.n.clone()),
                _ => None,
            });
            let (Some(source), Some(amount)) = (source, amount) else {
                continue;
            };
            if product.kind == Kind::Shl
                && !(BigInt::from(0) <= amount && amount < BigInt::from(16))
            {
                continue;
            }
            let scale = if product.kind == Kind::Mul {
                amount
            } else {
                BigInt::from(1) << u32::try_from(&amount).expect("0 <= amount < 16")
            };
            let result = &product.results[0];
            let (Arg::Held(result @ mir::Held { width: 2, .. }), Some(scale)) = (
                result,
                i64::try_from(&scale)
                    .ok()
                    .filter(|scale| secondary.scales.contains(scale)),
            ) else {
                continue;
            };
            if scale <= 1 || exposed.contains(&result.value.id) {
                continue;
            }
            let Some(additions) = use_ops
                .get(&result.value.id)
                .filter(|additions| !additions.is_empty())
            else {
                continue;
            };
            let mut candidates: Vec<(&Op, &mir::Held, &mir::Held)> = Vec::new();
            let mut safe = true;
            for &addition in additions {
                if !plain(addition, Kind::Add) || addition.args.len() != 2 {
                    safe = false;
                    break;
                }
                let base_args = addition
                    .args
                    .iter()
                    .filter_map(|arg| match arg {
                        Arg::Held(arg @ mir::Held { width: 2, .. })
                            if arg.value.id != result.value.id =>
                        {
                            Some(arg)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let address = match addition.results.as_slice() {
                    [Arg::Held(address)] if base_args.len() == 1 => address,
                    _ => {
                        safe = false;
                        break;
                    }
                };
                if address.width != 2
                    || other.contains_key(&address.value.id)
                    || !bases.contains_key(&address.value.id)
                {
                    safe = false;
                    break;
                }
                let Some(readers) = use_ops
                    .get(&address.value.id)
                    .filter(|readers| !readers.is_empty())
                else {
                    safe = false;
                    break;
                };
                for &reader in readers {
                    let cells = reader
                        .args
                        .iter()
                        .chain(&reader.results)
                        .filter_map(|one| match one {
                            Arg::Cell(one) if one.r#ref.base == Some(address.value) => {
                                Some(&one.r#ref)
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    if cells.is_empty() {
                        safe = false;
                        break;
                    }
                    let fact = scoped
                        .get(&block_of[&(reader as *const Op)])
                        .and_then(|known| known.get(&source.value));
                    let typed_access =
                        !cells.is_empty() && cells.iter().all(|cell| cell.typed.is_some());
                    if fact.is_none_or(|fact| {
                        fact.width != 2
                            || fact.low < BigInt::from(0)
                            // A typed C lvalue may only be evaluated through a
                            // pointer that designates its object.  Any execution
                            // whose scaled offset exceeds the 16-bit segment is
                            // already undefined, so the wider address need agree
                            // only on the defined range.  Untyped/BASIC accesses
                            // retain their explicit 16-bit wrapping semantics.
                            || &fact.high * scale > BigInt::from(0xFFFF) && !typed_access
                    }) {
                        safe = false;
                        break;
                    }
                    if cells.iter().any(|cell| {
                        !cell
                            .addr
                            .is_some_and(|addr| matches!(addr.space, Space::Far | Space::Literal))
                    }) {
                        safe = false;
                        break;
                    }
                }
                if !safe {
                    break;
                }
                candidates.push((addition, base_args[0], address));
            }
            if !safe {
                continue;
            }
            // The word definitions themselves are promoted below after far
            // pointer halves have been selected as one complete load.  Thus
            // the low word and its widened address value remain one live
            // range rather than introducing the very pressure this removes.
            promoted.insert(source.value.id);
            folded.insert(result.value.id);
            for (_addition, base, address) in candidates {
                promoted.insert(base.value.id);
                forms.insert(
                    address.value.id,
                    FoldedForm::Indexed((
                        IndexedBase::Held(ir::Held {
                            value: base.value.id,
                            width: 4,
                        }),
                        ir::Held {
                            value: source.value.id,
                            width: 4,
                        },
                        scale,
                    )),
                );
                folded.insert(address.value.id);
            }
        }
    }

    let phi_reads: BTreeSet<u32> = body
        .blocks
        .iter()
        .flat_map(|block| &block.phis)
        .flat_map(|phi| phi.incoming.values().map(|value| value.id))
        .collect();
    // Prove deletion backwards from actual folded memory operands.  Being a
    // recognizable fixed address is insufficient: its defining arithmetic
    // may remain as an ordinary value computation.  The old forward test
    // accepted any child in `fixed_frames` and deleted the parent LEA while
    // leaving `add child,parent,constant` behind with an undefined source.
    // A chain becomes dead only after every child operation that reads it is
    // itself in `folded`; iterate because the proof runs from leaves to root.
    loop {
        let before = folded.len();
        for &value in fixed_frames.keys() {
            if folded.contains(&value) || exposed.contains(&value) || phi_reads.contains(&value) {
                continue;
            }
            // Python's `for ... else`: added only when no reader breaks.
            let mut refused = false;
            'blocks: for block in &body.blocks {
                for op in &block.ops {
                    if !op.uses.iter().any(|one| one.id == value) {
                        continue;
                    }
                    let based = op
                        .args
                        .iter()
                        .chain(&op.results)
                        .filter_map(|one| match one {
                            Arg::Cell(one) => one.r#ref.base.map(|base| base.id),
                            _ => None,
                        })
                        .collect::<BTreeSet<u32>>();
                    let held = op
                        .args
                        .iter()
                        .any(|one| matches!(one, Arg::Held(one) if one.value.id == value));
                    let derived = held
                        && op
                            .results
                            .iter()
                            .any(|result| matches!(result, Arg::Held(result) if folded.contains(&result.value.id)));
                    if (held && !derived) || (!held && !based.contains(&value)) {
                        refused = true;
                        break 'blocks;
                    }
                }
            }
            if !refused {
                folded.insert(value);
            }
        }
        if folded.len() == before {
            break;
        }
    }
    Ok((forms, folded, promoted))
}

/// Make selected word definitions usable as dword address components.
///
/// A plain word load can directly select `movzx r32,m16`.  A complete far
/// pointer load must still use LES/LFS/LGS, so its offset is first delivered
/// to a short-lived temporary and then zero-extended into the original SSA
/// value.  In both cases the promoted value has one definition and remains
/// the same live range for later low-word uses.
pub fn promote(
    blocks: &IndexMap<i64, Vec<Arc<Insn>>>,
    values: &BTreeSet<u32>,
    fresh: &mut dyn FnMut() -> u32,
) -> Result<IndexMap<i64, Vec<Arc<Insn>>>, String> {
    if values.is_empty() {
        return Ok(blocks.clone());
    }
    let mut definitions: IndexMap<u32, (i64, usize)> = IndexMap::default();
    for (&at, insns) in blocks {
        for (index, one) in insns.iter().enumerate() {
            for &value in &one.defines {
                if values.contains(&value) {
                    definitions.insert(value, (at, index));
                }
            }
        }
    }
    if definitions.keys().copied().collect::<BTreeSet<u32>>() != *values {
        let missing = values
            .iter()
            .copied()
            .filter(|value| !definitions.contains_key(value))
            .collect::<Vec<u32>>();
        return Err(format!(
            "secondary address values have no definition: {missing:?}"
        ));
    }
    let mut out: IndexMap<i64, Vec<Arc<Insn>>> = blocks.clone();
    // Work backwards within each block so inserting a follower cannot move a
    // definition still waiting to be rewritten.
    let mut ordered = definitions.into_iter().collect::<Vec<_>>();
    ordered.sort_by_key(|&(_, (at, index))| (at, -(index as i64)));
    for (value, (at, index)) in ordered {
        let one = Arc::clone(&out[&at][index]);
        let Some(what) = &one.what else {
            return Err(format!(
                "value#{value} has no selected definition to promote"
            ));
        };
        let mut destinations = what.dests.clone();
        let position = destinations.iter().position(
            |destination| matches!(destination, Loc::Held(destination) if destination.value == value && destination.width == 2),
        );
        let Some(position) = position else {
            return Err(format!("value#{value} has no word destination to promote"));
        };
        if what.op == Operation::Move
            && what.name.as_deref() == Some("mov")
            && destinations.len() == 1
            && what.sources.len() == 1
            && match &what.sources[0] {
                Loc::Mem(source) => source.width == 2,
                Loc::Held(source) => source.width == 2,
                _ => false,
            }
        {
            destinations[0] = Loc::Held(ir::Held { value, width: 4 });
            let mut changed = (*one).clone();
            changed.what = Some(Semantics {
                op: Operation::Extend,
                name: Some("movzx".to_owned()),
                dests: destinations,
                ..what.clone()
            });
            changed.widths = one
                .widths
                .iter()
                .copied()
                .chain([(value, 4)])
                .collect::<IndexSet<_>>()
                .into_iter()
                .collect();
            out[&at][index] = Arc::new(changed);
            continue;
        }
        if what.op != Operation::Move
            || !matches!(what.name.as_deref(), Some("les" | "lfs" | "lgs"))
            || position != 0
        {
            let spelled = match what.name.as_deref() {
                Some(name) if !name.is_empty() => name.to_owned(),
                _ => what.op.to_string(),
            };
            return Err(format!("value#{value} cannot be promoted from {spelled}"));
        }
        let temporary = fresh();
        destinations[0] = Loc::Held(ir::Held {
            value: temporary,
            width: 2,
        });
        let mut leader = (*one).clone();
        leader.what = Some(Semantics {
            dests: destinations,
            ..what.clone()
        });
        leader.defines = one
            .defines
            .iter()
            .map(|&found| if found == value { temporary } else { found })
            .collect();
        leader.widths = one
            .widths
            .iter()
            .map(|&(found, width)| (if found == value { temporary } else { found }, width))
            .collect();
        let mut follower = (*lir::anchor(Arc::clone(&one))).clone();
        follower.what = Some(Semantics {
            name: Some("movzx".to_owned()),
            dests: vec![Loc::Held(ir::Held { value, width: 4 })],
            sources: vec![Loc::Held(ir::Held {
                value: temporary,
                width: 2,
            })],
            ..Semantics::new(Operation::Extend)
        });
        follower.defines = vec![value];
        follower.uses = vec![temporary];
        follower.widths = vec![(temporary, 2), (value, 4)];
        follower.op = None;
        follower.node = None;
        out[&at].splice(index..=index, [Arc::new(leader), Arc::new(follower)]);
    }
    Ok(out)
}

/// `what` with every folded far address written as its cell's base and index.
pub fn scaled(what: Option<&Semantics>, forms: &IndexMap<u32, FoldedForm>) -> Option<Semantics> {
    let what = what?;
    if forms.is_empty() {
        return Some(what.clone());
    }

    let operand = |arg: &Loc| -> Loc {
        let Loc::Mem(cell) = arg else {
            return arg.clone();
        };
        let Some(form) = cell.base.and_then(|base| forms.get(&base.value)) else {
            return arg.clone();
        };
        let (base, index, scale) = match form {
            FoldedForm::Address(form) => (IndexedBase::Address(form.clone()), None, 1),
            FoldedForm::Indexed((base, index, scale)) => (base.clone(), Some(*index), *scale),
        };
        match base {
            IndexedBase::Address(base) => {
                let (Some(base_addr), Some(arg_addr)) = (base.addr, cell.addr) else {
                    return arg.clone();
                };
                if base_addr.space != Space::Frame || arg_addr.space != Space::Literal {
                    return arg.clone();
                }
                // A frame address is already BP plus a constant displacement.
                // Keep the dynamic byte offset as the word index and put the
                // constant directly in the memory operand: [bp+si+disp].  The
                // literal spelling says no relocation owns the displacement;
                // SS preserves the frame selector when the data model has DS != SS.
                let displacement = (arg_addr.disp + base.offset + 32768).rem_euclid(65536) - 32768;
                let mut changed = cell.clone();
                if index.is_none() {
                    changed.addr = Some(Addr {
                        disp: displacement,
                        ..base_addr
                    });
                    changed.through = Register::None;
                    changed.base = None;
                    changed.index = None;
                    changed.scale = 1;
                    return Loc::Mem(changed);
                }
                changed.addr = Some(Addr {
                    space: Space::Literal,
                    disp: displacement,
                    segment: Register::SS,
                    ..arg_addr
                });
                changed.through = Register::BP;
                changed.base = None;
                changed.index = index;
                changed.scale = scale;
                Loc::Mem(changed)
            }
            IndexedBase::Held(base) => {
                let mut changed = cell.clone();
                changed.base = Some(base);
                changed.index = index;
                changed.scale = scale;
                changed.through = Register::None;
                Loc::Mem(changed)
            }
        }
    };

    Some(Semantics {
        dests: what.dests.iter().map(operand).collect(),
        sources: what.sources.iter().map(operand).collect(),
        ..what.clone()
    })
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_addressforms.py`, the tests that need no
    //! unported module.

    use std::collections::BTreeSet;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::{FoldedForm, IndexedBase, indexed, scaled};
    use crate::backend::cpu;
    use crate::backend::lower::{self, Place, Placed};
    use crate::model::ir::{self, Addr, Loc, Operation, Semantics, Space};
    use crate::model::mir::{
        self, Arg, Cell, Const, FrameAddress, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Value,
    };

    fn op(at: i64, operation: Operation, name: &str, defines: Vec<Value>, uses: Vec<Value>) -> Op {
        Op::new(at, Some(OpCode::Operation(operation)), name, defines, uses)
    }

    fn held(value: Value, width: u32) -> Arg {
        Arg::Held(mir::Held { value, width })
    }

    fn cell(r#ref: &MemRef) -> Arg {
        Arg::Cell(Cell {
            r#ref: r#ref.clone(),
        })
    }

    fn set(values: &[u32]) -> BTreeSet<u32> {
        values.iter().copied().collect()
    }

    fn based(addr: Addr, width: u32, base: Value, space: Space) -> MemRef {
        MemRef {
            base: Some(base),
            space: Some(space),
            base_width: 2,
            ..MemRef::new(Some(addr), width)
        }
    }

    fn ss(disp: i64) -> Addr {
        Addr {
            segment: Register::SS,
            ..Addr::new(Space::Literal, disp)
        }
    }

    fn cell_of(value: u32, width: u32) -> ir::Mem {
        ir::Mem {
            base: Some(ir::Held { value, width: 2 }),
            ..ir::Mem::new(Some(ss(0)), width)
        }
    }

    fn add_constant(at: i64, result: Value, source: Value, amount: i64) -> Op {
        let mut add = op(at, Operation::Binary, "add", vec![result], vec![source]);
        add.kind = Kind::Add;
        add.args = vec![held(source, 2), Arg::Const(Const::new(amount, 2))];
        add.results = vec![held(result, 2)];
        add
    }

    fn load(
        at: i64,
        operation: Operation,
        name: &str,
        kind: Kind,
        loaded: Value,
        width: u32,
        from: &MemRef,
    ) -> Op {
        let mut load = op(
            at,
            operation,
            name,
            vec![loaded],
            vec![from.base.expect("based")],
        );
        load.kind = kind;
        load.args = vec![cell(from)];
        load.results = vec![held(loaded, width)];
        load.loads = vec![from.clone()];
        load
    }

    fn frame_address(at: i64, frame: Value, offset: i64, extent: (i64, i64)) -> Op {
        let mut made = op(at, Operation::Address, "lea", vec![frame], vec![]);
        made.kind = Kind::Address;
        made.args = vec![Arg::FrameAddress(FrameAddress {
            extent: Some(extent),
            ..FrameAddress::new(offset, 2)
        })];
        made.results = vec![held(frame, 2)];
        made
    }

    fn fld(cell: ir::Mem) -> Semantics {
        Semantics {
            name: Some("fld".to_owned()),
            dests: vec![Loc::St(ir::St { index: 0 })],
            sources: vec![Loc::Mem(cell)],
            ..Semantics::new(Operation::FloatLoad)
        }
    }

    fn source_mem(changed: &Semantics) -> &ir::Mem {
        let Loc::Mem(memory) = &changed.sources[0] else {
            panic!("a memory source")
        };
        memory
    }

    #[test]
    fn test_based_constant_offset_address_computation_is_fully_folded() {
        // Matmul kept the symbolic ADD after its cell became `ss:[base+6]`.
        let (base, address, loaded) = (Value::new(1, 0), Value::new(2, 0), Value::new(3, 0));
        let mut add = add_constant(1, address, base, 6);
        add.symbol = Some(true);
        let r#ref = based(ss(0), 2, address, Space::Frame);
        let load = load(2, Operation::Move, "mov", Kind::Load, loaded, 2, &r#ref);
        let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![add, load], vec![])]);

        let (_forms, folded, _promoted) = indexed(&body, &set(&[]), &[], None).unwrap();

        assert_eq!(folded, set(&[address.id]));
    }

    #[test]
    fn test_whole_word_merge_does_not_keep_folded_address_arithmetic() {
        // qmove's word ADD survived because of its upper-half merge hint.
        let (base, address, loaded) = (Value::new(1, 0), Value::new(2, 0), Value::new(3, 0));
        let mut add = add_constant(1, address, base, 4);
        add.merges.insert(base, address);
        add.symbol = Some(true);
        let r#ref = based(Addr::new(Space::Literal, 0), 4, address, Space::Literal);
        let load = load(2, Operation::Move, "mov", Kind::Load, loaded, 4, &r#ref);
        let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![add, load], vec![])]);

        let (forms, folded, _promoted) = indexed(&body, &set(&[]), &[], None).unwrap();

        assert_eq!(forms, IndexMap::default());
        assert_eq!(folded, set(&[address.id]));
    }

    #[test]
    fn test_relocated_constant_offset_address_computation_is_not_deleted() {
        // A relocation owns a SEGMENT cell's displacement.
        let (base, address, loaded) = (Value::new(1, 0), Value::new(2, 0), Value::new(3, 0));
        let mut add = add_constant(1, address, base, 6);
        add.symbol = Some(true);
        let segment = Addr {
            index: 1,
            ..Addr::new(Space::Segment, 20)
        };
        let r#ref = based(segment, 2, address, Space::Segment);
        let load = load(2, Operation::Move, "mov", Kind::Load, loaded, 2, &r#ref);
        let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![add, load], vec![])]);

        let (_forms, folded, _promoted) = indexed(&body, &set(&[]), &[], None).unwrap();

        assert!(!folded.contains(&address.id));
    }

    #[test]
    fn test_based_local_array_folds_add_into_word_addressing() {
        // shellsort formed `base + (index << 1)` in a third register.
        let (index, shifted, base, address, loaded) = (
            Value::new(1, 0),
            Value::new(2, 1),
            Value::new(3, 0),
            Value::new(4, 2),
            Value::new(5, 3),
        );
        let mut shift = op(1, Operation::Binary, "shl", vec![shifted], vec![index]);
        shift.kind = Kind::Shl;
        shift.args = vec![held(index, 2), Arg::Const(Const::new(1, 2))];
        shift.results = vec![held(shifted, 2)];
        let mut add = op(
            2,
            Operation::Binary,
            "add",
            vec![address],
            vec![base, shifted],
        );
        add.kind = Kind::Add;
        add.args = vec![held(base, 2), held(shifted, 2)];
        add.results = vec![held(address, 2)];
        let r#ref = based(Addr::new(Space::Literal, 0), 2, address, Space::Literal);
        let load = load(3, Operation::Move, "mov", Kind::Load, loaded, 2, &r#ref);
        let body = MirBody::new(
            0,
            vec![MirBlock::new(0, vec![], vec![shift, add, load], vec![])],
        );

        let (forms, folded, _promoted) = indexed(&body, &set(&[]), &[], None).unwrap();

        let expected: IndexMap<u32, FoldedForm> = [(
            address.id,
            FoldedForm::Indexed((
                IndexedBase::Held(ir::Held {
                    value: base.id,
                    width: 2,
                }),
                ir::Held {
                    value: shifted.id,
                    width: 2,
                },
                1,
            )),
        )]
        .into_iter()
        .collect();
        assert_eq!(forms, expected);
        assert_eq!(folded, set(&[address.id]));
    }

    #[test]
    fn test_indexed_frame_array_uses_bp_as_the_encoded_base() {
        // C shellsort emitted `lea bx,[bp-132]` in every hot array block.
        let (index, frame, address, loaded) = (
            Value::new(1, 0),
            Value::new(2, 0),
            Value::new(3, 0),
            Value::new(4, 0),
        );
        let made = frame_address(1, frame, -132, (-132, -4));
        let mut add = op(
            2,
            Operation::Binary,
            "add",
            vec![address],
            vec![frame, index],
        );
        add.kind = Kind::Add;
        add.args = vec![held(frame, 2), held(index, 2)];
        add.results = vec![held(address, 2)];
        let r#ref = MemRef {
            within: Some(vec![(-132, -4)]),
            ..based(Addr::new(Space::Literal, 0), 2, address, Space::Frame)
        };
        let load = load(3, Operation::Move, "mov", Kind::Load, loaded, 2, &r#ref);
        let body = MirBody::new(
            0,
            vec![MirBlock::new(0, vec![], vec![made, add, load], vec![])],
        );
        let (forms, folded, _promoted) = indexed(&body, &set(&[]), &[], None).unwrap();
        let what = Semantics {
            name: Some("mov".to_owned()),
            dests: vec![Loc::Held(ir::Held {
                value: loaded.id,
                width: 2,
            })],
            sources: vec![Loc::Mem(cell_of(address.id, 2))],
            ..Semantics::new(Operation::Move)
        };

        let changed = scaled(Some(&what), &forms);

        assert_eq!(folded, set(&[frame.id, address.id]));
        let changed = changed.expect("changed");
        let indexed = source_mem(&changed);
        assert_eq!(indexed.addr, Some(ss(-132)));
        assert_eq!(indexed.through, Register::BP);
        assert_eq!(indexed.base, None);
        assert_eq!(
            indexed.index,
            Some(ir::Held {
                value: index.id,
                width: 2
            })
        );
    }

    #[test]
    fn test_named_data_address_is_a_relocatable_immediate() {
        // Modern nbody's first string address could not be written to OMF.
        let result = Value::new(2, 1);
        let address = Addr {
            index: 7,
            ..Addr::new(Space::Segment, 4)
        };
        let mut operation = op(1, Operation::Address, "lea", vec![result], vec![]);
        operation.kind = Kind::Address;
        operation.args = vec![Arg::Cell(Cell {
            r#ref: MemRef {
                space: Some(Space::Segment),
                ..MemRef::new(Some(address), 2)
            },
        })];
        operation.results = vec![held(result, 2)];

        let lowered = lower::semantics(&operation, None, Place::AsAValue).unwrap();

        let lowered = lowered.expect("lowered");
        assert_eq!(lowered.op, Operation::Move);
        assert_eq!(lowered.name.as_deref(), Some("mov"));
        assert_eq!(
            lowered.sources,
            vec![Placed::Loc(Loc::Imm(ir::Imm {
                value: 0,
                width: 2,
                address: Some(address)
            }))]
        );
    }

    #[test]
    fn test_chained_constant_frame_addresses_fold_to_one_displacement() {
        // Peeled C nbody spilled `&x[4] - 16` instead of encoding `[bp-20]`.
        let (frame, end, element, loaded) = (
            Value::new(1, 0),
            Value::new(2, 0),
            Value::new(3, 0),
            Value::new(4, 0),
        );
        let made = frame_address(1, frame, -36, (-36, -4));
        let r#ref = MemRef {
            within: Some(vec![(-36, -4)]),
            ..based(Addr::new(Space::Literal, 0), 8, element, Space::Frame)
        };
        let load = load(
            4,
            Operation::FloatLoad,
            "fld",
            Kind::Fload,
            loaded,
            10,
            &r#ref,
        );
        let body = MirBody::new(
            0,
            vec![MirBlock::new(
                0,
                vec![],
                vec![
                    made,
                    add_constant(2, end, frame, 32),
                    add_constant(3, element, end, 65520),
                    load,
                ],
                vec![],
            )],
        );

        let (forms, folded, _promoted) = indexed(&body, &set(&[]), &[], None).unwrap();
        let changed = scaled(Some(&fld(cell_of(element.id, 8))), &forms);

        assert_eq!(folded, set(&[frame.id, end.id, element.id]));
        let changed = changed.expect("changed");
        let direct = source_mem(&changed);
        assert_eq!(direct.addr, Some(Addr::new(Space::Frame, -20)));
        assert_eq!(direct.base, None);
    }

    #[test]
    fn test_frame_address_root_survives_a_live_derived_value() {
        // Peeled C matmul lowered three ADDs from an undefined frame-address root.
        let (frame, derived) = (Value::new(1, 0), Value::new(2, 0));
        let made = frame_address(1, frame, -132, (-132, -4));
        let add = add_constant(2, derived, frame, 14);
        let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![made, add], vec![])]);

        let (forms, folded, _promoted) = indexed(&body, &set(&[derived.id]), &[], None).unwrap();

        assert_eq!(forms, IndexMap::default());
        assert_eq!(folded, set(&[]));
    }

    #[test]
    fn test_secondary_scaled_address_replaces_a_live_word_product_before_spilling() {
        // indexed.lru_use spilled `bnext` while carrying `b * 2`.
        let [index, base, product, address, loaded] =
            [1, 2, 3, 4, 5].map(|number| Value::new(number, i64::from(number)));
        let flags = Value {
            flags: true,
            ..Value::new(6, 1)
        };
        let typed = |name: &str, disp: i64| MemRef {
            typed: Some((name.to_owned(), true)),
            ..MemRef::new(Some(Addr::new(Space::Frame, disp)), 2)
        };
        let (index_ref, base_ref) = (typed("int2", 6), typed("pointer4", 8));
        let read = |at: i64, value: Value, r#ref: &MemRef| {
            let mut read = op(at, Operation::Move, "mov", vec![value], vec![]);
            read.kind = Kind::Load;
            read.args = vec![cell(r#ref)];
            read.results = vec![held(value, 2)];
            read.loads = vec![r#ref.clone()];
            read
        };
        let read_index = read(1, index, &index_ref);
        let read_base = read(2, base, &base_ref);
        let mut compare = op(3, Operation::Compare, "cmp", vec![flags], vec![index]);
        compare.kind = Kind::Sub;
        compare.args = vec![held(index, 2), Arg::Const(Const::new(0, 2))];
        let mut branch = op(4, Operation::Branch, "jge", vec![], vec![flags]);
        branch.kind = Kind::Branch;
        branch.test = Some(Kind::Ge);
        branch.target = Some(7);
        let mut multiply = op(7, Operation::Multiply, "imul", vec![product], vec![index]);
        multiply.kind = Kind::Mul;
        multiply.args = vec![held(index, 2), Arg::Const(Const::new(2, 2))];
        multiply.results = vec![held(product, 2)];
        let mut addition = op(
            8,
            Operation::Binary,
            "add",
            vec![address],
            vec![base, product],
        );
        addition.kind = Kind::Add;
        addition.args = vec![held(base, 2), held(product, 2)];
        addition.results = vec![held(address, 2)];
        let element = MemRef {
            typed: Some(("int2".to_owned(), false)),
            ..based(Addr::new(Space::Far, 0), 2, address, Space::Far)
        };
        let load = load(9, Operation::Move, "mov", Kind::Load, loaded, 2, &element);
        let body = MirBody::new(
            1,
            vec![
                MirBlock::new(
                    1,
                    vec![],
                    vec![read_index, read_base, compare, branch],
                    vec![5, 7],
                ),
                MirBlock::new(5, vec![], vec![], vec![]),
                MirBlock::new(7, vec![], vec![multiply, addition, load], vec![]),
            ],
        );

        let target = cpu::profile("386").unwrap();
        let (forms, folded, promoted) = indexed(
            &body,
            &set(&[]),
            &target.address_forms,
            Some(&target.operations),
        )
        .unwrap();

        assert_eq!(
            forms[&address.id],
            FoldedForm::Indexed((
                IndexedBase::Held(ir::Held {
                    value: base.id,
                    width: 4
                }),
                ir::Held {
                    value: index.id,
                    width: 4
                },
                2,
            ))
        );
        assert!(set(&[product.id, address.id]).is_subset(&folded));
        assert_eq!(promoted, set(&[index.id, base.id]));
    }
}
