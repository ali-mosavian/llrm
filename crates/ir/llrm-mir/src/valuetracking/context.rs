//! Facts from the context a query is asked in: `llvm.assume` calls, the
//! conditions of the branches that dominate the context instruction, and
//! `isImpliedCondition`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`computeKnownBitsFromContext`, `isValidAssumeForContext`,
//! `findValuesAffectedByCondition`, `isImpliedCondition`,
//! `isImpliedByDomCondition` and what they call), each function linking the
//! line it translates. `Query::dt`, `Query::assumptions` and
//! `Query::dom_conditions` and `Query::cond_context` are `SQ.DT`, `SQ.AC`,
//! `SQ.DC` and `SQ.CC`: absent, nothing here applies. Not translated, as MIR
//! has no such thing:
//! - `computeKnownFPClassFromContext`'s use of `Q.CC` (floating point).
//! - Operand bundles: `assumeBundleImpliesNonNull` and the `align` bundle
//!   (L856-863, L1085-1100).
//! - Floating point comparisons: `isImpliedCondFCmps` (L10088-10147) needs
//!   `ConstantFPRange`; `isImpliedCondition` answers nothing for an `fcmp`
//!   (L10200-10208, L10247-10253).
//! - Vector constants: `cmpExcludesZero` on a `ConstantDataVector` (L796-808),
//!   `m_APInt` splats.
//! - Atomics and fences, which MIR lacks: `maySynchronize` is false for all but
//!   calls (L736-747).
//! - `Q.AllowEphemerals` is always false.
//!
//! # Example
//! After `call void @llvm.assume(i1 %c)` with `%c = icmp ult i32 %x, 16`, `%x`
//! has its top 28 bits known zero at any later instruction.

use super::MAX_ANALYSIS_RECURSION_DEPTH;
use super::pattern::{
    CmpPredicate, Side, add_like, binary, cast, commutative_intrinsic_with, commutative_with, constant_int_u64, fneg,
    icmp, icmp_like, imm_constant, intrinsic_call, logical_and, logical_op, logical_or, not, power2_constant, select,
    zero,
};
use super::query::Query;
use crate::apint::{countl_one, countl_zero, set_high, signed};
use crate::constantrange::ConstantRange;
use crate::context::{ConstantKind, mask};
use crate::hash::HashMap;
use crate::knownbits::KnownBits;
use crate::module::{BlockId, InstId, Operand, ValueDef};
use crate::opcode::{Attribute, BinaryOp, CastOp, Flags, IntPredicate, Opcode};

/// `MaxInstrsToCheckForFree`.
const MAX_INSTRS_TO_CHECK_FOR_FREE: u32 = 32;

/// `BasicBlock::getSinglePredecessor`: the block with the one edge into
/// `block`.
pub(super) fn single_predecessor(
    q: &Query,
    block: BlockId,
) -> Option<BlockId> {
    let mut found = None;
    for one in q.function.block_users(block) {
        if !q.function.instruction(one.user).opcode.is_terminator() {
            continue;
        }
        let Some(parent) = q.function.parent(one.user) else { continue };
        if found.is_some() {
            return None;
        }
        found = Some(parent);
    }
    found
}

fn callee_attrs<'a>(
    q: &Query<'a>,
    call: InstId,
) -> Option<&'a [Attribute]> {
    let global = crate::memory::callee(q.context, q.function, call)?;
    q.globals.get(global.0 as usize)?.function().map(|function| function.attrs.as_slice())
}

/// `CallBase::hasFnAttr(Kind)`: on the call or on the function it calls.
fn has_fn_attr(
    q: &Query,
    call: InstId,
    flag: &str,
) -> bool {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &q.function.instruction(call).opcode else { return false };
    crate::memory::has(&info.attrs, flag) || callee_attrs(q, call).is_some_and(|attrs| crate::memory::has(attrs, flag))
}

/// `Instruction::mayHaveSideEffects`, of an instruction that is no terminator.
fn may_have_side_effects(
    q: &Query,
    inst: InstId,
) -> bool {
    match &q.function.instruction(inst).opcode {
        Opcode::Store { .. } | Opcode::Load { volatile: true, .. } => true,
        Opcode::Call(info) => {
            let callee = callee_attrs(q, inst);
            let declared = callee.map_or(crate::memory::Effects::ANY, crate::memory::stated);
            let writes = crate::memory::stated(&info.attrs).writes && declared.writes;
            let site = crate::facts::Facts::of(&info.attrs);
            let callee = callee.map(crate::facts::Facts::of);
            let no_unwind = site.no_unwind() || callee.as_ref().is_some_and(|one| one.no_unwind());
            let will_return = site.will_return() || callee.as_ref().is_some_and(|one| one.will_return());
            writes || !no_unwind || !will_return
        }
        _ => false,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L636>
fn is_ephemeral_value_of(
    q: &Query,
    i: InstId,
    e: InstId,
) -> bool {
    if let Some(result) = q.function.instruction(e).result
        && q.function.instruction(i).operands.contains(&Operand::Value(result))
    {
        return true;
    }
    if e == i {
        return true;
    }
    let mut visited = vec![e];
    let mut work_list = vec![e];
    let mut reaches_i = false;
    while let Some(v) = work_list.pop() {
        let Some(result) = q.function.instruction(v).result else { continue };
        for one in q.function.users(result) {
            let ui = one.user;
            if ui == i {
                reaches_i = true;
                continue;
            }
            if q.function.instruction(ui).opcode.is_terminator() || may_have_side_effects(q, ui) {
                return false;
            }
            if !visited.contains(&ui) {
                visited.push(ui);
                work_list.push(ui);
            }
        }
    }
    reaches_i
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L673>
pub fn is_assume_like_intrinsic(
    q: &Query,
    inst: InstId,
) -> bool {
    const NAMES: [&str; 15] = [
        "llvm.assume",
        "llvm.sideeffect",
        "llvm.pseudoprobe",
        "llvm.dbg.assign",
        "llvm.dbg.declare",
        "llvm.dbg.value",
        "llvm.dbg.label",
        "llvm.invariant.start",
        "llvm.invariant.end",
        "llvm.lifetime.start",
        "llvm.lifetime.end",
        "llvm.experimental.noalias.scope.decl",
        "llvm.objectsize",
        "llvm.ptr.annotation",
        "llvm.var.annotation",
    ];
    let Some(name) = q.callee_name(inst) else { return false };
    NAMES.iter().any(|base| name.strip_prefix(base).is_some_and(|rest| rest.is_empty() || rest.starts_with('.')))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L680>
pub fn is_valid_assume_for_context(
    q: &Query,
    assume: InstId,
    cxt: InstId,
    allow_ephemerals: bool,
) -> bool {
    let (Some(inv_block), Some(cxt_block)) = (q.block_of(assume), q.block_of(cxt)) else { return false };
    if inv_block == cxt_block {
        if q.comes_before(assume, cxt) {
            return true;
        }
        if !allow_ephemerals && assume == cxt {
            return false;
        }
        if !super::guarantee::is_guaranteed_to_transfer_execution_to_successor_range(
            q,
            q.function.iter_range(cxt, assume),
            15,
        ) {
            return false;
        }
        return allow_ephemerals || !is_ephemeral_value_of(q, assume, cxt);
    }
    if let Some(dt) = q.dt {
        if dt.instruction_dominates(q.function, assume, cxt) {
            return true;
        }
    } else if Some(inv_block) == single_predecessor(q, cxt_block) || Some(inv_block) == q.function.entry() {
        return true;
    }
    false
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L730>
pub fn will_not_free_between(
    q: &Query,
    assume: InstId,
    ctx: InstId,
) -> bool {
    let mut checked = 0_u32;
    let mut has_no_free_in_range = |range: &mut dyn Iterator<Item = InstId>| -> bool {
        for inst in range {
            let n = checked;
            checked += 1;
            if n > MAX_INSTRS_TO_CHECK_FOR_FREE {
                return false;
            }
            if matches!(
                q.function.instruction(inst).opcode,
                Opcode::Call(_) | Opcode::Invoke(_)
            )
                && !has_fn_attr(q, inst, "nofree")
            {
                return false;
            }
        }
        true
    };
    let (Some(ctx_block), Some(assume_block)) = (q.block_of(ctx), q.block_of(assume)) else { return false };
    if ctx_block == assume_block {
        if assume != ctx && !q.comes_before(assume, ctx) {
            return false;
        }
        return has_no_free_in_range(&mut q.function.iter_range(assume, ctx));
    }
    let mut cur = ctx_block;
    loop {
        if cur == assume_block {
            return has_no_free_in_range(&mut q.function.iter_from(assume));
        }
        let Some(pred) = single_predecessor(q, cur) else { return false };
        // The context's own block is read up to it, the others whole.
        let upto = (cur == ctx_block).then_some(ctx);
        if !has_no_free_in_range(&mut q.function.iter_block(cur).take_while(|&one| Some(one) != upto)) {
            return false;
        }
        cur = pred;
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L781>
fn cmp_excludes_zero(
    q: &Query,
    pred: IntPredicate,
    rhs: Operand,
) -> bool {
    if pred == IntPredicate::Ugt {
        return true;
    }
    if pred == IntPredicate::Ne {
        return zero(q, rhs);
    }
    if let Some((c, width)) = q.int_constant(rhs) {
        return !ConstantRange::make_exact_icmp_region(pred, width, c).contains(0);
    }
    false
}

/// `U` is operand `use_index` of `phi` (a value; its block follows it).
/// Returns `ValOut`; `cxt_out` is `CtxIOut` and `phi_out` is `PhiOut`, both
/// left alone where LLVM leaves them.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L812>
pub fn break_self_recursive_phi(
    q: &Query,
    phi: InstId,
    use_index: usize,
    cxt_out: &mut Option<InstId>,
    mut phi_out: Option<&mut InstId>,
) -> Operand {
    let phi_instruction = q.function.instruction(phi);
    let phi_value = Operand::Value(phi_instruction.result.expect("a phi has a result"));
    let mut val_out = phi_instruction.operands[use_index];
    if val_out == phi_value {
        return val_out;
    }
    let Operand::Block(block) = phi_instruction.operands[use_index + 1] else { unreachable!("a phi names blocks") };
    *cxt_out = q.function.terminator(block);
    if let Some(out) = phi_out.as_deref_mut() {
        *out = phi;
    }
    if let Some((_, _, _, t, f)) = select(q, val_out) {
        if t == phi_value {
            val_out = f;
        } else if f == phi_value {
            val_out = t;
        }
    }
    if let Some((inc_phi, inc)) = q.instruction_of(val_out)
        && inc.opcode == Opcode::Phi
        && inc.operands.len() == 4
    {
        for idx in 0..2 {
            if inc.operands[idx * 2] == phi_value {
                val_out = inc.operands[(1 - idx) * 2];
                if let Some(out) = phi_out.as_deref_mut() {
                    *out = inc_phi;
                }
                let Operand::Block(block) = inc.operands[(1 - idx) * 2 + 1] else { unreachable!("a phi names blocks") };
                *cxt_out = q.function.terminator(block);
                break;
            }
        }
    }
    val_out
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L847>
pub fn is_known_non_zero_from_assume(
    q: &Query,
    v: Operand,
) -> bool {
    let (Some(assumptions), Some(cxt)) = (q.assumptions, q.cxt) else { return false };
    let is_v = |x: Operand| x == v || cast(q, x, CastOp::PtrToInt).is_some_and(|(_, _, op)| op == v);
    for &assume in assumptions.assumptions_for(v) {
        // m_c_ICmp(Pred, m_V, m_Value(RHS))
        let Some((_, _, pred, a, b)) = icmp(q, q.function.instruction(assume).operands[0]) else { continue };
        let (pred, rhs) = if is_v(a) {
            (pred.pred, b)
        } else if is_v(b) {
            (pred.pred.swapped(), a)
        } else {
            continue;
        };
        if cmp_excludes_zero(q, pred, rhs) && is_valid_assume_for_context(q, assume, cxt, false) {
            return true;
        }
    }
    false
}

/// `m_V`: the value, or a `ptrtoint` of it to the same size.
fn is_v(
    q: &Query,
    x: Operand,
    v: Operand,
) -> bool {
    x == v || cast(q, x, CastOp::PtrToInt).is_some_and(|(_, _, op)| op == v && q.scalar_bits(x) == q.scalar_bits(op))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L886>
fn compute_known_bits_from_cmp(
    q: &Query,
    v: Operand,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
    known: &mut KnownBits,
) {
    if q.is_pointer(rhs) {
        if lhs == v && zero(q, rhs) {
            match pred {
                IntPredicate::Eq => known.set_all_zero(),
                IntPredicate::Sge | IntPredicate::Sgt => known.make_non_negative(),
                IntPredicate::Slt => known.make_negative(),
                _ => {}
            }
        }
        return;
    }
    let width = known.width;
    let m_v = |x: Operand| is_v(q, x, v);
    let Some((c, _)) = q.int_constant(rhs) else { return };
    match pred {
        IntPredicate::Eq => {
            if m_v(lhs) {
                *known = known.union_with(&KnownBits::constant(width, c));
            } else if let Some((_, _, y)) = commutative_with(q, lhs, BinaryOp::And, m_v) {
                known.one |= c;
                if let Some((m, _)) = q.int_constant(y) {
                    known.zero |= !c & m;
                }
            } else if let Some((_, _, y)) = commutative_with(q, lhs, BinaryOp::Or, m_v) {
                known.zero |= !c & mask(width);
                if let Some((m, _)) = q.int_constant(y) {
                    known.one |= c & !m;
                }
            } else if let Some(sh) = binary(q, lhs, BinaryOp::Shl)
                .filter(|(_, _, a, _)| m_v(*a))
                .and_then(|(_, _, _, b)| constant_int_u64(q, b))
                .filter(|sh| *sh < u64::from(width))
            {
                *known = known.union_with(&KnownBits::constant(width, c).lshr_by(sh as u32));
            } else if let Some(sh) = [BinaryOp::LShr, BinaryOp::AShr]
                .into_iter()
                .find_map(|op| binary(q, lhs, op))
                .filter(|(_, _, a, _)| m_v(*a))
                .and_then(|(_, _, _, b)| constant_int_u64(q, b))
                .filter(|sh| *sh < u64::from(width))
            {
                *known = known.union_with(&KnownBits::constant(width, c).shl_by(sh as u32));
            }
        }
        IntPredicate::Ne => {
            if c == 0
                && let Some((_, _, _, b)) = binary(q, lhs, BinaryOp::And).filter(|(_, _, a, _)| m_v(*a))
                && let Some(pow2) = power2_constant(q, b)
            {
                known.one |= pow2;
            }
        }
        _ => {
            let offset = if m_v(lhs) {
                Some(None)
            } else if let Some((a, b)) = add_like(q, lhs, Flags::default())
                && m_v(a)
                && let Some((off, _)) = q.int_constant(b)
            {
                Some(Some(off))
            } else {
                None
            };
            if let Some(offset) = offset {
                let mut lhs_range = ConstantRange::make_allowed_icmp_region(pred, &ConstantRange::single(width, c));
                if let Some(off) = offset {
                    lhs_range = lhs_range.subtract(off);
                }
                *known = known.union_with(&lhs_range.to_known_bits());
            }
            if pred == IntPredicate::Ugt || pred == IntPredicate::Uge {
                let nuw_sub = binary(q, lhs, BinaryOp::Sub)
                    .is_some_and(|(_, ins, a, _)| ins.flags.contains(Flags::NUW) && m_v(a));
                if commutative_with(q, lhs, BinaryOp::And, m_v).is_some() || nuw_sub {
                    let bumped = c.wrapping_add(u128::from(pred == IntPredicate::Ugt)) & mask(width);
                    known.one = set_high(known.one, width, countl_one(bumped, width));
                }
            }
            if pred == IntPredicate::Ult || pred == IntPredicate::Ule {
                let nuw_add = binary(q, lhs, BinaryOp::Add)
                    .is_some_and(|(_, ins, a, b)| ins.flags.contains(Flags::NUW) && (m_v(a) || m_v(b)));
                if commutative_with(q, lhs, BinaryOp::Or, m_v).is_some() || nuw_add {
                    let lowered = c.wrapping_sub(u128::from(pred == IntPredicate::Ult)) & mask(width);
                    known.zero = set_high(known.zero, width, countl_zero(lowered, width));
                }
            }
        }
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L992>
fn compute_known_bits_from_icmp_cond(
    q: &Query,
    v: Operand,
    cmp: InstId,
    known: &mut KnownBits,
    invert: bool,
) {
    let instruction = q.function.instruction(cmp);
    let Opcode::ICmp(cmp_pred) = instruction.opcode else { return };
    let pred = if invert { cmp_pred.inverse() } else { cmp_pred };
    let (lhs, rhs) = (instruction.operands[0], instruction.operands[1]);
    if let Some((_, trunc, op)) = cast(q, lhs, CastOp::Trunc)
        && op == v
    {
        let mut dst_known = KnownBits::unknown(q.scalar_bits(lhs));
        compute_known_bits_from_cmp(q, lhs, pred, lhs, rhs, &mut dst_known);
        if trunc.flags.contains(Flags::NUW) {
            *known = known.union_with(&dst_known.zext(known.width));
        } else {
            *known = known.union_with(&dst_known.anyext(known.width));
        }
        return;
    }
    compute_known_bits_from_cmp(q, v, pred, lhs, rhs, known);
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L1014>
pub fn compute_known_bits_from_cond(
    q: &Query,
    v: Operand,
    cond: Operand,
    known: &mut KnownBits,
    invert: bool,
    depth: u32,
) {
    if depth < MAX_ANALYSIS_RECURSION_DEPTH
        && let Some((a, b)) = logical_op(q, cond)
    {
        let mut known2 = KnownBits::unknown(known.width);
        let mut known3 = KnownBits::unknown(known.width);
        compute_known_bits_from_cond(q, v, a, &mut known2, invert, depth + 1);
        compute_known_bits_from_cond(q, v, b, &mut known3, invert, depth + 1);
        let unioned = if invert { logical_or(q, cond).is_some() } else { logical_and(q, cond).is_some() };
        known2 = if unioned { known2.union_with(&known3) } else { known2.intersect_with(&known3) };
        *known = known.union_with(&known2);
        return;
    }
    if let Some((cmp, ..)) = icmp(q, cond) {
        compute_known_bits_from_icmp_cond(q, v, cmp, known, invert);
        return;
    }
    if let Some((_, trunc, op)) = cast(q, cond, CastOp::Trunc)
        && op == v
    {
        let mut dst_known = KnownBits::unknown(1);
        if invert {
            dst_known.set_all_zero();
        } else {
            dst_known.set_all_ones();
        }
        if trunc.flags.contains(Flags::NUW) {
            *known = known.union_with(&dst_known.zext(known.width));
            return;
        }
        *known = known.union_with(&dst_known.anyext(known.width));
        return;
    }
    if depth < MAX_ANALYSIS_RECURSION_DEPTH
        && let Some(a) = not(q, cond)
    {
        compute_known_bits_from_cond(q, v, a, known, !invert, depth + 1);
    }
}

/// `DominatorTree::dominates(BasicBlockEdge(start, end), use_block)`.
pub(super) fn edge_dominates(
    q: &Query,
    start: BlockId,
    end: BlockId,
    use_block: BlockId,
) -> bool {
    let Some(dt) = q.dt else { return false };
    if !dt.dominates(end, use_block) {
        return false;
    }
    if single_predecessor(q, end).is_some() {
        return true;
    }
    let mut duplicate = false;
    for one in q.function.block_users(end) {
        if !q.function.instruction(one.user).opcode.is_terminator() {
            continue;
        }
        let Some(pred) = q.function.parent(one.user) else { continue };
        if pred == start {
            if duplicate {
                return false;
            }
            duplicate = true;
            continue;
        }
        if !dt.dominates(end, pred) {
            return false;
        }
    }
    true
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L1057>
pub fn compute_known_bits_from_context(
    q: &Query,
    v: Operand,
    known: &mut KnownBits,
    depth: u32,
) {
    if let Some(cc) = q.cond_context
        && cc.affected_values.contains(&v)
    {
        compute_known_bits_from_cond(q, v, cc.cond, known, cc.invert, depth);
    }

    let Some(cxt) = q.cxt else { return };
    if let (Some(conditions), Some(_)) = (q.dom_conditions, q.dt) {
        let cxt_block = q.block_of(cxt);
        for &branch in conditions.conditions_for(v).iter() {
            let instruction = q.function.instruction(branch);
            let (Some(start), Some(cxt_block), Operand::Block(taken), Operand::Block(otherwise)) =
                (q.block_of(branch), cxt_block, instruction.operands[1], instruction.operands[2])
            else {
                continue;
            };
            if edge_dominates(q, start, taken, cxt_block) {
                compute_known_bits_from_cond(q, v, instruction.operands[0], known, false, depth);
            }
            if edge_dominates(q, start, otherwise, cxt_block) {
                compute_known_bits_from_cond(q, v, instruction.operands[0], known, true, depth);
            }
        }
        if known.has_conflict() {
            known.reset_all();
        }
    }
    let Some(assumptions) = q.assumptions else { return };
    for &assume in assumptions.assumptions_for(v) {
        let arg = q.function.instruction(assume).operands[0];
        if arg == v && is_valid_assume_for_context(q, assume, cxt, false) {
            known.set_all_ones();
            return;
        }
        if not(q, arg) == Some(v) && is_valid_assume_for_context(q, assume, cxt, false) {
            known.set_all_zero();
            return;
        }
        if let Some((_, trunc, op)) = cast(q, arg, CastOp::Trunc)
            && op == v
            && is_valid_assume_for_context(q, assume, cxt, false)
        {
            if trunc.flags.contains(Flags::NUW) {
                *known = KnownBits::constant(known.width, 1);
                return;
            }
            known.one |= 1;
            return;
        }
        if depth == MAX_ANALYSIS_RECURSION_DEPTH {
            continue;
        }
        let Some((cmp, ..)) = icmp(q, arg) else { continue };
        if !is_valid_assume_for_context(q, assume, cxt, false) {
            continue;
        }
        compute_known_bits_from_icmp_cond(q, v, cmp, known, false);
    }
    if known.has_conflict() {
        known.reset_all();
    }
}

/// `CmpInst::isTrueWhenEqual`.
fn is_true_when_equal(pred: IntPredicate) -> bool {
    pred.true_when_equal()
}

/// `m_Specific(side)` of `x`.
fn is_side(
    q: &Query,
    x: Operand,
    side: Side,
) -> bool {
    Side::of(q, x) == side
}

fn side_operand(side: Side) -> Option<Operand> {
    match side {
        Side::Value(v) => Some(v),
        Side::Int(..) => None,
    }
}

/// `m_NSWAddLike(m_Value(X), m_APInt(C))`.
fn nsw_add_like_const(
    q: &Query,
    v: Operand,
) -> Option<(Operand, u128)> {
    let (x, c) = add_like(q, v, Flags::NSW)?;
    q.int_constant(c).map(|(c, _)| (x, c))
}

/// `m_NUWAddLike(m_Value(X), m_APInt(C))`.
fn nuw_add_like_const(
    q: &Query,
    v: Operand,
) -> Option<(Operand, u128)> {
    let (x, c) = add_like(q, v, Flags::NUW)?;
    q.int_constant(c).map(|(c, _)| (x, c))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9809>
pub fn is_true_predicate(
    q: &Query,
    pred: IntPredicate,
    lhs: Side,
    rhs: Side,
) -> bool {
    if is_true_when_equal(pred) && lhs == rhs {
        return true;
    }
    let (lhs_op, rhs_op) = (side_operand(lhs), side_operand(rhs));
    let is_lhs = |x: Operand| is_side(q, x, lhs);
    let is_rhs = |x: Operand| is_side(q, x, rhs);
    match pred {
        IntPredicate::Sle => {
            if let Some(r) = rhs_op {
                if let Some((_, ins, a, b)) = binary(q, r, BinaryOp::Add)
                    && ins.flags.contains(Flags::NSW)
                    && is_lhs(a)
                    && let Some((c, w)) = q.int_constant(b)
                {
                    return signed(c, w) >= 0;
                }
                if let Some((_, _, a, b)) = binary(q, r, BinaryOp::Or)
                    && is_lhs(a)
                    && let Some((c, w)) = q.int_constant(b)
                {
                    return signed(c, w) >= 0;
                }
                if commutative_intrinsic_with(q, r, "llvm.smax", is_lhs).is_some() {
                    return true;
                }
            }
            if let Some(l) = lhs_op {
                if commutative_intrinsic_with(q, l, "llvm.smin", is_rhs).is_some() {
                    return true;
                }
                if let Some((x, clhs)) = nsw_add_like_const(q, l)
                    && let Some(r) = rhs_op
                    && let Some((rx, crhs)) = nsw_add_like_const(q, r)
                    && Side::of(q, rx) == Side::of(q, x)
                {
                    let w = q.scalar_bits(l);
                    return signed(clhs, w) <= signed(crhs, w);
                }
            }
            false
        }
        IntPredicate::Ule => {
            if let Some(r) = rhs_op {
                if let Some((_, ins, _)) = commutative_with(q, r, BinaryOp::Add, is_lhs)
                    && ins.flags.contains(Flags::NUW)
                {
                    return true;
                }
                if commutative_with(q, r, BinaryOp::Or, is_lhs).is_some() {
                    return true;
                }
                if commutative_intrinsic_with(q, r, "llvm.umax", is_lhs).is_some() {
                    return true;
                }
            }
            if let Some(l) = lhs_op {
                if binary(q, l, BinaryOp::LShr).is_some_and(|(_, _, a, _)| is_rhs(a)) {
                    return true;
                }
                if let Some((_, _, a, b)) = binary(q, l, BinaryOp::UDiv)
                    && is_rhs(a)
                    && q.int_constant(b).is_some_and(|(c, _)| c > 1)
                {
                    return true;
                }
                if commutative_with(q, l, BinaryOp::And, is_rhs).is_some() {
                    return true;
                }
                if commutative_intrinsic_with(q, l, "llvm.umin", is_rhs).is_some() {
                    return true;
                }
                if let Some((x, clhs)) = nuw_add_like_const(q, l)
                    && let Some(r) = rhs_op
                    && let Some((rx, crhs)) = nuw_add_like_const(q, r)
                    && Side::of(q, rx) == Side::of(q, x)
                {
                    return clhs <= crhs;
                }
            }
            false
        }
        _ => false,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9890>
fn is_implied_cond_operands(
    q: &Query,
    pred: CmpPredicate,
    alhs: Side,
    arhs: Side,
    blhs: Side,
    brhs: Side,
) -> Option<bool> {
    let (kind, flip) = match pred.pred {
        IntPredicate::Slt | IntPredicate::Sle => (IntPredicate::Sle, false),
        IntPredicate::Sgt | IntPredicate::Sge => (IntPredicate::Sle, true),
        IntPredicate::Ult | IntPredicate::Ule => (IntPredicate::Ule, false),
        IntPredicate::Ugt | IntPredicate::Uge => (IntPredicate::Ule, true),
        _ => return None,
    };
    let implied = if flip {
        is_true_predicate(q, kind, alhs, blhs) && is_true_predicate(q, kind, brhs, arhs)
    } else {
        is_true_predicate(q, kind, blhs, alhs) && is_true_predicate(q, kind, arhs, brhs)
    };
    implied.then_some(true)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9930>
fn is_implied_cond_common_operand_with_cr(
    lpred: CmpPredicate,
    lcr: &ConstantRange,
    rpred: CmpPredicate,
    rcr: &ConstantRange,
) -> Option<bool> {
    let cr_implies_pred = |cr: ConstantRange, pred: IntPredicate| -> Option<bool> {
        if cr.icmp(pred, rcr) {
            return Some(true);
        }
        if cr.icmp(pred.inverse(), rcr) {
            return Some(false);
        }
        None
    };
    let allowed = |pred: CmpPredicate| ConstantRange::make_allowed_icmp_region_samesign(pred.pred, pred.same_sign, lcr);
    if let Some(res) = cr_implies_pred(allowed(lpred), rpred.pred) {
        return Some(res);
    }
    if lpred.same_sign ^ rpred.same_sign {
        let flip = |pred: CmpPredicate| {
            if pred.same_sign { CmpPredicate::flipped_signedness(pred.pred) } else { pred.pred }
        };
        let (lpred, rpred) = (flip(lpred), flip(rpred));
        return cr_implies_pred(allowed(CmpPredicate::new(lpred)), rpred);
    }
    None
}

/// `isImpliedTrueByMatchingCmp`, behind `ICmpInst::isImpliedByMatchingCmp`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L4031>
fn is_implied_true_by_matching_cmp(
    mut pred1: CmpPredicate,
    mut pred2: CmpPredicate,
) -> bool {
    if CmpPredicate::get_matching(pred1, pred2).is_some() {
        return true;
    }
    if pred1.same_sign && is_signed(pred2.pred) {
        pred1 = CmpPredicate::new(CmpPredicate::flipped_signedness(pred1.pred));
    } else if pred2.same_sign && is_signed(pred1.pred) {
        pred2 = CmpPredicate::new(CmpPredicate::flipped_signedness(pred2.pred));
    }
    let p2 = pred2.pred;
    match pred1.pred {
        IntPredicate::Eq => {
            matches!(
                p2,
                IntPredicate::Uge | IntPredicate::Ule | IntPredicate::Sge | IntPredicate::Sle
            )
        }
        IntPredicate::Ugt => matches!(p2, IntPredicate::Ne | IntPredicate::Uge),
        IntPredicate::Ult => matches!(p2, IntPredicate::Ne | IntPredicate::Ule),
        IntPredicate::Sgt => matches!(p2, IntPredicate::Ne | IntPredicate::Sge),
        IntPredicate::Slt => matches!(p2, IntPredicate::Ne | IntPredicate::Sle),
        _ => false,
    }
}

/// `ICmpInst::isImpliedByMatchingCmp`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L4067>
fn is_implied_by_matching_cmp(
    pred1: CmpPredicate,
    pred2: CmpPredicate,
) -> Option<bool> {
    if is_implied_true_by_matching_cmp(pred1, pred2) {
        return Some(true);
    }
    if is_implied_true_by_matching_cmp(pred1, pred2.inverse()) {
        return Some(false);
    }
    None
}

/// `computeConstantRange(S, ForSigned, SimplifyQuery(DL), MaxDepth - 1)`.
fn constant_range_of(
    q: &Query,
    side: Side,
    for_signed: bool,
) -> ConstantRange {
    match side {
        Side::Int(c, w) => ConstantRange::single(w, c),
        Side::Value(v) => {
            let bare = Query { dt: None, assumptions: None, dom_conditions: None, cond_context: None, cxt: None, ..*q };
            super::range::compute_constant_range(&bare, v, for_signed, MAX_ANALYSIS_RECURSION_DEPTH - 1)
        }
    }
}

/// `CmpInst::isSigned`.
fn is_signed(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

/// `m_PtrToIntOrAddr(m_Specific(x))` matching `a`.
fn is_ptr_to_int_of(
    q: &Query,
    a: Operand,
    x: Side,
) -> bool {
    cast(q, a, CastOp::PtrToInt).is_some_and(|(_, _, op)| is_side(q, op, x))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9962>
#[allow(clippy::too_many_arguments)]
fn is_implied_cond_icmps(
    q: &Query,
    mut lpred: CmpPredicate,
    mut l0: Side,
    mut l1: Side,
    mut rpred: CmpPredicate,
    mut r0: Side,
    mut r1: Side,
    lhs_is_true: bool,
) -> Option<bool> {
    if !lhs_is_true {
        lpred = lpred.inverse();
    }
    if l0 == r1 {
        std::mem::swap(&mut r0, &mut r1);
        rpred = rpred.swapped();
    }
    if r0 == l1 {
        std::mem::swap(&mut l0, &mut l1);
        lpred = lpred.swapped();
    }
    if l1 == r1 {
        let l0_imm = match l0 {
            Side::Int(..) => true,
            Side::Value(v) => imm_constant(q, v),
        };
        if l0 != r0 || l0_imm {
            std::mem::swap(&mut l0, &mut l1);
            lpred = lpred.swapped();
            std::mem::swap(&mut r0, &mut r1);
            rpred = rpred.swapped();
        }
    }
    if l0 == r0 && (l1.apint(q).is_some() || r1.apint(q).is_some()) {
        let lcr = constant_range_of(q, l1, is_signed(lpred.pred));
        let rcr = constant_range_of(q, r1, is_signed(rpred.pred));
        if let Some(r) = is_implied_cond_common_operand_with_cr(lpred, &lcr, rpred, &rcr) {
            return Some(r);
        }
        if l1.apint(q).is_some() && r1.apint(q).is_some() {
            return None;
        }
    }
    if l0 == r0 && l1 == r1 {
        return is_implied_by_matching_cmp(lpred, rpred);
    }
    let signed_lpred = lpred.preferred_signed_predicate();
    let r0_nsw_sub_of_l = side_operand(r0)
        .and_then(|r| binary(q, r, BinaryOp::Sub))
        .is_some_and(|(_, ins, a, b)| ins.flags.contains(Flags::NSW) && is_side(q, a, l0) && is_side(q, b, l1));
    if matches!(signed_lpred, IntPredicate::Sgt | IntPredicate::Sge)
        && r0_nsw_sub_of_l
        && r1.apint(q).is_some_and(|(c, w)| signed(c, w) <= 0)
        && is_implied_by_matching_cmp(CmpPredicate::new(signed_lpred), rpred) == Some(false)
    {
        return Some(false);
    }
    if matches!(signed_lpred, IntPredicate::Slt | IntPredicate::Sle)
        && r0_nsw_sub_of_l
        && r1.apint(q).is_some_and(|(c, w)| signed(c, w) >= 0)
        && is_implied_by_matching_cmp(CmpPredicate::new(signed_lpred), rpred) == Some(true)
    {
        return Some(true);
    }
    if lpred.pred == IntPredicate::Eq
        && matches!(rpred.pred, IntPredicate::Eq | IntPredicate::Ne)
        && l1.apint(q).is_some_and(|(c, _)| c != 0)
        && let Some(l0_op) = side_operand(l0)
        && let Some((_, _, a, b)) = binary(q, l0_op, BinaryOp::Sub)
        && ((is_side(q, a, r0) && is_side(q, b, r1))
            || (is_side(q, a, r1) && is_side(q, b, r0))
            || (is_ptr_to_int_of(q, a, r0) && is_ptr_to_int_of(q, b, r1))
            || (is_ptr_to_int_of(q, a, r1) && is_ptr_to_int_of(q, b, r0)))
    {
        return Some(rpred.pred == IntPredicate::Ne);
    }
    if l0 == r0
        && matches!(lpred.pred, IntPredicate::Ult | IntPredicate::Uge)
        && matches!(rpred.pred, IntPredicate::Ult | IntPredicate::Uge)
        && let Some(l0_op) = side_operand(l0)
        && let Some((_, _, a, b)) = binary(q, l0_op, BinaryOp::Add)
        && ((is_side(q, a, l1) && is_side(q, b, r1)) || (is_side(q, b, l1) && is_side(q, a, r1)))
    {
        return Some(CmpPredicate::get_matching(lpred, rpred).is_some());
    }
    if let Some(p) = CmpPredicate::get_matching(lpred, rpred) {
        return is_implied_cond_operands(q, p, l0, l1, r0, r1);
    }
    if let Some((lc, lw)) = l1.apint(q)
        && let Some((rc, rw)) = r1.apint(q)
        && let Some(r0_op) = side_operand(r0)
        && let Some((_, _, a, b)) = binary(q, r0_op, BinaryOp::And)
        && is_side(q, a, l0)
        && let Some((mask_c, mw)) = q.int_constant(b)
    {
        let masked =
            ConstantRange::make_exact_icmp_region(lpred.pred, lw, lc).binary_and(&ConstantRange::single(mw, mask_c));
        if masked.icmp(rpred.pred, &ConstantRange::single(rw, rc)) {
            return Some(true);
        }
        if masked.icmp(rpred.pred.inverse(), &ConstantRange::single(rw, rc)) {
            return Some(false);
        }
    }
    None
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10151>
fn is_implied_cond_and_or(
    q: &Query,
    lhs: Operand,
    rhs_pred: CmpPredicate,
    rhs_op0: Side,
    rhs_op1: Side,
    lhs_is_true: bool,
    depth: u32,
) -> Option<bool> {
    let (alhs, arhs) = if !lhs_is_true { logical_or(q, lhs) } else { logical_and(q, lhs) }?;
    if let Some(implication) = is_implied_condition_cmp(q, alhs, rhs_pred, rhs_op0, rhs_op1, lhs_is_true, depth + 1) {
        return Some(implication);
    }
    is_implied_condition_cmp(q, arhs, rhs_pred, rhs_op0, rhs_op1, lhs_is_true, depth + 1)
}

/// `isImpliedCondition(LHS, RHSPred, RHSOp0, RHSOp1, DL, LHSIsTrue, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10181>
pub fn is_implied_condition_cmp(
    q: &Query,
    mut lhs: Operand,
    rhs_pred: CmpPredicate,
    rhs_op0: Side,
    rhs_op1: Side,
    mut lhs_is_true: bool,
    depth: u32,
) -> Option<bool> {
    if depth == MAX_ANALYSIS_RECURSION_DEPTH {
        return None;
    }
    if let Some(inner) = not(q, lhs) {
        lhs = inner;
        lhs_is_true = !lhs_is_true;
    }
    let is_int_or_ptr = match rhs_op0 {
        Side::Int(..) => true,
        Side::Value(v) => q.scalar_bits(v) != 0,
    };
    if is_int_or_ptr && let Some((lhs_pred, lhs_op0, lhs_op1)) = icmp_like(q, lhs) {
        return is_implied_cond_icmps(q, lhs_pred, lhs_op0, lhs_op1, rhs_pred, rhs_op0, rhs_op1, lhs_is_true);
    }
    if let Some((_, instruction)) = q.instruction_of(lhs)
        && matches!(
            instruction.opcode,
            Opcode::Binary(BinaryOp::And | BinaryOp::Or) | Opcode::Select
        )
    {
        return is_implied_cond_and_or(q, lhs, rhs_pred, rhs_op0, rhs_op1, lhs_is_true, depth);
    }
    None
}

/// `isImpliedCondition(LHS, RHS, DL, LHSIsTrue, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10230>
pub fn is_implied_condition(
    q: &Query,
    lhs: Operand,
    mut rhs: Operand,
    lhs_is_true: bool,
    depth: u32,
) -> Option<bool> {
    if lhs == rhs {
        return Some(lhs_is_true);
    }
    let mut invert_rhs = false;
    if let Some(inner) = not(q, rhs) {
        rhs = inner;
        if lhs == rhs {
            return Some(!lhs_is_true);
        }
        invert_rhs = true;
    }
    if let Some((rhs_pred, rhs_op0, rhs_op1)) = icmp_like(q, rhs) {
        let implied = is_implied_condition_cmp(q, lhs, rhs_pred, rhs_op0, rhs_op1, lhs_is_true, depth)?;
        return Some(implied != invert_rhs);
    }
    if q.instruction_of(rhs).is_some_and(|(_, ins)| matches!(ins.opcode, Opcode::FCmp(_))) {
        return None;
    }
    if depth == MAX_ANALYSIS_RECURSION_DEPTH {
        return None;
    }
    if let Some((rhs1, rhs2)) = logical_or(q, rhs) {
        if is_implied_condition(q, lhs, rhs1, lhs_is_true, depth + 1) == Some(true) {
            return Some(!invert_rhs);
        }
        if is_implied_condition(q, lhs, rhs2, lhs_is_true, depth + 1) == Some(true) {
            return Some(!invert_rhs);
        }
    }
    if let Some((rhs1, rhs2)) = logical_and(q, rhs) {
        if is_implied_condition(q, lhs, rhs1, lhs_is_true, depth + 1) == Some(false) {
            return Some(invert_rhs);
        }
        if is_implied_condition(q, lhs, rhs2, lhs_is_true, depth + 1) == Some(false) {
            return Some(invert_rhs);
        }
    }
    None
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10293>
fn get_dom_predecessor_condition(
    q: &Query,
    context_i: InstId,
) -> Option<(Operand, bool)> {
    let context_block = q.block_of(context_i)?;
    let pred_block = single_predecessor(q, context_block)?;
    let terminator = q.function.instruction(q.function.terminator(pred_block)?);
    if terminator.opcode != Opcode::Br || terminator.operands.len() != 3 {
        return None;
    }
    let (cond, Operand::Block(true_block), Operand::Block(false_block)) =
        (terminator.operands[0], terminator.operands[1], terminator.operands[2])
    else {
        return None;
    };
    if true_block == false_block {
        return None;
    }
    Some((cond, true_block == context_block))
}

/// `isImpliedByDomCondition(Cond, ContextI, DL)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10322>
pub fn is_implied_by_dom_condition_cond(
    q: &Query,
    cond: Operand,
    cxt: InstId,
) -> Option<bool> {
    let (pred_cond, is_true) = get_dom_predecessor_condition(q, cxt)?;
    is_implied_condition(q, pred_cond, cond, is_true, 0)
}

/// `isImpliedByDomCondition(Pred, LHS, RHS, ContextI, DL)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10332>
pub fn is_implied_by_dom_condition(
    q: &Query,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
    cxt: InstId,
) -> Option<bool> {
    is_implied_by_dom_condition_cmp(q, CmpPredicate::new(pred), lhs, rhs, cxt)
}

/// `isImpliedByDomCondition(Pred, LHS, RHS, ContextI, DL)` for a
/// `CmpPredicate`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10332>
pub fn is_implied_by_dom_condition_cmp(
    q: &Query,
    pred: CmpPredicate,
    lhs: Operand,
    rhs: Operand,
    cxt: InstId,
) -> Option<bool> {
    let (pred_cond, is_true) = get_dom_predecessor_condition(q, cxt)?;
    is_implied_condition_cmp(q, pred_cond, pred, Side::of(q, lhs), Side::of(q, rhs), is_true, 0)
}

fn is_argument(
    q: &Query,
    v: Operand,
) -> bool {
    matches!(
        v,
        Operand::Value(id) if matches!(q.function.value(id).def, ValueDef::Argument(_))
    )
}

/// `SmallPtrSet<Value *, 8>`: the first eight values in a scanned array on the
/// stack, the rest in a set.
#[derive(Default)]
struct SmallVisited {
    small: [Option<Operand>; 8],
    len: usize,
    rest: HashMap<Operand, ()>,
}

impl SmallVisited {
    /// Whether `v` is new.
    fn insert(
        &mut self,
        v: Operand,
    ) -> bool {
        if self.small[..self.len].contains(&Some(v)) {
            return false;
        }
        if self.len < self.small.len() {
            self.small[self.len] = Some(v);
            self.len += 1;
            return true;
        }
        self.rest.insert(v, ()).is_none()
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10875>
fn add_value_affected_by_condition(
    q: &Query,
    v: Operand,
    insert_affected: &mut dyn FnMut(Operand),
) {
    let is_global = matches!(
        v,
        Operand::Constant(id) if matches!(q.context().get(id).kind, ConstantKind::Global(_))
    );
    if is_argument(q, v) || is_global {
        insert_affected(v);
    } else if q.instruction_of(v).is_some() {
        insert_affected(v);
        if let Some((_, _, op)) = cast(q, v, CastOp::PtrToInt).or_else(|| cast(q, v, CastOp::Trunc))
            && (q.instruction_of(op).is_some() || is_argument(q, op))
        {
            insert_affected(op);
        }
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10894>
pub fn find_values_affected_by_condition(
    q: &Query,
    cond: Operand,
    is_assume: bool,
    insert_affected: &mut dyn FnMut(Operand),
) {
    fn add_cmp_operands(
        q: &Query,
        is_assume: bool,
        lhs: Operand,
        rhs: Operand,
        insert_affected: &mut dyn FnMut(Operand),
    ) {
        if is_assume {
            add_value_affected_by_condition(q, lhs, insert_affected);
            add_value_affected_by_condition(q, rhs, insert_affected);
        } else if matches!(rhs, Operand::Constant(_)) {
            add_value_affected_by_condition(q, lhs, insert_affected);
        }
    }
    fn add_nuw_square_operand(
        q: &Query,
        op: Operand,
        insert_affected: &mut dyn FnMut(Operand),
    ) {
        if let Some((_, ins, a, b)) = binary(q, op, BinaryOp::Mul)
            && ins.flags.contains(Flags::NUW)
            && a == b
        {
            add_value_affected_by_condition(q, a, insert_affected);
        }
    }

    let mut worklist = vec![cond];
    let mut visited = SmallVisited::default();
    while let Some(v) = worklist.pop() {
        if !visited.insert(v) {
            continue;
        }
        if is_assume {
            add_value_affected_by_condition(q, v, insert_affected);
            if let Some(x) = not(q, v) {
                add_value_affected_by_condition(q, x, insert_affected);
            }
        }
        if let Some((a, b)) = logical_op(q, v) {
            if !is_assume {
                worklist.push(a);
                worklist.push(b);
            }
        } else if let Some((_, _, pred, a, b)) = icmp(q, v) {
            let pred = pred.pred;
            let has_rhsc = q.int_constant(b).is_some();
            if matches!(pred, IntPredicate::Eq | IntPredicate::Ne) {
                add_value_affected_by_condition(q, a, insert_affected);
                if is_assume {
                    add_value_affected_by_condition(q, b, insert_affected);
                }
                if has_rhsc {
                    let shift = [BinaryOp::Shl, BinaryOp::LShr, BinaryOp::AShr]
                        .into_iter()
                        .find_map(|op| binary(q, a, op))
                        .filter(|(_, _, _, amount)| q.int_constant(*amount).is_some());
                    if let Some((_, _, x, _)) = shift {
                        add_value_affected_by_condition(q, x, insert_affected);
                    } else if let Some((_, _, x, y)) =
                        binary(q, a, BinaryOp::And).or_else(|| binary(q, a, BinaryOp::Or))
                    {
                        add_value_affected_by_condition(q, x, insert_affected);
                        add_value_affected_by_condition(q, y, insert_affected);
                    } else if let Some((_, _, x, y)) = binary(q, a, BinaryOp::Sub) {
                        add_value_affected_by_condition(q, x, insert_affected);
                        add_value_affected_by_condition(q, y, insert_affected);
                    }
                }
            } else {
                add_cmp_operands(q, is_assume, a, b, insert_affected);
                if has_rhsc {
                    if let Some((x, c)) = add_like(q, a, Flags::default())
                        && q.int_constant(c).is_some()
                    {
                        add_value_affected_by_condition(q, x, insert_affected);
                    }
                    if matches!(
                        pred,
                        IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule
                    ) {
                        let nuw_add =
                            binary(q, a, BinaryOp::Add).filter(|(_, ins, _, _)| ins.flags.contains(Flags::NUW));
                        if let Some((_, _, x, y)) =
                            binary(q, a, BinaryOp::And).or_else(|| binary(q, a, BinaryOp::Or)).or(nuw_add)
                        {
                            add_value_affected_by_condition(q, x, insert_affected);
                            add_value_affected_by_condition(q, y, insert_affected);
                        }
                        if let Some((_, ins, x, _)) = binary(q, a, BinaryOp::Sub)
                            && ins.flags.contains(Flags::NUW)
                        {
                            add_value_affected_by_condition(q, x, insert_affected);
                        }
                    }
                }
                if let Some((_, _, x)) = cast(q, a, CastOp::BitCast) {
                    if pred == IntPredicate::Slt && zero(q, b) {
                        insert_affected(x);
                    } else if pred == IntPredicate::Sgt
                        && q.int_constant(b).is_some_and(|(bits, width)| bits == mask(width))
                    {
                        insert_affected(x);
                    }
                }
            }
            add_nuw_square_operand(q, a, insert_affected);
            add_nuw_square_operand(q, b, insert_affected);
            if has_rhsc && let Some((_, args)) = intrinsic_call(q, a, "llvm.ctpop") {
                add_value_affected_by_condition(q, args[0], insert_affected);
            }
        } else if let Some((_, instruction)) = q.instruction_of(v)
            && matches!(instruction.opcode, Opcode::FCmp(_))
        {
            let (mut a, b) = (instruction.operands[0], instruction.operands[1]);
            add_cmp_operands(q, is_assume, a, b, insert_affected);
            if let Some(x) = fneg(q, a) {
                a = x;
                add_value_affected_by_condition(q, a, insert_affected);
            }
            if let Some((_, args)) = intrinsic_call(q, a, "llvm.fabs") {
                add_value_affected_by_condition(q, args[0], insert_affected);
            }
        } else if let Some((_, args)) = intrinsic_call(q, v, "llvm.is.fpclass") {
            add_value_affected_by_condition(q, args[0], insert_affected);
        } else if !is_assume && let Some((_, _, x)) = cast(q, v, CastOp::Trunc) {
            add_value_affected_by_condition(q, x, insert_affected);
        } else if !is_assume && let Some(x) = not(q, v) {
            worklist.push(x);
        }
    }
}
