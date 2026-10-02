//! Port of `qbopt/backend/addressforms.py`: fold address arithmetic into
//! the memory operands that read it.

use std::rc::Rc;
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

/// Word or dword addresses that are another value plus a constant.
///
/// A word sum wraps as 16-bit addressing does; a dword sum is the 32-bit
/// effective address itself, so either constant is a displacement.
pub fn offsets(body: &MirBody) -> IndexMap<u32, (ir::Held, BigInt)> {
    let mut result = IndexMap::default();
    for block in &body.blocks {
        for op in &block.ops {
            if !op.loads.is_empty() || !op.stores.is_empty() || op.barrier() {
                continue;
            }
            let (source, amount, dest) = match (op.kind, op.args.as_slice(), op.results.as_slice()) {
                (Kind::Copy, [Arg::Held(source)], [Arg::Held(dest)]) => (source, BigInt::from(0), dest),
                (Kind::Add, [Arg::Held(source), Arg::Const(amount)], [Arg::Held(dest)]) if amount.width == dest.width => {
                    (source, amount.n.clone(), dest)
                }
                _ => continue,
            };
            if matches!(dest.width, 2 | 4) && source.width == dest.width {
                result.insert(dest.value.id, (ir::Held { value: source.value.id, width: dest.width }, amount));
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
        let Some(mut base) = cell.base.filter(|base| matches!(base.width, 2 | 4)) else {
            return arg.clone();
        };
        let width = base.width;
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
        // A word read of a dword sum is its low half, which word addressing
        // wraps to anyway; a dword read of a word sum is not the dword sum.
        while forms.get(&base.value).is_some_and(|(next, _)| next.width >= width) && !seen.contains(&base.value) {
            seen.insert(base.value);
            let (next, step) = &forms[&base.value];
            base = ir::Held { value: next.value, width };
            offset += step;
        }
        if seen.contains(&base.value) {
            return arg.clone();
        }
        // A word address wraps; a dword one is the exact 32-bit sum.
        let wrapped = |value: BigInt| {
            if width == 2 {
                mod_floor(&(value + 32768), &BigInt::from(65536)) - 32768
            } else {
                value
            }
        };
        let displacement = wrapped(BigInt::from(cell.offset) + &offset);
        let disp = wrapped(BigInt::from(addr.disp) + &offset);
        if seen.is_empty() {
            return arg.clone();
        }
        let (Ok(displacement), Ok(disp)) = (i64::try_from(&displacement), i64::try_from(&disp)) else {
            return arg.clone();
        };
        if width == 4 && i32::try_from(disp).is_err() {
            return arg.clone();
        }
        let mut changed = cell.clone();
        changed.base = Some(base);
        changed.offset = displacement;
        changed.disp_width = width;
        changed.addr = Some(Addr { disp, ..addr });
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
    /// No base register: the cell is its index, scaled, plus its displacement.
    Absent,
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
    body: &Rc<MirBody>,
    exposed: &BTreeSet<u32>,
    address_forms: &[AddressForm],
    costs: Option<&OperationCosts>,
) -> Result<(IndexMap<u32, FoldedForm>, BTreeSet<u32>, BTreeSet<u32>, BTreeSet<u32>), String> {
    let default = OperationCosts::default();
    let costs = costs.unwrap_or(&default);
    let exact = ranges::exact_offsets(body)?;
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
    // A named data object's near address is a relocated constant: a cell
    // based on it is that object's own displacement, as a frame's is BP's.
    let mut object_bases: IndexMap<u32, ir::Address> = IndexMap::default();
    for op in made.values() {
        if op.kind != Kind::Address || !op.loads.is_empty() || !op.stores.is_empty() || op.barrier() {
            continue;
        }
        if let ([Arg::Cell(cell)], [Arg::Held(result @ mir::Held { width: 2, .. })]) =
            (op.args.as_slice(), op.results.as_slice())
        {
            if let Some(addr) = cell.r#ref.addr.filter(|addr| matches!(addr.space, Space::Segment | Space::External)) {
                if cell.r#ref.base.is_none() {
                    object_bases.insert(result.value.id, ir::Address { disp_width: 2, ..ir::Address::new(Some(addr)) });
                }
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
                if matches!(cell.base_width, 2 | 4)
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
    for (&value, fixed) in &object_bases {
        if bases.contains_key(&value) && !forms.contains_key(&value) {
            forms.insert(value, FoldedForm::Address(fixed.clone()));
        }
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

    // A dword product read only as cell bases, directly or through constant
    // adds `selected` folds into displacements, is those cells' scaled index:
    // `[x+x+d]` for two, `[x*s+d]` otherwise. With a constant array origin
    // there is no base register for the add-and-shift form above to find.
    let mut readers: IndexMap<u32, Vec<&Op>> = IndexMap::default();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        for value in &op.uses {
            readers.entry(value.id).or_default().push(op);
        }
    }
    for product in made.values() {
        if !matches!(product.kind, Kind::Shl | Kind::Mul) || !plain(product, product.kind) {
            continue;
        }
        let ([Arg::Held(result)], [Arg::Held(source), Arg::Const(amount)]) =
            (product.results.as_slice(), product.args.as_slice())
        else {
            continue;
        };
        if result.width != 4 || source.width != 4 || forms.contains_key(&result.value.id) || exposed.contains(&result.value.id) {
            continue;
        }
        let scale = match product.kind {
            Kind::Shl => u32::try_from(&amount.n).ok().filter(|shift| *shift < 4).map(|shift| 1_i64 << shift),
            _ => i64::try_from(&amount.n).ok(),
        };
        let Some(scale) = scale.filter(|scale| _SCALES[&4].iter().any(|shift| 1 << shift == *scale) && *scale > 1) else {
            continue;
        };
        let folds = |value: u32| {
            constant_bases.contains(&value) && !unencodable_constant_bases.contains(&value) && !other.contains_key(&value)
        };
        let uses = readers.get(&result.value.id).map_or(&[][..], Vec::as_slice);
        let mut added = 0;
        let spelled = !uses.is_empty()
            && uses.iter().all(|reader| {
                let held = reader.args.iter().any(|arg| matches!(arg, Arg::Held(arg) if arg.value.id == result.value.id));
                if !held {
                    return true;
                }
                added += 1;
                reader.kind == Kind::Add
                    && reader.results.iter().any(|one| {
                        matches!(one, Arg::Held(one) if folded.contains(&one.value.id) && folds(one.value.id))
                    })
            });
        if !spelled || other.get(&result.value.id).copied().unwrap_or(0) != added {
            continue;
        }
        if bases.contains_key(&result.value.id) && unencodable_constant_bases.contains(&result.value.id) {
            continue;
        }
        let index = ir::Held { value: source.value.id, width: 4 };
        let form = if scale == 2 { (IndexedBase::Held(index), index, 1) } else { (IndexedBase::Absent, index, scale) };
        forms.insert(result.value.id, FoldedForm::Indexed(form));
        folded.insert(result.value.id);
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
        let scoped = ranges::scoped(body)?;
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
                            // only on the defined range.  Any other access
                            // wraps at 16 bits, so its wider sum must be exact.
                            || !typed_access
                                && !exact.contains(&address.value.id)
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
            // `promote` widens a value by rewriting its definition, which it
            // can do for a load or a copy only: a phi or an arithmetic result
            // keeps its word form.
            let promotable = |value: &mir::Value| {
                made.get(&value.id).is_some_and(|op| matches!(op.kind, Kind::Load | Kind::Copy))
            };
            if !safe || !promotable(&source.value) || !candidates.iter().all(|(_, base, _)| promotable(&base.value)) {
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

        // The same product with no base: a cell reads it directly, or through
        // a constant add `selected` folds into the displacement. A constant
        // array origin leaves `x * 2` alone as the address.
        let reads = |address: u32, source: &mir::Held| -> bool {
            use_ops.get(&address).is_some_and(|readers| {
                readers.iter().all(|reader| {
                    let cells = reader
                        .args
                        .iter()
                        .chain(&reader.results)
                        .filter_map(|one| match one {
                            Arg::Cell(one) if one.r#ref.base.is_some_and(|base| base.id == address) => Some(&one.r#ref),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let fact = scoped.get(&block_of[&(*reader as *const Op)]).and_then(|known| known.get(&source.value));
                    !cells.is_empty()
                        && !reader.args.iter().any(|arg| matches!(arg, Arg::Held(arg) if arg.value.id == address))
                        && fact.is_some_and(|fact| fact.width == 2 && fact.low >= BigInt::from(0))
                        && (cells.iter().all(|cell| cell.typed.is_some()) || exact.contains(&address))
                        && cells.iter().all(|cell| cell.addr.is_some_and(|addr| matches!(addr.space, Space::Far | Space::Literal)))
                })
            })
        };
        let promotable = |value: &mir::Value| made.get(&value.id).is_some_and(|op| matches!(op.kind, Kind::Load | Kind::Copy));
        for product in made.values() {
            if !plain(product, product.kind) || !matches!(product.kind, Kind::Mul | Kind::Shl) || folded.contains(&product.defines[0].id) {
                continue;
            }
            let ([Arg::Held(result @ mir::Held { width: 2, .. })], [Arg::Held(source @ mir::Held { width: 2, .. }), Arg::Const(amount)]) =
                (product.results.as_slice(), product.args.as_slice())
            else {
                continue;
            };
            let scale = match product.kind {
                Kind::Shl => u32::try_from(&amount.n).ok().filter(|shift| *shift < 16).map(|shift| 1_i64 << shift),
                _ => i64::try_from(&amount.n).ok(),
            };
            let Some(scale) = scale.filter(|scale| *scale > 1 && secondary.scales.contains(scale)) else {
                continue;
            };
            if exposed.contains(&result.value.id) || !promotable(&source.value) {
                continue;
            }
            let uses = use_ops.get(&result.value.id).map_or(&[][..], Vec::as_slice);
            let mut added = 0;
            let spelled = !uses.is_empty()
                && uses.iter().all(|reader| {
                    if !reader.args.iter().any(|arg| matches!(arg, Arg::Held(arg) if arg.value.id == result.value.id)) {
                        return reads(result.value.id, source);
                    }
                    added += 1;
                    reader.kind == Kind::Add
                        && reader.results.iter().any(|one| {
                            matches!(one, Arg::Held(one) if folded.contains(&one.value.id)
                                && constant_offsets.contains_key(&one.value.id)
                                && reads(one.value.id, source))
                        })
                });
            if !spelled || other.get(&result.value.id).copied().unwrap_or(0) != added {
                continue;
            }
            let index = ir::Held { value: source.value.id, width: 4 };
            let form = if scale == 2 { (IndexedBase::Held(index), index, 1) } else { (IndexedBase::Absent, index, scale) };
            promoted.insert(source.value.id);
            forms.insert(result.value.id, FoldedForm::Indexed(form));
            folded.insert(result.value.id);
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
        for &value in fixed_frames.keys().chain(object_bases.keys().filter(|value| forms.contains_key(*value))) {
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
    Ok((forms, folded, promoted, exact))
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

/// `what` with every folded far address written as its cell's base and index,
/// and each cell whose address `exact` names marked exact.
pub fn scaled(what: Option<&Semantics>, forms: &IndexMap<u32, FoldedForm>, exact: &BTreeSet<u32>) -> Option<Semantics> {
    let what = what?;
    let operand = |arg: &Loc| -> Loc {
        let Loc::Mem(cell) = arg else {
            return arg.clone();
        };
        let cell = &ir::Mem { exact: cell.base.is_some_and(|base| exact.contains(&base.value)), ..cell.clone() };
        let arg = &Loc::Mem(cell.clone());
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
                if matches!(base_addr.space, Space::Segment | Space::External)
                    && arg_addr.space == Space::Literal
                    && index.is_none()
                {
                    let mut changed = cell.clone();
                    changed.addr =
                        Some(Addr { disp: base_addr.disp + arg_addr.disp, segment: arg_addr.segment, ..base_addr });
                    changed.through = Register::None;
                    changed.base = None;
                    changed.index = None;
                    changed.scale = 1;
                    return Loc::Mem(changed);
                }
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
            IndexedBase::Absent => {
                let mut changed = cell.clone();
                changed.base = None;
                changed.index = index;
                changed.scale = scale;
                changed.through = Register::None;
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
