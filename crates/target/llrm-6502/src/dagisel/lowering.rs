//! The 6502's action table: what the DAG legalizers do with each operation.
//!
//! `i8` and `i16` are register types (`Imag8`, `Imag16`: the pair is what a
//! pointer lives in). Byte and word add, subtract and logic are instructions;
//! the carry forms (`UADDO`, `UADDO_CARRY`, `USUBO`, `USUBO_CARRY`, the carry
//! an `i1` in the `C` register) are too, so the type legalizer expands an `i32`
//! into two words joined by the carry, an `i64` into four. There is no
//! multiply, divide or shift by a variable: those are library calls.
//!
//! A comparison is `MOSISD::CMP a, b`: the `i8` flags of `a - b`. `SETCC`,
//! `CMOV` and `BRCOND` read a condition code (a constant, [`cond`]) and those
//! flags. Only the flags the 6502 branches on are codes: Z (`EQ`, `NE`), C
//! (`CC`, `CS`) and N (`MI`, `PL`). An unsigned greater-than swaps the
//! operands; a signed compare flips both sign bits and compares unsigned,
//! except against zero, where N is the sign.
//!
//! A word compare is `CMP` too (C and N come out of the high byte's
//! subtract-with-borrow); equality is `CMPZ`, whose Z covers both bytes.

use llrm_codegen::backend::selectiondag::isd::{CondCode, ISD};
use llrm_codegen::backend::selectiondag::legalize_dag::Hooks;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{Action, LoadExt, NodeData, SDValue, SelectionDAG};
use llrm_codegen::backend::selectiondag::{TargetLowering, TargetLoweringInfo};

use super::generated::ids::isd as ids;

/// The 6502's own nodes, numbered by `own_ids.txt`.
pub mod mosisd {
    use super::{ISD, ids};

    pub const CMP: ISD = ISD::Target(ids::CMP);
    pub const WRAPPER: ISD = ISD::Target(ids::WRAPPER);
    pub const MUL32: ISD = ISD::Target(ids::MUL32);
    pub const UDIV32: ISD = ISD::Target(ids::UDIV32);
    pub const SDIV32: ISD = ISD::Target(ids::SDIV32);
    pub const UREM32: ISD = ISD::Target(ids::UREM32);
    pub const SREM32: ISD = ISD::Target(ids::SREM32);
    pub const SHL32: ISD = ISD::Target(ids::SHL32);
    pub const SRL32: ISD = ISD::Target(ids::SRL32);
    pub const SRA32: ISD = ISD::Target(ids::SRA32);
    pub const CMPZ: ISD = ISD::Target(ids::CMPZ);
    pub const BRCOND: ISD = ISD::Target(ids::BRCOND);
    pub const SETCC: ISD = ISD::Target(ids::SETCC);
    pub const CMOV: ISD = ISD::Target(ids::CMOV);
}

/// Condition codes; a code and its opposite differ in bit 0.
pub mod cond {
    pub const EQ: u128 = 0;
    pub const NE: u128 = 1;
    pub const CC: u128 = 2;
    pub const CS: u128 = 3;
    pub const MI: u128 = 4;
    pub const PL: u128 = 5;
}

pub fn target_lowering() -> TargetLowering {
    use ISD::*;
    let mut tl = TargetLowering::empty(TargetLoweringInfo::new(&[MVT::I1, MVT::I8, MVT::I16], false));
    tl.init_actions();
    for vt in [MVT::I1, MVT::I8, MVT::I16, MVT::I32, MVT::I64] {
        for ext in [LoadExt::SExtLoad, LoadExt::ZExtLoad, LoadExt::ExtLoad] {
            tl.set_load_ext_action(ext, vt, MVT::I1, Action::Promote);
        }
    }
    // a sign-extending load is a load and a sign extension: no instruction reads one
    for (vt, mem) in [(MVT::I16, MVT::I8), (MVT::I32, MVT::I8), (MVT::I32, MVT::I16), (MVT::I64, MVT::I8)]
        .into_iter()
        .chain([(MVT::I64, MVT::I16), (MVT::I64, MVT::I32)])
    {
        tl.set_load_ext_action(LoadExt::SExtLoad, vt, mem, Action::Expand);
    }
    for (vt, mem) in [(MVT::I64, MVT::I32), (MVT::I64, MVT::I16), (MVT::I64, MVT::I8), (MVT::I32, MVT::I16)]
        .into_iter()
        .chain([(MVT::I32, MVT::I8), (MVT::I16, MVT::I8)])
    {
        tl.set_trunc_store_action(vt, mem, Action::Expand);
    }
    let mut set = |ops: &[ISD], action: Action| {
        for &op in ops {
            for vt in [MVT::I8, MVT::I16] {
                tl.set_operation_action(op, vt, action);
            }
        }
    };
    set(&[Add, Sub, And, Or, Xor, UAddO, USubO, UAddOCarry, USubOCarry], Action::Legal);
    // Multiply, divide, remainder and every shift are instructions; the
    // variable ones expand to runtime calls where they are emitted.
    set(&[Mul, SDiv, UDiv, SRem, URem, Shl, Srl, Sra], Action::Legal);
    set(&[UDivRem, SDivRem, MulHS, MulHU, UMulLoHi, SMulLoHi], Action::Expand);
    set(&[CtTz, CtLz, CtPop, BSwap], Action::Expand);
    // rotates are shifts and an or, built where they are lowered
    set(&[Rotl, Rotr], Action::Custom);
    set(&[ZeroExtend, AnyExtend], Action::Custom);
    set(&[SignExtend], Action::Custom);
    // a sign extension in a register is a pair of shifts (a negated bit for a bit)
    set(&[SignExtendInReg], Action::Expand);
    // its action is read at the type it extends from, a bit among them
    tl.set_operation_action(SignExtendInReg, MVT::I1, Action::Expand);
    // A long multiply, divide or shift by a variable is one node of two words
    // (`lower_long`).
    for op in [Mul, SDiv, UDiv, SRem, URem, Shl, Srl, Sra, Rotl, Rotr] {
        tl.set_operation_action(op, MVT::I32, Action::Custom);
    }
    tl.set_operation_action(BrCond, MVT::Other, Action::Custom);
    tl.set_operation_action(GlobalAddress, MVT::I16, Action::Custom);
    for op in [SetCC, Select] {
        for vt in [MVT::I8, MVT::I16] {
            tl.set_operation_action(op, vt, Action::Custom);
        }
    }
    tl
}

/// The custom lowerings.
pub struct MosLowering;

impl Hooks for MosLowering {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::Rotl | ISD::Rotr => Some(lower_rotate(dag, node)),
            ISD::SetCC => lower_setcc(dag, node),
            ISD::Select => Some(lower_select(dag, node)),
            ISD::BrCond => Some(lower_brcond(dag, node)),
            ISD::GlobalAddress => lower_global_address(dag, node),
            ISD::ZeroExtend | ISD::AnyExtend | ISD::SignExtend => Some(lower_extend(dag, node)),
            ISD::Mul | ISD::SDiv | ISD::UDiv | ISD::SRem | ISD::URem | ISD::Shl | ISD::Srl | ISD::Sra
                if dag.value_type(node) == EVT::I32 =>
            {
                lower_long(dag, node)
            }
            _ => None,
        }
    }
}

/// `expandROT`: `rotl x, c` is `(x << c) | (x >> (w - c))`, the amounts taken modulo the width. A
/// constant amount is two shifts by constants (the first is a rotate by nothing when the amount is 0).
fn lower_rotate(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (x, amount) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let width = vt.size_in_bits() as u128;
    let (left, right) = if dag.opcode(node) == ISD::Rotl { (ISD::Shl, ISD::Srl) } else { (ISD::Srl, ISD::Shl) };
    let shift_vt = dag.value_type(amount);
    if let Some(c) = dag.constant(amount) {
        let c = c % width;
        if c == 0 {
            return x;
        }
        let (by, back) = (dag.get_constant(c, shift_vt), dag.get_constant(width - c, shift_vt));
        let (high, low) = (dag.get_node(left, vt, &[x, by]), dag.get_node(right, vt, &[x, back]));
        return dag.get_node(ISD::Or, vt, &[high, low]);
    }
    let mask = dag.get_constant(width - 1, shift_vt);
    let by = dag.get_node(ISD::And, shift_vt, &[amount, mask]);
    let zero = dag.get_constant(0, shift_vt);
    let negated = dag.get_node(ISD::Sub, shift_vt, &[zero, amount]);
    let back = dag.get_node(ISD::And, shift_vt, &[negated, mask]);
    let (high, low) = (dag.get_node(left, vt, &[x, by]), dag.get_node(right, vt, &[x, back]));
    dag.get_node(ISD::Or, vt, &[high, low])
}

/// A long (`i32`) multiply, divide, remainder or shift by a variable:
/// `MOSISD::<op>32` of the operands' halves, two words joined. A shift by a
/// constant is left to the type legalizer.
fn lower_long(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let opcode = match dag.opcode(node) {
        ISD::Mul => mosisd::MUL32,
        ISD::UDiv => mosisd::UDIV32,
        ISD::SDiv => mosisd::SDIV32,
        ISD::URem => mosisd::UREM32,
        ISD::SRem => mosisd::SREM32,
        ISD::Shl => mosisd::SHL32,
        ISD::Srl => mosisd::SRL32,
        _ => mosisd::SRA32,
    };
    let shift = matches!(opcode, mosisd::SHL32 | mosisd::SRL32 | mosisd::SRA32);
    let (a, b) = (dag.operand(node, 0), dag.operand(node, 1));
    if shift && dag.constant(b).is_some() {
        return None;
    }
    let sixteen = dag.get_constant(16, EVT::I8);
    let mut operands = Vec::new();
    for (at, value) in [a, b].into_iter().enumerate() {
        if shift && at == 1 {
            operands.push(dag.get_node(ISD::Truncate, EVT::I8, &[value]));
            continue;
        }
        let high = dag.get_node(ISD::Srl, EVT::I32, &[value, sixteen]);
        operands.push(dag.get_node(ISD::Truncate, EVT::I16, &[value]));
        operands.push(dag.get_node(ISD::Truncate, EVT::I16, &[high]));
    }
    let pair = dag.get_node_multi(opcode, &[EVT::I16, EVT::I16], &operands);
    Some(dag.get_node(
        ISD::BuildPair,
        EVT::I32,
        &[SDValue { node: pair.node, res: 0 }, SDValue { node: pair.node, res: 1 }],
    ))
}

/// A condition widened to a byte or a word is `select cond, 1, 0`; any other
/// extension is selected as it is.
fn lower_extend(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let condition = dag.operand(node, 0);
    if dag.value_type(condition) != EVT::I1 {
        return node;
    }
    let vt = dag.value_type(node);
    // a sign-extended bit is all ones (`select c, 1, 2` reaches here as `sext(c) + 2`)
    let width = vt.size_in_bits();
    let set = if dag.opcode(node) == ISD::SignExtend { (1u128 << width) - 1 } else { 1 };
    let (one, zero) = (dag.get_constant(set, vt), dag.get_constant(0, vt));
    let select = dag.get_node(ISD::Select, vt, &[condition, one, zero]);
    lower_select(dag, select)
}

/// `LowerGlobalAddress`: `WRAPPER` of the address, an immediate the linker
/// fills in.
fn lower_global_address(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let NodeData::Global { symbol, offset } = dag.node(node).data else { return None };
    let address = dag.get_target_global_address(EVT::I16, symbol, offset);
    Some(dag.get_node(mosisd::WRAPPER, EVT::I16, &[address]))
}

fn cc_operand(
    dag: &mut SelectionDAG,
    code: u128,
) -> SDValue {
    dag.get_constant(code, EVT::I8)
}

fn cond_of(
    dag: &SelectionDAG,
    setcc: SDValue,
) -> CondCode {
    match dag.node(setcc).data {
        NodeData::CondCode(cc) => cc,
        other => unreachable!("a condition code operand, not {other:?}"),
    }
}

/// `x ^ sign bit`: so a signed compare is an unsigned one. A constant folds.
fn flip_sign(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> SDValue {
    let vt = dag.value_type(value);
    let sign = 1u128 << (vt.size_in_bits() - 1);
    match dag.constant(value) {
        Some(bits) => dag.get_constant(bits ^ sign, vt),
        None => {
            let sign = dag.get_constant(sign, vt);
            dag.get_node(ISD::Xor, vt, &[value, sign])
        }
    }
}

/// The code a comparison of `op0` with `op1` branches on, and the operands to
/// compare (possibly swapped or sign-flipped).
fn translate_cc(
    dag: &mut SelectionDAG,
    cc: CondCode,
    op0: SDValue,
    op1: SDValue,
) -> (u128, SDValue, SDValue) {
    use CondCode::*;
    let against_zero = dag.is_null_constant(op1);
    // `x > C` is `x >= C + 1`: the constant stays the right-hand side, an immediate of the compare
    // (AArch64's `getAArch64Cmp` adjusts the immediate the same way)
    let width = dag.value_type(op0).size_in_bits();
    if let (SetUGt | SetULe | SetGt | SetLe, Some(c)) = (cc, dag.constant(op1)) {
        let top = match cc {
            SetUGt | SetULe => (1u128 << width) - 1,
            _ => (1u128 << (width - 1)) - 1,
        };
        if c != top {
            let next = dag.get_constant((c + 1) & ((1u128 << width) - 1), dag.value_type(op0));
            let wider = if matches!(cc, SetUGt | SetGt) { if cc == SetUGt { SetUGe } else { SetGe } } else if cc == SetULe { SetULt } else { SetLt };
            return translate_cc(dag, wider, op0, next);
        }
    }
    match cc {
        SetEq => (cond::EQ, op0, op1),
        SetNe => (cond::NE, op0, op1),
        SetULt => (cond::CC, op0, op1),
        SetUGe => (cond::CS, op0, op1),
        SetUGt => (cond::CC, op1, op0),
        SetULe => (cond::CS, op1, op0),
        SetLt if against_zero => (cond::MI, op0, op1),
        SetGe if against_zero => (cond::PL, op0, op1),
        SetLt | SetGe => {
            let (a, b) = (flip_sign(dag, op0), flip_sign(dag, op1));
            (if cc == SetLt { cond::CC } else { cond::CS }, a, b)
        }
        SetGt | SetLe => {
            // a > b is b < a, a <= b is b >= a
            let (a, b) = (flip_sign(dag, op1), flip_sign(dag, op0));
            (if cc == SetGt { cond::CC } else { cond::CS }, a, b)
        }
        other => unreachable!("invalid integer condition {other:?}"),
    }
}

/// `LowerSETCC`, integer part: `SETCC cc, (CMP a, b)`.
fn lower_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (mut op0, mut op1) = (dag.operand(node, 0), dag.operand(node, 1));
    let mut cc = cond_of(dag, dag.operand(node, 2));
    if dag.value_type(op0) == EVT::I1 {
        // Compare the bits as bytes of 0 or 1. A signed order is the unsigned one reversed, since
        // a set bit is -1.
        (op0, op1) = (condition_byte(dag, op0), condition_byte(dag, op1));
        cc = match cc {
            CondCode::SetLt => CondCode::SetUGt,
            CondCode::SetGt => CondCode::SetULt,
            CondCode::SetLe => CondCode::SetUGe,
            CondCode::SetGe => CondCode::SetULe,
            other => other,
        };
    }
    if !matches!(dag.value_type(op0), EVT::I8 | EVT::I16) {
        return None;
    }
    let (code, a, b) = translate_cc(dag, cc, op0, op1);
    // Equality on a word needs Z over both bytes.
    let compare =
        if dag.value_type(a) == EVT::I16 && matches!(code, cond::EQ | cond::NE) { mosisd::CMPZ } else { mosisd::CMP };
    let flags = dag.get_node(compare, EVT::I8, &[a, b]);
    let code = cc_operand(dag, code);
    Some(dag.get_node(mosisd::SETCC, EVT::I8, &[code, flags]))
}

/// The code and flags a condition value branches on: a lowered compare, or the
/// value tested against zero (`NE`).
fn condition(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> (SDValue, SDValue) {
    let value = if dag.opcode(value) == ISD::SetCC { lower_setcc(dag, value).unwrap_or(value) } else { value };
    if dag.opcode(value) == mosisd::SETCC {
        return (dag.operand(value, 0), dag.operand(value, 1));
    }
    let byte = condition_byte(dag, value);
    let zero = dag.get_constant(0, EVT::I8);
    let flags = dag.get_node(mosisd::CMP, EVT::I8, &[byte, zero]);
    (cc_operand(dag, cond::NE), flags)
}

/// A condition value as a byte of 0 or 1. A logic op over conditions stays
/// in bytes: zero-extending it legalizes back into a `select` on itself, whose
/// lowering is `CMOV` on a compare of that `select`.
fn condition_byte(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> SDValue {
    match dag.value_type(value) {
        EVT::I1 => match dag.opcode(value) {
            ISD::Constant => {
                let bit = dag.constant(value).expect("a constant") & 1;
                dag.get_constant(bit, EVT::I8)
            }
            ISD::SetCC => match lower_setcc(dag, value) {
                Some(lowered) => lowered,
                None => dag.get_node(ISD::ZeroExtend, EVT::I8, &[value]),
            },
            ISD::Truncate if matches!(dag.value_type(dag.operand(value, 0)), EVT::I8 | EVT::I16) => {
                let wide = dag.operand(value, 0);
                let byte = if dag.value_type(wide) == EVT::I8 { wide } else { dag.get_node(ISD::Truncate, EVT::I8, &[wide]) };
                let one = dag.get_constant(1, EVT::I8);
                dag.get_node(ISD::And, EVT::I8, &[byte, one])
            }
            ISD::Freeze => {
                let bit = dag.operand(value, 0);
                condition_byte(dag, bit)
            }
            ISD::Select => {
                let (test, a, b) = (dag.operand(value, 0), dag.operand(value, 1), dag.operand(value, 2));
                let (a, b) = (condition_byte(dag, a), condition_byte(dag, b));
                dag.get_node(ISD::Select, EVT::I8, &[test, a, b])
            }
            op @ (ISD::And | ISD::Or | ISD::Xor) => {
                let (a, b) = (dag.operand(value, 0), dag.operand(value, 1));
                let (a, b) = (condition_byte(dag, a), condition_byte(dag, b));
                dag.get_node(op, EVT::I8, &[a, b])
            }
            _ => dag.get_node(ISD::ZeroExtend, EVT::I8, &[value]),
        },
        _ => value,
    }
}

/// `LowerSELECT`: `CMOV false, true, cc, flags`.
fn lower_select(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (test, on_true, on_false) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let (code, flags) = condition(dag, test);
    let vt = dag.value_type(node);
    dag.get_node(mosisd::CMOV, vt, &[on_false, on_true, code, flags])
}

/// `LowerBRCOND`: `BRCOND chain, dest, cc, flags`.
fn lower_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (chain, test, dest) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let (code, flags) = condition(dag, test);
    dag.get_node(mosisd::BRCOND, EVT::OTHER, &[chain, dest, code, flags])
}
