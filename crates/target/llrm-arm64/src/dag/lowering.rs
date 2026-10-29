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
//! `emitConjunction` (`CCMP` chains), no `getCmpOperandFoldingProfit`
//! swaps, no zero-extending-load `CMN` trick, no overflow-intrinsic folds, no
//! `lookThroughSignExtension` `TST` form, no floating-point compare. Reason: the
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
mod cc {
    pub const EQ: u128 = 0;
    pub const NE: u128 = 1;
    pub const HS: u128 = 2;
    pub const LO: u128 = 3;
    pub const MI: u128 = 4;
    pub const PL: u128 = 5;
    pub const HI: u128 = 8;
    pub const LS: u128 = 9;
    pub const GE: u128 = 10;
    pub const LT: u128 = 11;
    pub const GT: u128 = 12;
    pub const LE: u128 = 13;
}

/// `changeIntCCToAArch64CC`.
fn int_cc(
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

fn is_equality(code: CondCode) -> bool {
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
fn signed_constant(
    dag: &SelectionDAG,
    value: SDValue,
) -> Option<i128> {
    let bits = dag.constant(value)?;
    let width = dag.value_type(value).size_in_bits();
    let shift = 128 - width;
    Some(((bits << shift) as i128) >> shift)
}

fn cond_of(
    dag: &SelectionDAG,
    value: SDValue,
) -> CondCode {
    match dag.node(value).data {
        NodeData::CondCode(code) => code,
        other => unreachable!("a condition code operand, not {other:?}"),
    }
}

fn cond_const(
    dag: &mut SelectionDAG,
    code: u128,
) -> SDValue {
    dag.get_constant(code, EVT::I32)
}

/// `isCMN`: `(sub 0, x)` compared for (in)equality is `ADDS` of `x`.
fn is_cmn(
    dag: &SelectionDAG,
    op: SDValue,
    code: CondCode,
) -> bool {
    dag.opcode(op) == ISD::Sub && dag.is_null_constant(dag.operand(op, 0)) && is_equality(code)
}

/// `emitComparison` for integers: the flags.
fn emit_comparison(
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

/// `getAArch64Cmp`: the flags of the comparison and the AArch64 condition code to test.
fn get_cmp(
    dag: &mut SelectionDAG,
    lhs: SDValue,
    mut rhs: SDValue,
    mut code: CondCode,
) -> (SDValue, u128) {
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
    if !dag.value_type(lhs).is_integer() {
        return None;
    }
    let mut code = cond_of(dag, dag.operand(node, 2));
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
    if !dag.value_type(node).is_integer() {
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
    let masked = if dag.opcode(cond) == ISD::And && dag.constant(dag.operand(cond, 1)) == Some(1) {
        cond
    } else {
        dag.get_node(ISD::And, vt, &[cond, one])
    };
    let zero = dag.get_constant(0, vt);
    lower_br_cc(dag, chain, CondCode::SetNe, masked, zero, dest)
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
