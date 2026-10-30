//! `emitConjunction`: a tree of integer compares joined by `and`/`or` as one
//! compare and a chain of conditional compares (`CCMP`/`CCMN`).
//!
//! # How it works
//! `can_emit_conjunction` asks whether a tree has the shape and says which
//! subtrees can be negated just by changing their conditions, which must be
//! emitted first and which are better first. `emit_conjunction_rec` then emits
//! the compares right to left: each `CCMP` tests the previous condition and,
//! if it fails, sets the flags so the final condition reads false. The caller
//! branches or selects on the returned condition.
//!
//! DEVIATE: a floating-point compare is not a leaf (`FCCMP`, and the
//! two-condition compares, are not translated) so the tree is refused.
//! Exit: floating-point conditional compares.
//!
//! # See also
//! `AArch64ISelLowering.cpp` at pin d1106deb7:
//! [`emitConjunction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L4398),
//! [`emitConjunctionRec`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L4280),
//! [`canEmitConjunction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L4207),
//! [`emitConditionalComparison`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L4149).

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{SDValue, SelectionDAG};
use llrm_dagisel::generated::aarch64::ids::isd as a64;

use super::lowering::{cc, cond_const, cond_of, emit_comparison, int_cc, is_cmn, is_equality, signed_constant};

/// What `canEmitConjunction` reports about a subtree.
#[derive(Clone, Copy)]
struct Shape {
    /// The whole subtree can be negated by changing its compares' conditions.
    can_negate: bool,
    /// It needs negating and cannot be, so it must be emitted first.
    must_be_first: bool,
    /// Emitting it first may give better code.
    prefer_first: bool,
}

/// `AArch64CC::getInvertedCondCode`.
fn invert(code: u128) -> u128 {
    code ^ 1
}

/// `AArch64CC::getNZCVToSatisfyCondCode`: flags (`NZCV`, as `ccmp` takes them) that satisfy `code`.
fn nzcv_to_satisfy(code: u128) -> u128 {
    const N: u128 = 8;
    const Z: u128 = 4;
    const C: u128 = 2;
    const V: u128 = 1;
    match code {
        cc::EQ => Z,
        cc::HS | cc::HI => C,
        cc::MI | cc::LT => N,
        cc::VS => V,
        cc::LE => Z,
        _ => 0,
    }
}

/// `canEmitConjunction`.
fn can_emit_conjunction(
    dag: &SelectionDAG,
    val: SDValue,
    will_negate: bool,
    depth: u32,
) -> Option<Shape> {
    if !dag.has_one_use(val.node) {
        return None;
    }
    let opcode = dag.opcode(val);
    if opcode == ISD::SetCC {
        let (lhs, rhs) = (dag.operand(val, 0), dag.operand(val, 1));
        let vt = dag.value_type(lhs);
        if !vt.is_integer() {
            return None;
        }
        // A compare whose subtraction already exists is better emitted first.
        let prefer_first = dag.get_node_if_exists(ISD::Sub, &[vt], &[lhs, rhs]).is_some();
        return Some(Shape { can_negate: true, must_be_first: false, prefer_first });
    }
    // Protect against exponential runtime and stack overflow.
    if depth > 6 {
        return None;
    }
    if opcode != ISD::And && opcode != ISD::Or {
        return None;
    }
    let is_or = opcode == ISD::Or;
    let left = can_emit_conjunction(dag, dag.operand(val, 0), is_or, depth + 1)?;
    let right = can_emit_conjunction(dag, dag.operand(val, 1), is_or, depth + 1)?;
    if left.must_be_first && right.must_be_first {
        return None;
    }
    let (can_negate, must_be_first);
    if is_or {
        // An OR needs one side that negates naturally.
        if !left.can_negate && !right.can_negate {
            return None;
        }
        // A negated OR of negatable leaves negates as a whole.
        can_negate = will_negate && left.can_negate && right.can_negate;
        must_be_first = !can_negate;
    } else {
        // An AND cannot be negated naturally.
        can_negate = false;
        must_be_first = left.must_be_first || right.must_be_first;
    }
    Some(Shape { can_negate, must_be_first, prefer_first: left.prefer_first || right.prefer_first })
}

/// `emitConditionalComparison`, integers: `CCMP`, or `CCMN` where the operand is a small negative constant or a negation.
fn emit_conditional_comparison(
    dag: &mut SelectionDAG,
    mut lhs: SDValue,
    mut rhs: SDValue,
    code: llrm_codegen::backend::selectiondag::isd::CondCode,
    cc_op: SDValue,
    predicate: u128,
    out_cc: u128,
) -> SDValue {
    let mut opcode = a64::CCMP;
    if let Some(imm) = signed_constant(dag, rhs) {
        if imm < 0 && imm > -32 {
            opcode = a64::CCMN;
            let vt = dag.value_type(rhs);
            rhs = dag.get_constant(imm.unsigned_abs(), vt);
        }
    } else if is_cmn(dag, rhs, code) {
        opcode = a64::CCMN;
        rhs = dag.operand(rhs, 1);
    } else if dag.opcode(lhs) == ISD::Sub && dag.is_null_constant(dag.operand(lhs, 0)) && is_equality(code) {
        opcode = a64::CCMN;
        lhs = dag.operand(lhs, 1);
    }
    let condition = cond_const(dag, predicate);
    let nzcv = nzcv_to_satisfy(invert(out_cc));
    let nzcv_op = dag.get_constant(nzcv, EVT::I32);
    dag.get_node(ISD::Target(opcode), EVT::I32, &[lhs, rhs, nzcv_op, condition, cc_op])
}

/// `emitConjunctionRec`: the flags of `val`, and in `out_cc` the condition to test.
fn emit_conjunction_rec(
    dag: &mut SelectionDAG,
    val: SDValue,
    out_cc: &mut u128,
    negate: bool,
    cc_op: Option<SDValue>,
    predicate: u128,
) -> SDValue {
    let opcode = dag.opcode(val);
    // A leaf: a compare, or a conditional compare after the one before it.
    if opcode == ISD::SetCC {
        let (lhs, rhs) = (dag.operand(val, 0), dag.operand(val, 1));
        let mut code = cond_of(dag, dag.operand(val, 2));
        if negate {
            code = code.inverse_integer();
        }
        *out_cc = int_cc(code, dag.is_null_constant(rhs));
        return match cc_op {
            None => emit_comparison(dag, lhs, rhs, code),
            Some(previous) => emit_conditional_comparison(dag, lhs, rhs, code, previous, predicate, *out_cc),
        };
    }
    let is_or = opcode == ISD::Or;
    let (mut lhs, mut rhs) = (dag.operand(val, 0), dag.operand(val, 1));
    let mut left = can_emit_conjunction(dag, lhs, is_or, 0).expect("a valid conjunction tree");
    let mut right = can_emit_conjunction(dag, rhs, is_or, 0).expect("a valid conjunction tree");

    let should_first_left = left.prefer_first && !right.prefer_first && !right.must_be_first;
    // Swap the subtree that must or should come first to the right.
    if left.must_be_first || should_first_left {
        std::mem::swap(&mut lhs, &mut rhs);
        std::mem::swap(&mut left, &mut right);
    }

    let (negate_right, negate_after_right, negate_left, negate_after_all);
    if is_or {
        // Swap the subtree that negates naturally to the left.
        if !left.can_negate {
            std::mem::swap(&mut lhs, &mut rhs);
            negate_right = false;
            negate_after_right = true;
        } else {
            // Negate the left subtree if possible, otherwise the result.
            negate_right = right.can_negate;
            negate_after_right = !right.can_negate;
        }
        negate_left = true;
        negate_after_all = !negate;
    } else {
        negate_left = false;
        negate_right = false;
        negate_after_right = false;
        negate_after_all = false;
    }

    let mut right_cc = 0;
    let right_flags = emit_conjunction_rec(dag, rhs, &mut right_cc, negate_right, cc_op, predicate);
    if negate_after_right {
        right_cc = invert(right_cc);
    }
    let left_flags = emit_conjunction_rec(dag, lhs, out_cc, negate_left, Some(right_flags), right_cc);
    if negate_after_all {
        *out_cc = invert(*out_cc);
    }
    left_flags
}

/// `emitConjunction`: the flags for `val`, and the condition to test in
/// `out_cc`, or `None` where the tree has not the shape.
pub fn emit_conjunction(
    dag: &mut SelectionDAG,
    val: SDValue,
    out_cc: &mut u128,
) -> Option<SDValue> {
    can_emit_conjunction(dag, val, false, 0)?;
    Some(emit_conjunction_rec(dag, val, out_cc, false, None, cc::AL))
}

/// `performANDORCSELCombine`: `and`/`or` of two `cset`s, the flags of a `SUBS` each, is one
/// `CCMP` after the first compare and a `cset` of its flags.
pub fn combine_and_or_csel(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (csel0, csel1) = (dag.operand(node, 0), dag.operand(node, 1));
    if dag.opcode(csel0) != ISD::Target(a64::CSEL) || dag.opcode(csel1) != ISD::Target(a64::CSEL) {
        return None;
    }
    if !dag.has_one_use(csel0.node) || !dag.has_one_use(csel1.node) {
        return None;
    }
    let is_set = |dag: &SelectionDAG, csel: SDValue| {
        dag.is_null_constant(dag.operand(csel, 0)) && dag.constant(dag.operand(csel, 1)) == Some(1)
    };
    if !is_set(dag, csel0) || !is_set(dag, csel1) {
        return None;
    }
    let (mut cmp0, mut cmp1) = (dag.operand(csel0, 3), dag.operand(csel1, 3));
    let code = |dag: &SelectionDAG, csel: SDValue| dag.constant(dag.operand(csel, 2)).expect("a condition code");
    let (mut cc0, mut cc1) = (code(dag, csel0), code(dag, csel1));
    if !dag.has_one_use(cmp0.node) || !dag.has_one_use(cmp1.node) {
        return None;
    }
    let subs = ISD::Target(a64::SUBS);
    if dag.opcode(cmp1) != subs && dag.opcode(cmp0) == subs {
        std::mem::swap(&mut cmp0, &mut cmp1);
        std::mem::swap(&mut cc0, &mut cc1);
    }
    if dag.opcode(cmp1) != subs {
        return None;
    }
    let (condition, nzcv) = if dag.opcode(node) == ISD::And {
        (invert(cc0), nzcv_to_satisfy(cc1))
    } else {
        (cc0, nzcv_to_satisfy(invert(cc1)))
    };
    let condition = cond_const(dag, condition);
    let nzcv_op = dag.get_constant(nzcv, EVT::I32);
    let (lhs, mut rhs) = (dag.operand(cmp1, 0), dag.operand(cmp1, 1));
    let mut opcode = a64::CCMP;
    if let Some(imm) = signed_constant(dag, rhs).filter(|&imm| imm < 0 && imm > -32) {
        opcode = a64::CCMN;
        let vt = dag.value_type(rhs);
        rhs = dag.get_constant(imm.unsigned_abs(), vt);
    }
    let ccmp = dag.get_node(ISD::Target(opcode), EVT::I32, &[lhs, rhs, nzcv_op, condition, cmp0]);
    let (vt, cc1) = (dag.value_type(node), cond_const(dag, cc1));
    let (zero, one) = (dag.operand(csel0, 0), dag.operand(csel0, 1));
    Some(dag.get_node(ISD::Target(a64::CSEL), vt, &[zero, one, cc1, ccmp]))
}
