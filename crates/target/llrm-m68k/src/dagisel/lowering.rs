//! `M68kTargetLowering`: the action table and the custom lowering of
//! conditions.
//!
//! # How it works
//! A comparison is an `M68kISD::CMP` (or an arithmetic node's second result)
//! whose value is the `i8` condition-code register. `SETCC`, `CMOV` and
//! `BRCOND` read a condition code (a plain constant, `M68k::CondCode`) and that
//! register. `setcc`, `select` and `brcond` on a value that is not a compare
//! test it against zero.
//!
//! DEVIATE: not translated yet are `LowerToBTST`, overflow arithmetic
//! (`LowerXALUO`), the `SETCC_CARRY` folds of `LowerSELECT`, the
//! and/or-of-setcc and xor-1 folds of `LowerBRCOND`, `EmitTest`'s narrowing of
//! a truncated arithmetic result, and floating-point conditions. Reason: each
//! needs DAG queries or nodes the slice does not reach. Exit: M3b.
//!
//! # See also
//! `llvm/lib/Target/M68k/M68kISelLowering.cpp`, pin d1106deb71cc: the
//! constructor (L48), `TranslateM68kCC` (L1774), `EmitTest` (L1888), `EmitCmp`
//! (L2103), `LowerSETCC` (L2126), `LowerSELECT` (L2244), `LowerBRCOND` (L2433).

use llrm_codegen::backend::selectiondag::isd::{CondCode, ISD};
use llrm_codegen::backend::selectiondag::legalize_dag::Hooks;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{Action, LoadExt, SDNodeFlags, SDValue, SelectionDAG, TargetLowering};
use llrm_codegen::backend::selectiondag::{NodeData, TargetLoweringInfo};

use super::generated::ids::isd as ids;
use super::hooks::Cpu;

/// The pin's `M68kISD` numbers, as the matcher table names them.
pub mod m68kisd {
    use super::{ISD, ids};

    pub const CMP: ISD = ISD::Target(ids::CMP);
    pub const SETCC: ISD = ISD::Target(ids::SETCC);
    pub const BRCOND: ISD = ISD::Target(ids::BRCOND);
    pub const CMOV: ISD = ISD::Target(ids::CMOV);
    pub const ADD: ISD = ISD::Target(ids::ADD);
    pub const SUB: ISD = ISD::Target(ids::SUB);
    pub const AND: ISD = ISD::Target(ids::AND);
    pub const OR: ISD = ISD::Target(ids::OR);
    pub const XOR: ISD = ISD::Target(ids::XOR);
}

/// `M68k::CondCode`.
pub mod cond {
    pub const T: u128 = 0;
    pub const F: u128 = 1;
    pub const HI: u128 = 2;
    pub const LS: u128 = 3;
    pub const CC: u128 = 4;
    pub const CS: u128 = 5;
    pub const NE: u128 = 6;
    pub const EQ: u128 = 7;
    pub const VC: u128 = 8;
    pub const VS: u128 = 9;
    pub const PL: u128 = 10;
    pub const MI: u128 = 11;
    pub const GE: u128 = 12;
    pub const LT: u128 = 13;
    pub const GT: u128 = 14;
    pub const LE: u128 = 15;
}

/// The constructor of `M68kTargetLowering`, in the pin's order.
///
/// DEVIATE: `BR_JT`, `BR_CC`, `SELECT_CC`, `VAARG`, the atomic and
/// `ATOMIC_FENCE` rows are left out: [`ISD`] has no such opcodes yet. Exit:
/// with the atomics in the DAG.
pub fn target_lowering(cpu: Cpu) -> TargetLowering {
    use ISD::*;
    let int = [MVT::I8, MVT::I16, MVT::I32];
    let m68020 = cpu == Cpu::M68020;
    let mut tl = TargetLowering::empty(TargetLoweringInfo::new(&int, false));
    tl.init_actions();
    for vt in [MVT::I1, MVT::I8, MVT::I16, MVT::I32, MVT::I64, MVT::I128] {
        for ext in [LoadExt::SExtLoad, LoadExt::ZExtLoad, LoadExt::ExtLoad] {
            tl.set_load_ext_action(ext, vt, MVT::I1, Action::Promote);
        }
    }
    for (vt, mem) in [(MVT::I64, MVT::I32), (MVT::I64, MVT::I16), (MVT::I64, MVT::I8)].into_iter().chain([
        (MVT::I32, MVT::I16),
        (MVT::I32, MVT::I8),
        (MVT::I16, MVT::I8),
    ]) {
        tl.set_trunc_store_action(vt, mem, Action::Expand);
    }
    let mut set = |ops: &[ISD], vts: &[MVT], action: Action| {
        for &op in ops {
            for &vt in vts {
                tl.set_operation_action(op, vt, action);
            }
        }
    };
    set(&[Mul], &[MVT::I8], Action::Promote);
    set(&[Mul], &[MVT::I16], Action::Legal);
    set(&[SDiv, UDiv], &[MVT::I8, MVT::I16], Action::Legal);
    let wide = if m68020 { Action::Legal } else { Action::LibCall };
    set(&[Mul, SDiv, UDiv], &[MVT::I32], wide);
    set(&[Mul], &[MVT::I64], Action::LibCall);
    set(&[SRem, URem], &[MVT::I8, MVT::I16], Action::Legal);
    set(&[SRem, URem], &[MVT::I32], Action::LibCall);
    for op in [UDivRem, SDivRem, MulHS, MulHU, UMulLoHi, SMulLoHi] {
        set(&[op], &[MVT::I8], Action::Promote);
        set(&[op], &[MVT::I16], Action::Legal);
        set(&[op], &[MVT::I32], Action::LibCall);
    }
    set(&[UMulLoHi, SMulLoHi], &[MVT::I8, MVT::I16], Action::Expand);
    set(&[SMulO, UMulO], &int, Action::Custom);
    set(&[ShlParts, SraParts, SrlParts], &[MVT::I32], Action::Custom);
    set(&[AddC, AddE, SubC, SubE, SAddO, UAddO, SSubO, USubO], &int, Action::Custom);
    set(&[BrCond], &[MVT::Other], Action::Custom);
    set(&[Select, SetCC, SetCCCarry], &int, Action::Custom);
    set(&[BSwap], &[MVT::I8, MVT::I16], Action::Expand);
    set(&[BSwap], &[MVT::I32], Action::Legal);
    set(&[CtTz, CtLz, CtPop], &int, Action::Expand);
    set(&[ConstantPool, JumpTable, GlobalAddress, ExternalSymbol], &[MVT::I32], Action::Custom);
    set(&[VAStart], &[MVT::Other], Action::Custom);
    set(&[VAEnd, VACopy, StackSave, StackRestore], &[MVT::Other], Action::Expand);
    set(&[DynamicStackAlloc], &[MVT::I32], Action::Custom);
    tl
}

/// The custom lowerings.
pub struct M68kLowering;

impl Hooks for M68kLowering {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::SetCC => lower_setcc(dag, node),
            ISD::Select => Some(lower_select(dag, node)),
            ISD::BrCond => Some(lower_brcond(dag, node)),
            _ => None,
        }
    }
}

/// A condition-code operand: `DAG.getConstant(code, DL, MVT::i8)`.
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

/// `TranslateM68kCC` for integers: the code, and a right-hand side that may
/// have been replaced (`x > -1` is `PL` on `x`).
fn translate_cc(
    dag: &mut SelectionDAG,
    cc: CondCode,
    rhs: &mut SDValue,
) -> u128 {
    if let Some(bits) = dag.constant(*rhs) {
        let vt = dag.value_type(*rhs);
        let all_ones = bits == llrm_codegen::backend::selectiondag::fold::mask(vt.size_in_bits());
        match cc {
            CondCode::SetGt if all_ones => {
                *rhs = dag.get_constant(0, vt);
                return cond::PL;
            }
            CondCode::SetLt if bits == 0 => return cond::MI,
            CondCode::SetLt if bits == 1 => {
                *rhs = dag.get_constant(0, vt);
                return cond::LE;
            }
            _ => {}
        }
    }
    match cc {
        CondCode::SetEq => cond::EQ,
        CondCode::SetGt => cond::GT,
        CondCode::SetGe => cond::GE,
        CondCode::SetLt => cond::LT,
        CondCode::SetLe => cond::LE,
        CondCode::SetNe => cond::NE,
        CondCode::SetULt => cond::CS,
        CondCode::SetUGe => cond::CC,
        CondCode::SetUGt => cond::HI,
        CondCode::SetULe => cond::LS,
        other => unreachable!("invalid integer condition {other:?}"),
    }
}

/// `hasNonFlagsUse`: a user that reads `op` as a value.
fn has_non_flags_use(
    dag: &SelectionDAG,
    op: SDValue,
) -> bool {
    dag.users(op).any(|user| {
        let value = SDValue { node: user, res: 0 };
        let flags_user = matches!(dag.opcode(value), ISD::BrCond | ISD::SetCC)
            || (dag.opcode(value) == ISD::Select && dag.operand(value, 0) == op);
        !flags_user
    })
}

/// `EmitTest`: the condition-code register of `op` against zero.
fn emit_test(
    dag: &mut SelectionDAG,
    op: SDValue,
    code: u128,
) -> SDValue {
    let vt = dag.value_type(op);
    let zero = dag.get_constant(0, vt);
    let test = |dag: &mut SelectionDAG| dag.get_node(m68kisd::CMP, EVT::I8, &[zero, op]);
    let need_carry = matches!(code, cond::HI | cond::CC | cond::CS | cond::LS);
    let wraps = !matches!(dag.opcode(op), ISD::Add | ISD::Sub | ISD::Mul | ISD::Shl)
        || dag.node(op).flags.0 & SDNodeFlags::NO_SIGNED_WRAP.0 == 0;
    let need_overflow = matches!(
        code,
        cond::GT | cond::GE | cond::LT | cond::LE | cond::VS | cond::VC
    ) && wraps;
    if op.res != 0 || need_overflow || need_carry {
        return test(dag);
    }
    let opcode = match dag.opcode(op) {
        // No BMI `andn` on m68k: an `and` with no user of the flags is a plain
        // test.
        ISD::And if !has_non_flags_use(dag, op) => return test(dag),
        ISD::And => m68kisd::AND,
        ISD::Add => m68kisd::ADD,
        ISD::Sub => m68kisd::SUB,
        ISD::Or => m68kisd::OR,
        ISD::Xor => m68kisd::XOR,
        m68kisd::ADD | m68kisd::SUB | m68kisd::OR | m68kisd::XOR | m68kisd::AND => {
            return SDValue { node: op.node, res: 1 };
        }
        _ => return test(dag),
    };
    // The arithmetic result is stored: the flags would need a second
    // instruction.
    if dag.users(op).any(|user| dag.opcode(SDValue { node: user, res: 0 }) == ISD::Store) {
        return test(dag);
    }
    let (lhs, rhs) = (dag.operand(op, 0), dag.operand(op, 1));
    let new = dag.get_node_multi(opcode, &[vt, EVT::I8], &[lhs, rhs]);
    dag.replace_all_uses_with(op, SDValue { node: new.node, res: 0 });
    SDValue { node: new.node, res: 1 }
}

/// `EmitCmp`: the flags of `op0 - op1`; `M68kISD::CMP` takes its operands as
/// `op1, op0`.
fn emit_cmp(
    dag: &mut SelectionDAG,
    op0: SDValue,
    op1: SDValue,
    code: u128,
) -> SDValue {
    if dag.is_null_constant(op1) {
        return emit_test(dag, op0, code);
    }
    dag.get_node(m68kisd::CMP, EVT::I8, &[op1, op0])
}

/// `LowerSETCC`, integer part.
fn lower_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (op0, mut op1) = (dag.operand(node, 0), dag.operand(node, 1));
    if !dag.value_type(op0).is_integer() {
        return None;
    }
    let cc = cond_of(dag, dag.operand(node, 2));
    // `setcc (setcc c), 0|1` with `==` or `!=` is the inner condition or its
    // opposite.
    if matches!(cc, CondCode::SetEq | CondCode::SetNe)
        && (dag.is_null_constant(op1) || dag.constant(op1) == Some(1))
        && dag.opcode(op0) == m68kisd::SETCC
    {
        let invert = (cc == CondCode::SetNe) ^ dag.is_null_constant(op1);
        if !invert {
            return Some(op0);
        }
        let code = opposite(dag.constant(dag.operand(op0, 0))?);
        let (code, flags) = (cc_operand(dag, code), dag.operand(op0, 1));
        return Some(dag.get_node(m68kisd::SETCC, EVT::I8, &[code, flags]));
    }
    let code = translate_cc(dag, cc, &mut op1);
    let flags = emit_cmp(dag, op0, op1, code);
    let code = cc_operand(dag, code);
    Some(dag.get_node(m68kisd::SETCC, EVT::I8, &[code, flags]))
}

/// `M68k::GetOppositeBranchCondition`: the codes pair up as `(2n, 2n + 1)`:
/// `HI`/`LS`, `CC`/`CS`, `NE`/`EQ`, ...
fn opposite(code: u128) -> u128 {
    code ^ 1
}

/// `LowerSELECT`, the general case: `CMOV false, true, cc, flags`.
fn lower_select(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (mut cond_value, op1, op2) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    if dag.opcode(cond_value) == ISD::SetCC {
        if let Some(lowered) = lower_setcc(dag, cond_value) {
            cond_value = lowered;
        }
    }
    let (code, flags) = if dag.opcode(cond_value) == m68kisd::SETCC {
        (dag.operand(cond_value, 0), dag.operand(cond_value, 1))
    } else {
        let code = cc_operand(dag, cond::NE);
        let cond_value = if dag.value_type(cond_value) == EVT::I1 {
            dag.get_node(ISD::ZeroExtend, EVT::I8, &[cond_value])
        } else {
            cond_value
        };
        (code, emit_test(dag, cond_value, cond::NE))
    };
    let vt = dag.value_type(node);
    dag.get_node(m68kisd::CMOV, vt, &[op2, op1, code, flags])
}

/// `LowerBRCOND`, the general case: `BRCOND chain, dest, cc, flags`.
fn lower_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (chain, mut cond_value, dest) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    if dag.opcode(cond_value) == ISD::SetCC {
        if let Some(lowered) = lower_setcc(dag, cond_value) {
            cond_value = lowered;
        }
    }
    let (code, flags) = if dag.opcode(cond_value) == m68kisd::SETCC {
        let (code, cmp) = (dag.operand(cond_value, 0), dag.operand(cond_value, 1));
        // A carry or overflow code reads the flags of the arithmetic itself.
        (code, cmp)
    } else {
        let code = cc_operand(dag, cond::NE);
        (code, emit_test(dag, cond_value, cond::NE))
    };
    dag.get_node(m68kisd::BRCOND, EVT::OTHER, &[chain, dest, code, flags])
}
