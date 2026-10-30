//! The integer compare and select arms of `AArch64TargetLowering::
//! LowerOperation`: `SETCC`, `SELECT` and `SELECT_CC` to `SUBS`/`ADDS`/`ANDS`
//! and `CSEL`/`CSINC`/`CSINV`/`CSNEG` on the flags.
//!
//! # How it works
//! [`get_cmp`] is `getAArch64Cmp`: it picks the condition code and the
//! comparison (an immediate moved by one when that makes it encodable, `CMN`
//! for a negated operand, `TST` for a masked one) and returns the flags, the
//! second result of the flag-setting node. The users read them as the last
//! operand of `CSEL`.
//!
//! DEVIATE 1: no `computeKnownBits` `x == 1` to `CBZ` normalization, no `CSET` + `BR_CC` fold,
//! `BRCOND` expands to `BR_CC` here (the shared legalizer has no `ExpandNode` arm for it), no
//! floating-point `emitConjunction` (see `ccmp`), no zero-extending-load `CMN` trick, no overflow-intrinsic folds, no
//! `lookThroughSignExtension` `TST` form, and for floats no vector-compare mask and no `nsz` zero rewrite. Reason: the
//! first compares need none; each is its own slice. Owner arm64-quality, review
//! 2026-11-13, exit B4..B6.
//!
//! # See also
//! `AArch64ISelLowering.cpp` at pin d1106deb7:
//! [`getAArch64Cmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L4476),
//! [`emitComparison`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L4045),
//! [`LowerSETCC`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L12857),
//! [`LowerSELECT_CC`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L13094),
//! [`LowerSELECT`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L13428).

use llrm_codegen::backend::selectiondag::isd::{CondCode, ISD};
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{NodeData, SDValue, SelectionDAG};
use llrm_dagisel::generated::aarch64::ids::isd as a64;

use crate::expand_imm::mov_imm;
use crate::mc::inst::Gpr;

/// `AArch64CC::CondCode`.
pub(super) mod cc {
    pub const EQ: u128 = 0;
    pub const NE: u128 = 1;
    pub const HS: u128 = 2;
    pub const LO: u128 = 3;
    pub const MI: u128 = 4;
    pub const PL: u128 = 5;
    pub const VS: u128 = 6;
    pub const VC: u128 = 7;
    pub const HI: u128 = 8;
    pub const LS: u128 = 9;
    pub const GE: u128 = 10;
    pub const LT: u128 = 11;
    pub const GT: u128 = 12;
    pub const LE: u128 = 13;
    pub const AL: u128 = 14;
}

/// `changeIntCCToAArch64CC`.
pub(super) fn int_cc(
    code: CondCode,
    rhs_is_zero: bool,
) -> u128 {
    match code {
        CondCode::SetNe => cc::NE,
        CondCode::SetEq => cc::EQ,
        CondCode::SetGt => cc::GT,
        CondCode::SetGe if rhs_is_zero => cc::PL,
        CondCode::SetGe => cc::GE,
        CondCode::SetLt if rhs_is_zero => cc::MI,
        CondCode::SetLt => cc::LT,
        CondCode::SetLe => cc::LE,
        CondCode::SetUGt => cc::HI,
        CondCode::SetUGe => cc::HS,
        CondCode::SetULt => cc::LO,
        CondCode::SetULe => cc::LS,
        other => unreachable!("an integer compare is not {other:?}"),
    }
}

/// `changeFPCCToAArch64CC`: the condition, and a second one to OR with it.
fn fp_cc(code: CondCode) -> (u128, Option<u128>) {
    match code {
        CondCode::SetEq | CondCode::SetOEq => (cc::EQ, None),
        CondCode::SetGt | CondCode::SetOGt => (cc::GT, None),
        CondCode::SetGe | CondCode::SetOGe => (cc::GE, None),
        CondCode::SetOLt => (cc::MI, None),
        CondCode::SetOLe => (cc::LS, None),
        CondCode::SetONe => (cc::MI, Some(cc::GT)),
        CondCode::SetO => (cc::VC, None),
        CondCode::SetUo => (cc::VS, None),
        CondCode::SetUEq => (cc::EQ, Some(cc::VS)),
        CondCode::SetUGt => (cc::HI, None),
        CondCode::SetUGe => (cc::PL, None),
        CondCode::SetLt | CondCode::SetULt => (cc::LT, None),
        CondCode::SetLe | CondCode::SetULe => (cc::LE, None),
        CondCode::SetNe | CondCode::SetUNe => (cc::NE, None),
        other => unreachable!("a floating-point compare is not {other:?}"),
    }
}

/// `emitComparison` for floats: `FCMP`, whose result is the flags.
fn emit_fcmp(
    dag: &mut SelectionDAG,
    lhs: SDValue,
    rhs: SDValue,
) -> SDValue {
    dag.get_node(ISD::Target(a64::FCMP), EVT::I32, &[lhs, rhs])
}

fn is_float(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    dag.value_type(value).is_floating_point()
}

/// `LowerSETCC` for floats: one `CSEL` on the inverted condition, or two on the
/// pair that `changeFPCCToAArch64CC` returns.
fn lower_fp_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
    code: CondCode,
) -> SDValue {
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let (one, zero) = (dag.get_constant(1, vt), dag.get_constant(0, vt));
    let flags = emit_fcmp(dag, lhs, rhs);
    match fp_cc(code) {
        (_, None) => {
            let (inverse, _) = fp_cc(code.inverse_float());
            let ccode = cond_const(dag, inverse);
            dag.get_node(ISD::Target(a64::CSEL), vt, &[zero, one, ccode, flags])
        }
        (first, Some(second)) => {
            let first = cond_const(dag, first);
            let one_cc = dag.get_node(ISD::Target(a64::CSEL), vt, &[one, zero, first, flags]);
            let second = cond_const(dag, second);
            dag.get_node(ISD::Target(a64::CSEL), vt, &[one, one_cc, second, flags])
        }
    }
}

/// `LowerSELECT_CC` for floats, without the vector-compare form and the
/// `nsz` rewrite of a zero.
fn lower_fp_select_cc(
    dag: &mut SelectionDAG,
    code: CondCode,
    lhs: SDValue,
    rhs: SDValue,
    tval: SDValue,
    fval: SDValue,
) -> SDValue {
    let flags = emit_fcmp(dag, lhs, rhs);
    let vt = dag.value_type(tval);
    let (first, second) = fp_cc(code);
    let first = cond_const(dag, first);
    let selected = dag.get_node(ISD::Target(a64::CSEL), vt, &[tval, fval, first, flags]);
    match second {
        None => selected,
        Some(second) => {
            let second = cond_const(dag, second);
            dag.get_node(ISD::Target(a64::CSEL), vt, &[tval, selected, second, flags])
        }
    }
}

/// `LowerBR_CC` for floats: a `BRCOND` per condition.
fn lower_fp_br_cc(
    dag: &mut SelectionDAG,
    chain: SDValue,
    code: CondCode,
    lhs: SDValue,
    rhs: SDValue,
    dest: SDValue,
) -> SDValue {
    let flags = emit_fcmp(dag, lhs, rhs);
    let (first, second) = fp_cc(code);
    let first = cond_const(dag, first);
    let branch = dag.get_node(ISD::Target(a64::BRCOND), EVT::OTHER, &[chain, dest, first, flags]);
    match second {
        None => branch,
        Some(second) => {
            let second = cond_const(dag, second);
            dag.get_node(ISD::Target(a64::BRCOND), EVT::OTHER, &[branch, dest, second, flags])
        }
    }
}

pub(super) fn is_equality(code: CondCode) -> bool {
    matches!(code, CondCode::SetEq | CondCode::SetNe)
}

fn is_unsigned(code: CondCode) -> bool {
    matches!(code, CondCode::SetUGt | CondCode::SetUGe | CondCode::SetULt | CondCode::SetULe)
}

/// `AArch64_AM::isLegalArithImmed`.
fn legal_arith_immed(immed: u64) -> bool {
    immed >> 12 == 0 || (immed & 0xfff == 0 && immed >> 24 == 0)
}

/// `AArch64_AM::isLegalCmpImmed`: also as the `ADDS` of its negation.
fn legal_cmp_immed(
    value: i128,
) -> bool {
    legal_arith_immed(value.unsigned_abs().try_into().unwrap_or(u64::MAX))
}

/// `numberOfInstrToLoadImm`.
fn load_cost(value: i128) -> usize {
    mov_imm(value as u64, 32, Gpr(0)).len()
}

/// A constant as a signed number of its type's width.
pub(super) fn signed_constant(
    dag: &SelectionDAG,
    value: SDValue,
) -> Option<i128> {
    let bits = dag.constant(value)?;
    let width = dag.value_type(value).size_in_bits();
    let shift = 128 - width;
    Some(((bits << shift) as i128) >> shift)
}

pub(super) fn cond_of(
    dag: &SelectionDAG,
    value: SDValue,
) -> CondCode {
    match dag.node(value).data {
        NodeData::CondCode(code) => code,
        other => unreachable!("a condition code operand, not {other:?}"),
    }
}

pub(super) fn cond_const(
    dag: &mut SelectionDAG,
    code: u128,
) -> SDValue {
    dag.get_constant(code, EVT::I32)
}

/// `isCMN`: `(sub 0, x)` compared for (in)equality is `ADDS` of `x`.
pub(super) fn is_cmn(
    dag: &SelectionDAG,
    op: SDValue,
    code: CondCode,
) -> bool {
    dag.opcode(op) == ISD::Sub && dag.is_null_constant(dag.operand(op, 0)) && is_equality(code)
}

/// `emitComparison` for integers: the flags.
pub(super) fn emit_comparison(
    dag: &mut SelectionDAG,
    mut lhs: SDValue,
    mut rhs: SDValue,
    code: CondCode,
) -> SDValue {
    let vt = dag.value_type(lhs);
    let mut opcode = a64::SUBS;
    if is_cmn(dag, rhs, code) {
        opcode = a64::ADDS;
        rhs = dag.operand(rhs, 1);
    } else if dag.opcode(lhs) == ISD::Sub && dag.is_null_constant(dag.operand(lhs, 0)) && is_equality(code) {
        opcode = a64::ADDS;
        lhs = dag.operand(lhs, 1);
    } else if dag.is_null_constant(rhs) && !is_unsigned(code) {
        if dag.opcode(lhs) == ISD::And {
            let (x, y) = (dag.operand(lhs, 0), dag.operand(lhs, 1));
            let ands = dag.get_node_multi(ISD::Target(a64::ANDS), &[vt, EVT::I32], &[x, y]);
            dag.replace_all_uses_with(lhs, SDValue { node: ands.node, res: 0 });
            return SDValue { node: ands.node, res: 1 };
        } else if dag.opcode(lhs) == ISD::Target(a64::ANDS) {
            return SDValue { node: lhs.node, res: 1 };
        }
    }
    let node = dag.get_node_multi(ISD::Target(opcode), &[vt, EVT::I32], &[lhs, rhs]);
    SDValue { node: node.node, res: 1 }
}

/// `shouldBeAdjustedToZero`.
fn adjusted_to_zero(
    dag: &SelectionDAG,
    lhs: SDValue,
    c: i128,
    code: &mut CondCode,
) -> bool {
    if dag.opcode(lhs) != ISD::And && dag.opcode(lhs) != ISD::Target(a64::ANDS) {
        return false;
    }
    if c == 1 && matches!(*code, CondCode::SetLt | CondCode::SetGe) {
        *code = if *code == CondCode::SetLt { CondCode::SetLe } else { CondCode::SetGt };
        return true;
    }
    if c == -1 && matches!(*code, CondCode::SetLe | CondCode::SetGt) {
        *code = if *code == CondCode::SetLe { CondCode::SetLt } else { CondCode::SetGe };
        return true;
    }
    false
}

/// `isSupportedExtend`: a sign-extension in register or an `and` with a byte, halfword or word mask.
fn is_supported_extend(
    dag: &SelectionDAG,
    op: SDValue,
) -> bool {
    match dag.opcode(op) {
        ISD::SignExtendInReg => true,
        ISD::And => matches!(dag.constant(dag.operand(op, 1)), Some(0xFF | 0xFFFF | 0xFFFF_FFFF)),
        _ => false,
    }
}

/// `getCmpOperandFoldingProfit`: 2 for a shifted extend, 1 for an extend or a shift the compare
/// folds, 0 for anything else.
fn cmp_operand_folding_profit(
    dag: &SelectionDAG,
    op: SDValue,
    allow_extend: bool,
) -> u32 {
    if !dag.has_one_use(op.node) {
        return 0;
    }
    if allow_extend && is_supported_extend(dag, op) {
        return 1;
    }
    if matches!(dag.opcode(op), ISD::Shl | ISD::Srl | ISD::Sra) {
        if let Some(shift) = dag.constant(dag.operand(op, 1)) {
            if allow_extend && is_supported_extend(dag, dag.operand(op, 0)) {
                return if shift <= 4 { 2 } else { 1 };
            }
            let vt = dag.value_type(op);
            if (vt == EVT::I32 && shift <= 31) || (vt == EVT::I64 && shift <= 63) {
                return 1;
            }
        }
    }
    0
}

/// `getCmpOrCmnOperandFoldingProfit`.
fn cmp_or_cmn_operand_folding_profit(
    dag: &SelectionDAG,
    op: SDValue,
    code: CondCode,
) -> u32 {
    if is_cmn(dag, op, code) {
        return cmp_operand_folding_profit(dag, dag.operand(op, 1), true) + 1;
    }
    cmp_operand_folding_profit(dag, op, true)
}

/// `getAArch64Cmp`: the flags of the comparison and the AArch64 condition code to test.
fn get_cmp(
    dag: &mut SelectionDAG,
    lhs: SDValue,
    mut rhs: SDValue,
    mut code: CondCode,
) -> (SDValue, u128) {
    let mut lhs = lhs;
    if let Some(c) = signed_constant(dag, rhs) {
        let vt = dag.value_type(rhs);
        let bits = vt.size_in_bits();
        let (min, max) = (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1);
        if adjusted_to_zero(dag, lhs, c, &mut code) {
            rhs = dag.get_constant(0, vt);
        } else if !legal_cmp_immed(c) {
            let cost = load_cost(c);
            let better = |other: i128| legal_cmp_immed(other) || cost > load_cost(other);
            let adjusted = match code {
                CondCode::SetLt | CondCode::SetGe if c != min && better(c - 1) => {
                    Some((if code == CondCode::SetLt { CondCode::SetLe } else { CondCode::SetGt }, c - 1))
                }
                CondCode::SetULt | CondCode::SetUGe if better(c - 1) => {
                    Some((if code == CondCode::SetULt { CondCode::SetULe } else { CondCode::SetUGt }, c - 1))
                }
                CondCode::SetLe | CondCode::SetGt if c != max && better(c + 1) => {
                    Some((if code == CondCode::SetLe { CondCode::SetLt } else { CondCode::SetGe }, c + 1))
                }
                CondCode::SetULe | CondCode::SetUGt if c != -1 && better(c + 1) => {
                    Some((if code == CondCode::SetULe { CondCode::SetULt } else { CondCode::SetUGe }, c + 1))
                }
                _ => None,
            };
            if let Some((new_code, new_c)) = adjusted {
                code = new_code;
                rhs = dag.get_constant(new_c as u128 & super_mask(bits), vt);
            }
        } else if c == 0 && cmp_operand_folding_profit(dag, lhs, false) != 0 {
            // Fold into `CMP WZR, reg, shift #amount`.
            std::mem::swap(&mut lhs, &mut rhs);
            code = code.swapped_operands();
        }
    }
    // Comparisons put the simpler operand on the right, and the right can fold a shift or an
    // extend: swap when that is the more profitable side.
    if signed_constant(dag, rhs).map_or(true, |c| !legal_cmp_immed(c))
        && cmp_or_cmn_operand_folding_profit(dag, lhs, code) > cmp_or_cmn_operand_folding_profit(dag, rhs, code)
    {
        std::mem::swap(&mut lhs, &mut rhs);
        code = code.swapped_operands();
    }
    // `getAArch64Cmp`: `(setcc (and|or of compares), 0|1)` as a `CCMP` chain.
    if is_equality(code) {
        if let Some(c @ (0 | 1)) = dag.constant(rhs) {
            let mut out = 0;
            if let Some(flags) = super::ccmp::emit_conjunction(dag, lhs, &mut out) {
                let invert = (code == CondCode::SetNe) ^ (c == 0);
                return (flags, if invert { out ^ 1 } else { out });
            }
        }
    }
    let flags = emit_comparison(dag, lhs, rhs, code);
    (flags, int_cc(code, dag.is_null_constant(rhs)))
}

fn super_mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// `simplifySetCCIntoEq`: `(and x, m) u< 2^k` as `(and x, m & ~(2^k-1)) == 0`.
fn simplify_setcc_into_eq(
    dag: &mut SelectionDAG,
    code: &mut CondCode,
    lhs: &mut SDValue,
    rhs: &mut SDValue,
) {
    if *code != CondCode::SetULt || dag.opcode(*lhs) != ISD::And || !dag.has_one_use(lhs.node) {
        return;
    }
    let (Some(mask), Some(bound)) = (dag.constant(dag.operand(*lhs, 1)), dag.constant(*rhs)) else { return };
    if !bound.is_power_of_two() {
        return;
    }
    let vt = dag.value_type(*lhs);
    let narrowed = dag.get_constant(mask & !(bound - 1), vt);
    let x = dag.operand(*lhs, 0);
    *lhs = dag.get_node(ISD::And, vt, &[x, narrowed]);
    *rhs = dag.get_constant(0, dag.value_type(*rhs));
    *code = CondCode::SetEq;
}

/// `LowerSETCC` for integers.
pub fn lower_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (mut lhs, mut rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let mut code = cond_of(dag, dag.operand(node, 2));
    if is_float(dag, lhs) {
        return Some(lower_fp_setcc(dag, node, code));
    }
    if !dag.value_type(lhs).is_integer() {
        return None;
    }
    // `visitBRCOND` has made a `BR_CC` of a compare only a branch reads, before this runs; here
    // `lower_brcond` reads the `SETCC` itself, and a flags-setting `and` would not be undone.
    let users: Vec<u32> = dag.users(node).collect();
    let only_branched_on = users.iter().all(|&user| {
        let user = SDValue { node: user, res: 0 };
        dag.opcode(user) == ISD::BrCond && dag.operand(user, 1) == node
    });
    if !users.is_empty() && only_branched_on {
        return Some(node);
    }
    let vt = dag.value_type(node);
    let (one, zero) = (dag.get_constant(1, vt), dag.get_constant(0, vt));
    simplify_setcc_into_eq(dag, &mut code, &mut lhs, &mut rhs);
    let (flags, ccode) = get_cmp(dag, lhs, rhs, code.inverse_integer());
    let ccode = cond_const(dag, ccode);
    // The condition is inverted, so the operands are reversed: one `CSINC`.
    Some(dag.get_node(ISD::Target(a64::CSEL), vt, &[zero, one, ccode, flags]))
}

/// `LowerSELECT`: a `SETCC` condition is a `SELECT_CC`; any other is tested `!= 0`.
pub fn lower_select(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (cond, tval, fval) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let result = dag.value_type(node);
    if !result.is_integer() && !result.is_floating_point() {
        return None;
    }
    let (lhs, rhs, code) = if dag.opcode(cond) == ISD::SetCC {
        (dag.operand(cond, 0), dag.operand(cond, 1), cond_of(dag, dag.operand(cond, 2)))
    } else {
        (cond, dag.get_constant(0, dag.value_type(cond)), CondCode::SetNe)
    };
    lower_select_cc(dag, code, lhs, rhs, tval, fval)
}

/// `LowerSELECT_CC` node form.
pub fn lower_select_cc_node(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let ops: Vec<SDValue> = (0..5).map(|i| dag.operand(node, i)).collect();
    let code = cond_of(dag, ops[4]);
    lower_select_cc(dag, code, ops[0], ops[1], ops[2], ops[3])
}

/// `LowerSELECT_CC` for integers.
fn lower_select_cc(
    dag: &mut SelectionDAG,
    mut code: CondCode,
    lhs: SDValue,
    rhs: SDValue,
    mut tval: SDValue,
    mut fval: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(lhs);
    if vt.is_floating_point() {
        return Some(lower_fp_select_cc(dag, code, lhs, rhs, tval, fval));
    }
    if !vt.is_integer() {
        return None;
    }
    let constant = |dag: &SelectionDAG, v: SDValue| signed_constant(dag, v);
    let (mut ctval, mut cfval) = (constant(dag, tval), constant(dag, fval));
    let rhsc = constant(dag, rhs);

    // `smax(x, 0)` and `smin(x, 0)`: a shift and a mask, not a compare and a select.
    if matches!(code, CondCode::SetGt | CondCode::SetLt) && lhs == tval && rhsc == Some(0) && cfval == Some(0) {
        let bits = vt.size_in_bits();
        let by = dag.get_constant(u128::from(bits - 1), vt);
        let mut shift = dag.get_node(ISD::Sra, vt, &[lhs, by]);
        if code == CondCode::SetGt {
            shift = dag.get_not(shift, vt);
        }
        return Some(dag.get_node(ISD::And, vt, &[lhs, shift]));
    }

    // Absolute difference: `select_cc l, r, l-r, r-l` is `l-r` and its negation (`subs` + `cneg`).
    let is_sub = |dag: &SelectionDAG, v: SDValue| dag.opcode(v) == ISD::Sub;
    if is_sub(dag, tval) && is_sub(dag, fval) {
        let (t0, t1, f0, f1) =
            (dag.operand(tval, 0), dag.operand(tval, 1), dag.operand(fval, 0), dag.operand(fval, 1));
        if t0 == lhs && t1 == rhs && f0 == rhs && f1 == lhs {
            fval = dag.get_negative(tval, dag.value_type(tval));
        } else if t0 == rhs && t1 == lhs && f0 == lhs && f1 == rhs {
            tval = dag.get_negative(fval, dag.value_type(fval));
        }
        (ctval, cfval) = (constant(dag, tval), constant(dag, fval));
    }

    let mut opcode = a64::CSEL;
    let swap = |dag: &SelectionDAG,
                code: &mut CondCode,
                tval: &mut SDValue,
                fval: &mut SDValue,
                ct: &mut Option<i128>,
                cf: &mut Option<i128>| {
        std::mem::swap(tval, fval);
        std::mem::swap(ct, cf);
        *code = code.inverse_integer();
        let _ = dag;
    };
    let tvt = dag.value_type(tval);
    if let (Some(t), Some(f)) = (ctval, cfval) {
        let t_all_ones = t == -1;
        if (t_all_ones && f == 0) || (t == 1 && f == 0) {
            swap(dag, &mut code, &mut tval, &mut fval, &mut ctval, &mut cfval);
        } else if dag.opcode(tval) == ISD::Xor || dag.opcode(tval) == ISD::Sub {
            // handled below, as these are not constants
        } else {
            let mut do_swap = false;
            if t == !f {
                opcode = a64::CSINV;
            } else if f > i128::from(i64::MIN) && t == -f {
                opcode = a64::CSNEG;
            } else if tvt == EVT::I32 {
                let (t32, f32_) = (t as u32, f as u32);
                if t32 == f32_.wrapping_add(1) || t32.wrapping_add(1) == f32_ {
                    opcode = a64::CSINC;
                    do_swap = t32 > f32_;
                }
            } else {
                let (t64, f64_) = (t as u64, f as u64);
                if t64 == f64_.wrapping_add(1) || t64.wrapping_add(1) == f64_ {
                    opcode = a64::CSINC;
                    do_swap = t > f;
                }
            }
            if do_swap {
                swap(dag, &mut code, &mut tval, &mut fval, &mut ctval, &mut cfval);
            }
            if opcode != a64::CSEL {
                // FVal is TVal inverted, negated or incremented.
                fval = tval;
            }
        }
    } else if dag.opcode(tval) == ISD::Xor && dag.constant(dag.operand(tval, 1)).is_some_and(|c| c == super_mask(tvt.size_in_bits())) {
        // A NOT matches CSINV rather than CSEL.
        swap(dag, &mut code, &mut tval, &mut fval, &mut ctval, &mut cfval);
    } else if dag.opcode(tval) == ISD::Sub && dag.is_null_constant(dag.operand(tval, 0)) {
        // A negation matches CSNEG.
        swap(dag, &mut code, &mut tval, &mut fval, &mut ctval, &mut cfval);
    }

    // Reuse the compared register for the constant instead of materializing it.
    if opcode == a64::CSEL && rhsc.is_some_and(|r| r != 1 && r != 0 && r != -1) {
        let acc = int_cc(code, false);
        if ctval == rhsc && acc == cc::EQ {
            tval = lhs;
        } else if cfval == rhsc && acc == cc::NE {
            fval = lhs;
        }
    } else if opcode == a64::CSNEG && rhsc == Some(1) {
        let acc = int_cc(code, false);
        if ctval == rhsc && acc == cc::EQ {
            opcode = a64::CSINV;
            tval = lhs;
            fval = dag.get_constant(0, dag.value_type(fval));
        }
    }

    let (flags, ccode) = get_cmp(dag, lhs, rhs, code);
    let ccode = cond_const(dag, ccode);
    let out = dag.value_type(tval);
    Some(dag.get_node(ISD::Target(opcode), out, &[tval, fval, ccode, flags]))
}

/// `lookThroughSignExtension`: the value before a sign extension and the
/// position of the sign bit it extends from.
fn look_through_sign_extension(
    dag: &SelectionDAG,
    value: SDValue,
) -> (SDValue, u128) {
    match dag.opcode(value) {
        ISD::SignExtendInReg => {
            let from = match dag.node(dag.operand(value, 1)).data {
                NodeData::ValueType(vt) => vt.size_in_bits(),
                other => unreachable!("a value type operand, not {other:?}"),
            };
            (dag.operand(value, 0), u128::from(from - 1))
        }
        ISD::SignExtend => {
            let inner = dag.operand(value, 0);
            (inner, u128::from(dag.value_type(inner).size_in_bits() - 1))
        }
        _ => (value, u128::from(dag.value_type(value).size_in_bits() - 1)),
    }
}

/// `optimizeBitTest`: `(and x, 2^k) ==/!= 0` is a `TBZ`/`TBNZ` of bit `k`.
fn optimize_bit_test(
    dag: &mut SelectionDAG,
    op: SDValue,
    chain: SDValue,
    dest: SDValue,
    opcode: u16,
) -> Option<SDValue> {
    if dag.opcode(op) != ISD::And {
        return None;
    }
    if let Some(mask) = dag.constant(dag.operand(op, 1)).filter(|m| m.is_power_of_two()) {
        let test = dag.operand(op, 0);
        let bit = dag.get_constant(u128::from(mask.trailing_zeros()), EVT::I64);
        return Some(dag.get_node(ISD::Target(opcode), EVT::OTHER, &[chain, test, bit, dest]));
    }
    let shift = dag.operand(op, 0);
    if dag.opcode(shift) == ISD::Shl && dag.constant(dag.operand(shift, 0)) == Some(1) {
        let one = dag.operand(shift, 0);
        let (by, value) = (dag.operand(shift, 1), dag.operand(op, 1));
        let shifted = dag.get_node(ISD::Srl, dag.value_type(one), &[value, by]);
        let zero = dag.get_constant(0, EVT::I64);
        return Some(dag.get_node(ISD::Target(opcode), EVT::OTHER, &[chain, shifted, zero, dest]));
    }
    None
}

/// `LowerBRCOND` has only the `CCMP` chain; the rest is `ExpandNode`'s `BR_CC`.
pub fn lower_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (chain, cond, dest) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    // `LowerBRCOND`: a tree of compares is a `CCMP` chain and a `B.cc`.
    let mut out = 0;
    let tree = dag.opcode(cond) != ISD::SetCC;
    if let Some(flags) = tree.then(|| super::ccmp::emit_conjunction(dag, cond, &mut out)).flatten() {
        let ccode = cond_const(dag, out);
        return Some(dag.get_node(ISD::Target(a64::BRCOND), EVT::OTHER, &[chain, dest, ccode, flags]));
    }
    if dag.opcode(cond) == ISD::SetCC && dag.value_type(dag.operand(cond, 0)).is_integer() {
        let (lhs, rhs, code) = (dag.operand(cond, 0), dag.operand(cond, 1), cond_of(dag, dag.operand(cond, 2)));
        return lower_br_cc(dag, chain, code, lhs, rhs, dest);
    }
    if !dag.value_type(cond).is_integer() {
        return None;
    }
    // We test only the i1 bit; the `and` is skipped when it is there already.
    let vt = dag.value_type(cond);
    let one = dag.get_constant(1, vt);
    // The builder inverts a branch to the next block with `xor c, 1`; the pin inverts the
    // condition code instead, so the bit is tested for zero.
    let bit = |dag: &SelectionDAG, value: SDValue| {
        (dag.opcode(value) == ISD::And && dag.constant(dag.operand(value, 1)) == Some(1)).then(|| dag.operand(value, 0))
    };
    let inner = bit(dag, cond).unwrap_or(cond);
    let (tested, code) = if dag.opcode(inner) == ISD::Xor && matches!(signed_constant(dag, dag.operand(inner, 1)), Some(1 | -1)) {
        (dag.operand(inner, 0), CondCode::SetEq)
    } else {
        (cond, CondCode::SetNe)
    };
    // `br_cc setne (and|or of compares), 1` (the pin's inversion by `setne -1`): a `CCMP` chain.
    let tree = bit(dag, tested).unwrap_or(tested);
    if matches!(dag.opcode(tree), ISD::And | ISD::Or) {
        let mut out = 0;
        if let Some(flags) = super::ccmp::emit_conjunction(dag, tree, &mut out) {
            let ccode = cond_const(dag, if code == CondCode::SetEq { out ^ 1 } else { out });
            return Some(dag.get_node(ISD::Target(a64::BRCOND), EVT::OTHER, &[chain, dest, ccode, flags]));
        }
    }
    let masked = match bit(dag, tested) {
        Some(_) => tested,
        None => dag.get_node(ISD::And, vt, &[tested, one]),
    };
    let zero = dag.get_constant(0, vt);
    lower_br_cc(dag, chain, code, masked, zero, dest)
}

/// `LowerBR_CC` node form.
pub fn lower_br_cc_node(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let ops: Vec<SDValue> = (0..5).map(|i| dag.operand(node, i)).collect();
    let code = cond_of(dag, ops[1]);
    lower_br_cc(dag, ops[0], code, ops[2], ops[3], ops[4])
}

/// `LowerBR_CC` for integers: `CBZ`/`CBNZ`, `TBZ`/`TBNZ`, else flags and `BRCOND`.
fn lower_br_cc(
    dag: &mut SelectionDAG,
    chain: SDValue,
    code: CondCode,
    lhs: SDValue,
    rhs: SDValue,
    dest: SDValue,
) -> Option<SDValue> {
    if is_float(dag, lhs) {
        return Some(lower_fp_br_cc(dag, chain, code, lhs, rhs, dest));
    }
    if !dag.value_type(lhs).is_integer() {
        return None;
    }
    let rhsc = signed_constant(dag, rhs);
    let branch = |dag: &mut SelectionDAG, opcode: u16, ops: &[SDValue]| {
        Some(dag.get_node(ISD::Target(opcode), EVT::OTHER, ops))
    };
    if rhsc == Some(0) {
        if code == CondCode::SetEq {
            return optimize_bit_test(dag, lhs, chain, dest, a64::TBZ)
                .or_else(|| branch(dag, a64::CBZ, &[chain, lhs, dest]));
        } else if code == CondCode::SetNe {
            return optimize_bit_test(dag, lhs, chain, dest, a64::TBNZ)
                .or_else(|| branch(dag, a64::CBNZ, &[chain, lhs, dest]));
        } else if code == CondCode::SetLt && dag.opcode(lhs) != ISD::And {
            let (test, bit) = look_through_sign_extension(dag, lhs);
            let bit = dag.get_constant(bit, EVT::I64);
            return branch(dag, a64::TBNZ, &[chain, test, bit, dest]);
        }
    }
    if rhsc == Some(-1) && code == CondCode::SetGt && dag.opcode(lhs) != ISD::And {
        let (test, bit) = look_through_sign_extension(dag, lhs);
        let bit = dag.get_constant(bit, EVT::I64);
        return branch(dag, a64::TBZ, &[chain, test, bit, dest]);
    }
    let (flags, ccode) = get_cmp(dag, lhs, rhs, code);
    let ccode = cond_const(dag, ccode);
    branch(dag, a64::BRCOND, &[chain, dest, ccode, flags])
}

/// `LowerXOR`: the node itself, but `xor x, (select_cc a, b, cc, 0, -1)` is a `CSINV`.
/// DEVIATE 1 also leaves out the overflow-intrinsic form.
pub fn lower_xor(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (mut sel, mut other) = (dag.operand(node, 0), dag.operand(node, 1));
    if dag.opcode(sel) != ISD::SelectCC {
        std::mem::swap(&mut sel, &mut other);
    }
    if dag.opcode(sel) != ISD::SelectCC {
        return Some(node);
    }
    let ops: Vec<SDValue> = (0..5).map(|i| dag.operand(sel, i)).collect();
    let (lhs, rhs, mut tval, mut fval) = (ops[0], ops[1], ops[2], ops[3]);
    let mut code = cond_of(dag, ops[4]);
    if dag.value_type(lhs) != EVT::I32 && dag.value_type(lhs) != EVT::I64 {
        return Some(node);
    }
    let (Some(mut ct), Some(mut cf)) = (signed_constant(dag, tval), signed_constant(dag, fval)) else {
        return Some(node);
    };
    if ct == -1 && cf == 0 {
        std::mem::swap(&mut tval, &mut fval);
        std::mem::swap(&mut ct, &mut cf);
        code = code.inverse_integer();
    }
    if ct != 0 || cf != -1 {
        return Some(node);
    }
    let (flags, ccode) = get_cmp(dag, lhs, rhs, code);
    let ccode = cond_const(dag, ccode);
    let vt = dag.value_type(other);
    let inverted = dag.get_not(other, vt);
    Some(dag.get_node(ISD::Target(a64::CSEL), dag.value_type(sel), &[other, inverted, ccode, flags]))
}

fn is_double_or_single(vt: EVT) -> bool {
    vt.is_floating_point() && matches!(vt.size_in_bits(), 32 | 64)
}

/// `LowerINT_TO_FP`: a scalar `f32`/`f64` from an `i32`/`i64` is legal as it is.
pub fn lower_int_to_fp(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (to, from) = (dag.value_type(node), dag.value_type(dag.operand(node, 0)));
    (is_double_or_single(to) && matches!(from.size_in_bits(), 32 | 64)).then_some(node)
}

/// `LowerFP_TO_INT`: a scalar `f32`/`f64` to an `i32`/`i64` is legal as it is.
pub fn lower_fp_to_int(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (to, from) = (dag.value_type(node), dag.value_type(dag.operand(node, 0)));
    (is_double_or_single(from) && matches!(to.size_in_bits(), 32 | 64)).then_some(node)
}

/// `LowerFP_EXTEND`: `f32` to `f64` is legal as it is.
pub fn lower_fp_extend(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (to, from) = (dag.value_type(node), dag.value_type(dag.operand(node, 0)));
    (to.size_in_bits() == 64 && from.size_in_bits() == 32 && to.is_floating_point()).then_some(node)
}

/// `LowerFP_ROUND`: `f64` to `f32` is legal as it is.
pub fn lower_fp_round(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (to, from) = (dag.value_type(node), dag.value_type(dag.operand(node, 0)));
    (to.size_in_bits() == 32 && from.size_in_bits() == 64 && to.is_floating_point()).then_some(node)
}

/// `isFPImmLegal`: a constant an `fmov` immediate holds, positive zero, or one that
/// at most two `mov`s make (the pin's limit when not optimizing for size).
pub fn fp_imm_legal(
    vt: llrm_codegen::backend::selectiondag::mvt::MVT,
    bits: u128,
) -> bool {
    use llrm_codegen::backend::selectiondag::mvt::MVT;
    let (value, width) = match vt {
        MVT::F64 => (f64::from_bits(bits as u64), 64),
        MVT::F32 => (f64::from(f32::from_bits(bits as u32)), 32),
        _ => return false,
    };
    bits == 0 || crate::mc::immediate::fp_immediate(value).is_some() || mov_imm(bits as u64, width, Gpr(0)).len() <= 2
}
