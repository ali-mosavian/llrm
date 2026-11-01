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
    set(&[Add, Sub], &[MVT::I16], Action::Legal);
    // Word logic is the bytes' ops (`lower_wide_logic`).
    set(&[And, Or, Xor], &[MVT::I16], Action::Custom);
    set(&[Shl, Srl, Sra], &[MVT::I8, MVT::I16], Action::Legal);
    set(&[CtTz, CtLz, CtPop, BSwap], &[MVT::I8, MVT::I16], Action::Expand);
    set(&[SignExtendInReg], &[MVT::I1, MVT::I8, MVT::I16], Action::Expand);
    set(&[Undef, Poison], &[MVT::I1, MVT::I8, MVT::I16], Action::Expand);
    tl.set_operation_action(BrCond, MVT::Other, Action::Custom);
    tl.set_operation_action(GlobalAddress, MVT::I16, Action::Custom);
    tl
}

/// The custom lowerings.
pub struct Z80Lowering;

impl Hooks for Z80Lowering {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::And | ISD::Or | ISD::Xor => Some(lower_wide_logic(dag, node)),
            ISD::BrCond => Some(lower_brcond(dag, node)),
            ISD::GlobalAddress => lower_global_address(dag, node),
            _ => None,
        }
    }
}

/// A word `and`, `or` or `xor` is the two bytes' ops joined: the pair is `lo |
/// (hi << 8)`, which the table selects as the two halves of one register.
fn lower_wide_logic(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let op = dag.opcode(node);
    let (a, b) = (dag.operand(node, 0), dag.operand(node, 1));
    if op == ISD::Or && is_pair_join(dag, a, b) {
        return node;
    }
    let eight = dag.get_constant(8, EVT::I8);
    let half = |dag: &mut SelectionDAG, value: SDValue, high: bool| {
        let value = if high { dag.get_node(ISD::Srl, EVT::I16, &[value, eight]) } else { value };
        dag.get_node(ISD::Truncate, EVT::I8, &[value])
    };
    let (alo, blo) = (half(dag, a, false), half(dag, b, false));
    let (ahi, bhi) = (half(dag, a, true), half(dag, b, true));
    let lo = dag.get_node(op, EVT::I8, &[alo, blo]);
    let hi = dag.get_node(op, EVT::I8, &[ahi, bhi]);
    dag.get_node(ISD::BuildPair, EVT::I16, &[lo, hi])
}

/// `lo | (hi << 8)` over extended bytes: the join of a `BuildPair`, which the
/// table selects as one pair. It is an `or` too, so it must not be split again.
fn is_pair_join(
    dag: &SelectionDAG,
    a: SDValue,
    b: SDValue,
) -> bool {
    let extended = |value: SDValue| {
        matches!(dag.opcode(value), ISD::ZeroExtend | ISD::AnyExtend)
            && dag.value_type(dag.operand(value, 0)) == EVT::I8
    };
    let high = |value: SDValue| dag.opcode(value) == ISD::Shl && extended(dag.operand(value, 0));
    (extended(a) && high(b)) || (extended(b) && high(a))
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

/// `LowerBRCOND`: `BRCOND chain, dest, cc, flags` over a compare of bytes or
/// words. A condition that is not an integer compare is tested against zero.
fn lower_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (chain, test, dest) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let (code, flags) =
        if dag.opcode(test) == ISD::SetCC && matches!(dag.value_type(dag.operand(test, 0)), EVT::I8 | EVT::I16) {
            let (op0, op1) = (dag.operand(test, 0), dag.operand(test, 1));
            let cc = cond_of(dag, dag.operand(test, 2));
            let (code, a, b) = translate_cc(dag, cc, op0, op1);
            (dag.get_constant(code, EVT::I8), dag.get_node(z80isd::CMP, EVT::I8, &[a, b]))
        } else {
            let byte = dag.get_node(ISD::ZeroExtend, EVT::I8, &[test]);
            let zero = dag.get_constant(0, EVT::I8);
            (dag.get_constant(cond::NZ, EVT::I8), dag.get_node(z80isd::CMP, EVT::I8, &[byte, zero]))
        };
    dag.get_node(z80isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags])
}
