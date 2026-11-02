//! The Z80's action table: what the DAG legalizers do with each operation.
//!
//! `i8` and `i16` are register types (`GR8`, `GR16`). Byte add, subtract and
//! logic work on A, word add and subtract on HL; word logic is the two bytes'
//! ops joined. Shifts by a constant are instructions. There is no multiply,
//! divide, shift by a variable or `i32` instruction yet: those are refused by
//! the legalizer, not papered over.
//!
//! A comparison is `Z80ISD::CMP a, b`: the `i8` flags of `a - b`. `BRCOND`
//! reads a condition code (a constant, [`cond`]) and those flags. Only the
//! flags a `jp cc` tests are codes: Z (`EQ`, `NE`) and C (`CC`, `CS`). An
//! unsigned greater-than swaps the operands; a signed compare flips both sign
//! bits and compares unsigned. A word compare is `CMP` too: `sbc hl,rr` sets Z
//! over all sixteen bits and C as the borrow.

use llrm_codegen::backend::selectiondag::isd::{CondCode, ISD};
use llrm_codegen::backend::selectiondag::legalize_dag::Hooks;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{Action, LoadExt, NodeData, SDValue, SelectionDAG};
use llrm_codegen::backend::selectiondag::{TargetLowering, TargetLoweringInfo};

use super::generated::ids::isd as ids;

/// The Z80's own nodes, numbered by `own_ids.txt`.
pub mod z80isd {
    use super::{ISD, ids};

    pub const CMP: ISD = ISD::Target(ids::CMP);
    pub const WRAPPER: ISD = ISD::Target(ids::WRAPPER);
    pub const BRCOND: ISD = ISD::Target(ids::BRCOND);
    pub const SETCC: ISD = ISD::Target(ids::SETCC);
    pub const TSTS: ISD = ISD::Target(ids::TSTS);
    pub const CMOV: ISD = ISD::Target(ids::CMOV);
}

/// Condition codes; a code and its opposite differ in bit 0.
pub mod cond {
    pub const NZ: u128 = 0;
    pub const Z: u128 = 1;
    pub const NC: u128 = 2;
    pub const C: u128 = 3;
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
    // A sign-extending load is a load and a sign extension; a zero- or
    // any-extending one to a type that is not a register is not formed, to
    // be split again.
    for (vt, mem) in [(MVT::I16, MVT::I8), (MVT::I32, MVT::I8), (MVT::I32, MVT::I16), (MVT::I64, MVT::I8)]
        .into_iter()
        .chain([(MVT::I64, MVT::I16), (MVT::I64, MVT::I32)])
    {
        tl.set_load_ext_action(LoadExt::SExtLoad, vt, mem, Action::Expand);
    }
    for (vt, mem) in
        [(MVT::I32, MVT::I8), (MVT::I32, MVT::I16), (MVT::I64, MVT::I8), (MVT::I64, MVT::I16), (MVT::I64, MVT::I32)]
    {
        for ext in [LoadExt::ZExtLoad, LoadExt::ExtLoad] {
            tl.set_load_ext_action(ext, vt, mem, Action::Expand);
        }
    }
    for (vt, mem) in [(MVT::I64, MVT::I32), (MVT::I64, MVT::I16), (MVT::I64, MVT::I8), (MVT::I32, MVT::I16)]
        .into_iter()
        .chain([(MVT::I32, MVT::I8), (MVT::I16, MVT::I8)])
    {
        tl.set_trunc_store_action(vt, mem, Action::Expand);
    }
    let mut set = |ops: &[ISD], vts: &[MVT], action: Action| {
        for &op in ops {
            for &vt in vts {
                tl.set_operation_action(op, vt, action);
            }
        }
    };
    set(&[Add, Sub, And, Or, Xor], &[MVT::I8], Action::Legal);
    set(&[Add, Sub, And, Or, Xor], &[MVT::I16], Action::Legal);
    // A shift by a constant is an instruction; by a variable it is a call
    // (`lower_shift`).
    set(&[Shl, Srl, Sra], &[MVT::I8, MVT::I16], Action::Custom);
    // `RTLIB::MUL_I16` and the others: libgcc's names (`RuntimeLibcalls.def`),
    // with the same family for bytes, which this runtime has because a byte
    // loop is half the word's.
    set(&[Mul, SDiv, UDiv, SRem, URem], &[MVT::I8, MVT::I16], Action::LibCall);
    set(&[ShlParts, SrlParts, SraParts], &[MVT::I8, MVT::I16], Action::Expand);
    set(&[MulHS, MulHU, UMulLoHi, SMulLoHi, SDivRem, UDivRem], &[MVT::I8, MVT::I16], Action::Expand);
    set(&[CtTz, CtLz, CtPop, BSwap], &[MVT::I8, MVT::I16], Action::Expand);
    set(&[SignExtendInReg], &[MVT::I1, MVT::I8, MVT::I16], Action::Expand);
    set(&[Undef, Poison], &[MVT::I1, MVT::I8, MVT::I16], Action::Expand);
    set(&[ZeroExtend, AnyExtend, SignExtend], &[MVT::I8, MVT::I16], Action::Custom);
    set(&[SetCC, Select], &[MVT::I8, MVT::I16], Action::Custom);
    for (vt, names) in [
        (MVT::I8, ["__mulqi3", "__divqi3", "__udivqi3", "__modqi3", "__umodqi3"]),
        (MVT::I16, ["__mulhi3", "__divhi3", "__udivhi3", "__modhi3", "__umodhi3"]),
    ] {
        for (op, name) in [Mul, SDiv, UDiv, SRem, URem].into_iter().zip(names) {
            tl.set_libcall_name(op, vt, name);
        }
    }
    // the longs are expanded to words; a divide or a variable shift of one is
    // the call, not a word-wise expansion
    for (op, name) in [SDiv, UDiv, SRem, URem, Shl, Srl, Sra].into_iter().zip([
        "__divsi3",
        "__udivsi3",
        "__modsi3",
        "__umodsi3",
        "__ashlsi3",
        "__lshrsi3",
        "__ashrsi3",
    ]) {
        tl.set_libcall_name(op, MVT::I32, name);
    }
    tl.set_operation_action(BrCond, MVT::Other, Action::Custom);
    tl.set_operation_action(GlobalAddress, MVT::I16, Action::Custom);
    tl
}

/// The custom lowerings.
pub struct Z80Lowering;

impl Z80Lowering {
    /// A shift by a constant is selected as it is; by a variable it is the call
    /// of `__ashl`, `__lshr` or `__ashr` (`RTLIB::SHL_I16`, ...), the
    /// amount a byte.
    fn lower_shift(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        let (value, amount) = (dag.operand(node, 0), dag.operand(node, 1));
        if dag.constant(amount).is_some() {
            return Some(node);
        }
        let vt = dag.value_type(node);
        let name = match (dag.opcode(node), vt) {
            (ISD::Shl, EVT::I8) => "__ashlqi3",
            (ISD::Srl, EVT::I8) => "__lshrqi3",
            (ISD::Sra, EVT::I8) => "__ashrqi3",
            (ISD::Shl, _) => "__ashlhi3",
            (ISD::Srl, _) => "__lshrhi3",
            _ => "__ashrhi3",
        };
        self.make_lib_call(dag, name, &[value, amount], vt, dag.opcode(node) == ISD::Sra)
    }
}

impl Hooks for Z80Lowering {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::Shl | ISD::Srl | ISD::Sra => self.lower_shift(dag, node),
            ISD::SetCC => lower_setcc(dag, node),
            ISD::Select => Some(lower_select(dag, node)),
            ISD::BrCond => Some(lower_brcond(dag, node)),
            ISD::ZeroExtend | ISD::AnyExtend | ISD::SignExtend => Some(lower_extend(dag, node)),
            ISD::GlobalAddress => lower_global_address(dag, node),
            _ => None,
        }
    }

    /// `makeLibCall` through `LowerCall`: the routine's address is a target
    /// external symbol, the chain is the entry node (the result is the
    /// value, nothing orders the call after a store).
    fn make_lib_call(
        &mut self,
        dag: &mut SelectionDAG,
        name: &'static str,
        args: &[SDValue],
        ret: EVT,
        signed: bool,
    ) -> Option<SDValue> {
        use super::call::{Ext, In, Out, lower_call};
        let ext = if signed { Ext::Sign } else { Ext::Zero };
        // `LowerCallTo` splits a type the target holds in no register: a long
        // travels as two words, low first
        let mut outs = Vec::new();
        for &value in args {
            let vt = dag.value_type(value);
            let parts = if vt == EVT::I32 {
                let sixteen = dag.get_constant(16, EVT::I8);
                let high = dag.get_node(ISD::Srl, vt, &[value, sixteen]);
                vec![dag.get_node(ISD::Truncate, EVT::I16, &[value]), dag.get_node(ISD::Truncate, EVT::I16, &[high])]
            } else {
                vec![value]
            };
            let split = parts.len() > 1;
            outs.extend(parts.into_iter().map(|part| Out {
                value: part,
                ext,
                pointer: false,
                arg_vt: vt,
                split,
                variadic: false,
            }));
        }
        let part_vt = if ret == EVT::I32 { EVT::I16 } else { ret };
        let ins = vec![
            In { vt: part_vt, pointer: false, arg_vt: ret, split: ret == EVT::I32 };
            if ret == EVT::I32 { 2 } else { 1 }
        ];
        let callee = dag.get_target_external_symbol(EVT::I16, name);
        let root = dag.entry_node();
        let (_chain, values) = lower_call(dag, root, callee, &outs, &ins).ok()?;
        match values[..] {
            [one] => Some(one),
            [lo, hi] => Some(dag.get_node(ISD::BuildPair, ret, &[lo, hi])),
            _ => None,
        }
    }
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
    // a logic op over compares is a byte of 0 or 1 already (`condition_byte`):
    // widened, it is that byte, not a select that tests the byte it was
    // just made into
    if dag.opcode(node) != ISD::SignExtend
        && matches!(dag.opcode(condition), ISD::And | ISD::Or | ISD::Xor)
        && is_condition_tree(dag, condition)
    {
        let byte = condition_byte(dag, condition);
        return if vt == EVT::I8 { byte } else { dag.get_node(ISD::ZeroExtend, vt, &[byte]) };
    }
    // a sign-extended bit is all ones (`select c, 1, 2` reaches here as
    // `sext(c) + 2`)
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
    Some(dag.get_node(z80isd::WRAPPER, EVT::I16, &[address]))
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
/// compare (possibly swapped or sign-flipped). `x > C` is `x >= C + 1`, the
/// constant staying the right-hand side.
fn translate_cc(
    dag: &mut SelectionDAG,
    cc: CondCode,
    op0: SDValue,
    op1: SDValue,
) -> (u128, SDValue, SDValue) {
    use CondCode::*;
    let width = dag.value_type(op0).size_in_bits();
    if let (SetUGt | SetULe | SetGt | SetLe, Some(c)) = (cc, dag.constant(op1)) {
        let top = match cc {
            SetUGt | SetULe => (1u128 << width) - 1,
            _ => (1u128 << (width - 1)) - 1,
        };
        if c != top {
            let next = dag.get_constant((c + 1) & ((1u128 << width) - 1), dag.value_type(op0));
            let wider = match cc {
                SetUGt => SetUGe,
                SetGt => SetGe,
                SetULe => SetULt,
                _ => SetLt,
            };
            return translate_cc(dag, wider, op0, next);
        }
    }
    match cc {
        SetEq => (cond::Z, op0, op1),
        SetNe => (cond::NZ, op0, op1),
        SetULt => (cond::C, op0, op1),
        SetUGe => (cond::NC, op0, op1),
        SetUGt => (cond::C, op1, op0),
        SetULe => (cond::NC, op1, op0),
        SetLt | SetGe => {
            let (a, b) = (flip_sign(dag, op0), flip_sign(dag, op1));
            (if cc == SetLt { cond::C } else { cond::NC }, a, b)
        }
        SetGt | SetLe => {
            // a > b is b < a, a <= b is b >= a
            let (a, b) = (flip_sign(dag, op1), flip_sign(dag, op0));
            (if cc == SetGt { cond::C } else { cond::NC }, a, b)
        }
        other => unreachable!("invalid integer condition {other:?}"),
    }
}

/// The condition code and flags of `op0 cc op1`. A comparison of a signed value
/// with zero is the test of its sign bit (`bit 7`), not a compare of the value
/// with a flipped copy: `x < 0` is `NZ`, `x >= 0` is `Z`.
fn compare(
    dag: &mut SelectionDAG,
    mut cc: CondCode,
    mut op0: SDValue,
    mut op1: SDValue,
) -> (u128, SDValue) {
    use CondCode::*;
    let vt = dag.value_type(op0);
    let minus_one = (1u128 << vt.size_in_bits()) - 1;
    // x > -1 is x >= 0, x <= -1 is x < 0
    if let (SetGt | SetLe, Some(c)) = (cc, dag.constant(op1)) {
        if c == minus_one {
            cc = if cc == SetGt { SetGe } else { SetLt };
            op1 = dag.get_constant(0, vt);
        }
    }
    if matches!(cc, SetLt | SetGe) && dag.constant(op1) == Some(0) {
        let flags = dag.get_node(z80isd::TSTS, EVT::I8, &[op0]);
        return (if cc == SetLt { cond::NZ } else { cond::Z }, flags);
    }
    // `cp (hl)` takes the memory operand second: a loaded byte compared with a
    // register is compared the other way round, with the order of the
    // condition reversed
    if vt == EVT::I8 && dag.opcode(op0) == ISD::Load && dag.opcode(op1) != ISD::Load && dag.constant(op1).is_none() {
        cc = match cc {
            SetULt => SetUGt,
            SetUGt => SetULt,
            SetULe => SetUGe,
            SetUGe => SetULe,
            SetLt => SetGt,
            SetGt => SetLt,
            SetLe => SetGe,
            SetGe => SetLe,
            other => other,
        };
        std::mem::swap(&mut op0, &mut op1);
        let op1 = op1;
        let (code, a, b) = translate_cc(dag, cc, op0, op1);
        return (code, dag.get_node(z80isd::CMP, EVT::I8, &[a, b]));
    }
    let (code, a, b) = translate_cc(dag, cc, op0, op1);
    (code, dag.get_node(z80isd::CMP, EVT::I8, &[a, b]))
}

/// `LowerSETCC`, integer part: `SETCC cc, (CMP a, b)`.
fn lower_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (mut op0, mut op1) = (dag.operand(node, 0), dag.operand(node, 1));
    let mut cc = cond_of(dag, dag.operand(node, 2));
    if dag.value_type(op0) == EVT::I1 {
        // Compare the bits as bytes of 0 or 1. A signed order is the unsigned
        // one reversed, since a set bit is -1.
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
    let (code, flags) = compare(dag, cc, op0, op1);
    let code = cc_operand(dag, code);
    Some(dag.get_node(z80isd::SETCC, EVT::I8, &[code, flags]))
}

/// The code and flags a condition value branches on: a lowered compare, or the
/// value tested against zero (`NE`).
fn condition(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> (SDValue, SDValue) {
    let value = if dag.opcode(value) == ISD::SetCC { lower_setcc(dag, value).unwrap_or(value) } else { value };
    if dag.opcode(value) == z80isd::SETCC {
        return (dag.operand(value, 0), dag.operand(value, 1));
    }
    let byte = condition_byte(dag, value);
    let zero = dag.get_constant(0, EVT::I8);
    let flags = dag.get_node(z80isd::CMP, EVT::I8, &[byte, zero]);
    (cc_operand(dag, cond::NZ), flags)
}

/// Whether `condition_byte` of `value` is made of lowered compares and
/// constants alone, with no fallback zero extension (which would come back
/// here).
fn is_condition_tree(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    if dag.value_type(value) != EVT::I1 {
        return false;
    }
    let operand = |i: usize| dag.operand(value, i);
    let tree = |i: usize| is_condition_tree(dag, operand(i));
    match dag.opcode(value) {
        ISD::Constant => true,
        ISD::SetCC => matches!(dag.value_type(operand(0)), EVT::I8 | EVT::I16) || (tree(0) && tree(1)),
        ISD::Truncate => matches!(dag.value_type(operand(0)), EVT::I8 | EVT::I16),
        ISD::Freeze => tree(0),
        ISD::Select => tree(1) && tree(2),
        ISD::And | ISD::Or | ISD::Xor => tree(0) && tree(1),
        _ => false,
    }
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
                let byte =
                    if dag.value_type(wide) == EVT::I8 { wide } else { dag.get_node(ISD::Truncate, EVT::I8, &[wide]) };
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
    dag.get_node(z80isd::CMOV, vt, &[on_false, on_true, code, flags])
}

/// `LowerBRCOND`: `BRCOND chain, dest, cc, flags`.
fn lower_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (chain, test, dest) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let (code, flags) = condition(dag, test);
    dag.get_node(z80isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags])
}
