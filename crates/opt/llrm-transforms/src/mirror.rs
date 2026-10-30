//! Turns an index that only some trips step into a pointer that steps with it,
//! so a target whose addresses take no scale `k` does not shift the index on
//! every access of `base + k * i`.
//!
//! # How it works
//! In `if (a[j] < p) { ...; ++i; }` the counter `i` is stepped on some paths
//! of the loop and joined, so it is no counter for [`crate::lsr`], which
//! reduces `i * k` only for counters every trip steps. Without a scale every
//! `a[i]` is a shift (`lea bx,[i+i]`) and a base register, where the loop's
//! other accesses want that register too.
//!
//! `induction::step_cycle` finds the cycle: a header phi whose latch value is
//! a join of adds of constants back to the phi. `found` accepts a loop with
//! no loop inside it and a preheader that only enters it, and a cycle of the
//! native index width whose values `y` are scaled (`mul y, k` or `shl y, s`,
//! with `k > 1` and no scale of `AddressForm::scales` for it) into byte
//! `getelementptr`s from one invariant, flat pointer (not a far or huge one,
//! which carries). The family with most readers is taken.
//!
//! `applied` then builds the same cycle over pointers: a phi where the cycle
//! has a phi, and `gep p, step * k` where it has `add y, step`. The pointer
//! phi starts at `base + start * k`, built in the preheader. Each reading
//! `gep` is replaced by the pointer of the node it read.
//!
//! ```text
//!   before                              after
//!   i = phi [0, pre] [i2, latch]        p = phi [base, pre] [p2, latch]
//!   ...                                 ...
//!   o = i * 2 ; q = gep base, o         (q is p)
//!   if (c) { i1 = i + 1 }               if (c) { p1 = gep p, 2 }
//!   i2 = phi [i, ..] [i1, ..]           p2 = phi [p, ..] [p1, ..]
//! ```
//!
//! The new geps are not `inbounds`: the pointer moves with every step of `i`,
//! one of which may leave the object where no access follows. The counter
//! stays for whatever else reads it; a loop that reads it nowhere loses it to
//! `dead`.
//!
//! # Example
//! Before:
//! ```mir
//! define ... @"_keep@3"(ptr %0, ptr %1, i16 %2, i16 %3) ... {
//!   ; ...
//! b2:
//!   %4 = phi i16 [ 0, %b1 ], [ %13, %b5 ]
//!   %5 = phi i16 [ 0, %b1 ], [ %12, %b5 ]
//!   ; ...
//! b5:
//!   %12 = phi i16 [ %5, %b4 ], [ %16, %b6 ]
//!   %13 = add nsw i16 %4, 1
//!   br label %b2
//!
//! b6:
//!   %14 = mul i16 %5, 2
//!   %15 = getelementptr inbounds i8, ptr %1, i16 %14
//!   store i16 %10, ptr %15, !tbaa !9
//!   %16 = add nsw i16 %5, 1
//!   br label %b5
//! }
//! ```
//! After:
//! ```mir
//! define ... @"_keep@3"(ptr %0, ptr %1, i16 %2, i16 %3) ... {
//! b1:
//!   %4 = getelementptr i8, ptr %1, i16 0
//!   ; ...
//! b4:
//!   %10 = phi i16 [ %16, %b5 ], [ 0, %20 ]
//!   %11 = phi ptr [ %15, %b5 ], [ %4, %20 ]
//!   %lsr.iv1 = phi i16 [ %lsr.iv.next, %b5 ], [ %6, %20 ]
//!   ; ...
//! b5:
//!   %15 = phi ptr [ %11, %b4 ], [ %19, %b6 ]
//!   %16 = phi i16 [ %10, %b4 ], [ %18, %b6 ]
//!   ; ...
//! b6:
//!   store i16 %13, ptr %11, !tbaa !9
//!   %18 = add nsw i16 %10, 1
//!   %19 = getelementptr i8, ptr %11, i16 2
//!   br label %b5
//!   ; ...
//! }
//! ```
//! `i` is stepped only on the trips that store, so `lsr` could not reduce
//! `out[i]`. `%11` is the same cycle over pointers: it starts at `out` and
//! steps by 2 where `i` steps by 1. `i` stays for the return value.
//!
//! From `--dump` of `keep` (`if (a[j] < p) { out[i] = a[j]; ++i; }` in a loop
//! over `j`) at -O2 -m16, stages `fixednarrow` and `lsr` (the other changes of
//! `lsr` are left out).
//!
//! # Runs
//! Not a pass: [`crate::lsr`] calls `found` and `applied` on each loop before
//! its counters are chosen, then chooses again. Not at `-Os`, and with
//! `-fstrength-reduce` off nothing runs. Where the target's addresses take the
//! scale (`AddressForm::scales`) there is nothing to save and the loop is left.
//!
//! # See also
//! gcc's ivopts and LLVM's LSR reduce an affine counter only. The effect on
//! quicksort at -m16 is recorded in `docs/reference/levels.md`.

use std::collections::BTreeSet;

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, StepCycle};
use llrm_analysis::{cfg, memory};
use llrm_mir::context::signed;
use llrm_mir::dense::IdSet;
use llrm_mir::module::{BlockId, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::target::AddressForm;
use llrm_mir::types::Type;

use crate::edges;

/// One pointer family: `base + k * f` for the counters of a cycle.
pub(crate) struct Family {
    base: Operand,
    k: i128,
    /// The `getelementptr`s that read it, and the node each reads.
    reads: Vec<(InstId, ValueId)>,
}

/// A loop's counter, stepped on some paths, and the pointers that follow it.
pub(crate) struct Plan {
    header: BlockId,
    preheader: BlockId,
    cycle: StepCycle,
    family: Family,
    bits: u32,
}

/// The pointers `loop_` can have for a counter some trips step, where the
/// target's address takes no scale `forms[0]` names; none at all for a build
/// for size or a loop with another loop in it.
pub(crate) fn found(
    view: &memory::Unit,
    loop_: &Loop,
    forms: &[AddressForm],
) -> Option<Plan> {
    let function = view.function;
    let form = forms.first()?;
    let bits = (form.index_width * 8) as u32;
    let header = cfg::block(loop_.header);
    let body: IdSet<BlockId> = loop_.body.iter().map(|&at| cfg::block(at)).collect();
    if view.shape().loops.iter().any(|other| other.header != loop_.header && loop_.body.contains(&other.header)) {
        return None;
    }
    let outside: Vec<BlockId> = function.predecessors(header).into_iter().filter(|one| !body.contains(one)).collect();
    let [preheader] = outside[..] else { return None };
    if function.successors(preheader) != [header] {
        return None;
    }
    for phi in edges::phis(function, header) {
        if view.int_bits(Operand::Value(function.instruction(phi).result?)) != Some(bits) {
            continue;
        }
        let Some(cycle) = induction::step_cycle(view, loop_, phi) else { continue };
        if let Some(family) = family_of(view, &body, &cycle, &form.scales, bits) {
            return Some(Plan { header, preheader, cycle, family, bits });
        }
    }
    None
}

/// The pointers the cycle's values are scaled into: `gep inbounds base, y * k`
/// for one base and one `k` the target's addresses do not take.
fn family_of(
    view: &memory::Unit,
    body: &IdSet<BlockId>,
    cycle: &StepCycle,
    scales: &BTreeSet<i64>,
    bits: u32,
) -> Option<Family> {
    let function = view.function;
    let invariant = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(inst) => function.parent(inst).is_some_and(|block| !body.contains(&block)),
            _ => true,
        },
        _ => true,
    };
    let constant = |operand: Operand| view.int_constant(operand).map(|value| signed(value, bits));
    let mut families: Vec<Family> = Vec::new();
    for &y in &cycle.nodes {
        for one in function.users(y) {
            let user = function.instruction(one.user);
            if !function.parent(one.user).is_some_and(|block| body.contains(&block)) {
                continue;
            }
            // The pin's canonical form: `gep [k x i8], base, y`, one read of the
            // cycle's value by the pointer it scales.
            if let Opcode::GetElementPtr { source } = user.opcode
                && user.operands.len() == 2
                && user.operands[1] == Operand::Value(y)
            {
                let k = view.layout.alloc_size(&view.context.types, source) as i128;
                let base = user.operands[0];
                let flat = match function.operand_type(view.context, base).map(|ty| view.context.types.get(ty)) {
                    Some(Type::Pointer(space)) => !view.layout.is_pair(*space) && view.layout.offset_bits(*space) == bits,
                    _ => false,
                };
                if k > 1 && !scales.contains(&(k as i64)) && invariant(base) && (flat || std::env::var_os("LLRM_MIRROR_FAR").is_some()) {
                    match families.iter_mut().find(|one| one.base == base && one.k == k) {
                        Some(family) => family.reads.push((one.user, y)),
                        None => families.push(Family { base, k, reads: vec![(one.user, y)] }),
                    }
                }
                continue;
            }
            let k = match user.opcode {
                Opcode::Binary(BinaryOp::Mul) => constant(user.operands[1 - one.index as usize]),
                Opcode::Binary(BinaryOp::Shl) if one.index == 0 => {
                    constant(user.operands[1]).and_then(|s| (0..16).contains(&s).then(|| 1i128 << s))
                }
                _ => None,
            };
            let Some(k) = k.filter(|&k| k > 1 && !scales.contains(&(k as i64))) else { continue };
            let Some(scaled) = user.result else { continue };
            let readers: Vec<InstId> = function.users(scaled).iter().map(|one| one.user).collect();
            if readers.is_empty() {
                continue;
            }
            let mut geps = Vec::new();
            let mut base = None;
            for gep in readers {
                let op = function.instruction(gep);
                let Opcode::GetElementPtr { source } = op.opcode else { break };
                if op.operands.len() != 2
                    || op.operands[1] != Operand::Value(scaled)
                    || view.context.types.int_bits(source) != Some(8)
                {
                    break;
                }
                if base.is_some_and(|base| base != op.operands[0]) || !invariant(op.operands[0]) {
                    break;
                }
                // A pointer that is its offset alone: a far or huge one
                // carries.
                let flat =
                    match function.operand_type(view.context, op.operands[0]).map(|ty| view.context.types.get(ty)) {
                        Some(Type::Pointer(space)) => {
                            !view.layout.is_pair(*space) && view.layout.offset_bits(*space) == bits
                        }
                        _ => false,
                    };
                if !flat && std::env::var_os("LLRM_MIRROR_FAR").is_none() {
                    break;
                }
                base = Some(op.operands[0]);
                geps.push((gep, y));
            }
            let Some(base) = base else { continue };
            if !function.users(scaled).has_n_uses(geps.len()) {
                continue;
            }
            match families.iter_mut().find(|one| one.base == base && one.k == k) {
                Some(family) => family.reads.extend(geps),
                None => families.push(Family { base, k, reads: geps }),
            }
        }
    }
    families.into_iter().max_by_key(|one| one.reads.len())
}

