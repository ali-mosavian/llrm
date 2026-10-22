//! `X86TargetLowering`'s custom lowerings of conditions: `setcc`, `select`
//! and `brcond` become `X86ISD::CMP`, `SETCC`, `CMOV` and `BRCOND` on EFLAGS.
//!
//! # How it works
//! A comparison is an `X86ISD::SUB` (or `CMP` against zero) whose second
//! result is the `i32` flags value. `SETCC` reads a condition code (a target
//! constant, the pin's `X86::CondCode`) and the flags; `CMOV` and `BRCOND`
//! read the same. `select` and `brcond` on an `i1`/`i8` that is not a compare
//! test it against zero. A CPU with no CMOV gets the same DAG: the matcher
//! picks the `CMOV_GR*` pseudos and the emitter expands them to a branch
//! (`EmitLoweredSelect`).
//!
//! # Runs
//! As `Hooks::lower_operation` of [`super::legalize_dag::DagLegalizer`] for
//! the nodes the action table makes Custom (integer types only here; floats
//! are B1d).
//!
//! DEVIATE SD200: not translated yet, so the nodes are the plain forms: the
//! bit-test and arithmetic-flag folds (`LowerAndToBT`, `EmitTest` reusing an
//! `ADD`/`AND` as flags, the `SUB 0, x` and `XOR` forms of `EmitCmp`), the
//! `SETCC_CARRY` and `CTTZ`-select folds of `LowerSELECT`, overflow
//! intrinsics, and the 16-bit immediate widening in `EmitCmp`. Reason: each
//! needs known-bits or use queries the DAG lacks. Exit: with the combiner
//! (B2). Owner sdag-b. Review 2026-11-13.
//!
//! # See also
//! LLVM's `LowerSETCC`
//! ([L25293](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L25293)),
//! `LowerSELECT`
//! ([L25834](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L25834)),
//! `LowerConditionalBranch`
//! ([L26550](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L26550)),
//! `EmitCmp`
//! ([L24085](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L24085)).

use super::dag::{SDValue, SelectionDAG};
use super::isd::{CondCode, ISD};
use super::legalize_dag::Hooks;
use super::mvt::EVT;

/// The pin's `X86ISD` numbers, as the matcher table (`llrm-dagisel`) names them.
pub mod x86isd {
    use super::ISD;

    pub const ADD: ISD = ISD::Target(478);
    pub const AND: ISD = ISD::Target(485);
    pub const BRCOND: ISD = ISD::Target(493);
    pub const CMOV: ISD = ISD::Target(507);
    pub const CMP: ISD = ISD::Target(508);
    pub const OR: ISD = ISD::Target(697);
    pub const SETCC: ISD = ISD::Target(739);
    pub const SUB: ISD = ISD::Target(762);
    pub const XOR: ISD = ISD::Target(955);
}

/// `X86::CondCode`.
pub mod cond {
    pub const B: u128 = 2;
    pub const AE: u128 = 3;
    pub const E: u128 = 4;
    pub const NE: u128 = 5;
    pub const BE: u128 = 6;
    pub const A: u128 = 7;
    pub const S: u128 = 8;
    pub const NS: u128 = 9;
    pub const L: u128 = 12;
    pub const GE: u128 = 13;
    pub const LE: u128 = 14;
    pub const G: u128 = 15;
}

/// The custom lowerings, for a CPU with or without CMOV.
pub struct X86Lowering {
    pub cmov: bool,
}

impl Hooks for X86Lowering {
    fn lower_operation(&mut self, dag: &mut SelectionDAG, node: SDValue) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::SetCC => lower_setcc(dag, node),
            ISD::Select => lower_select(dag, node, self.cmov),
            ISD::BrCond => lower_brcond(dag, node),
            _ => None,
        }
    }
}

fn cond_code(dag: &mut SelectionDAG, code: u128) -> SDValue {
    dag.get_constant_with(code, EVT::I8, true, false)
}

fn cond_of(dag: &SelectionDAG, setcc: SDValue) -> CondCode {
    match dag.node(setcc).data {
        super::dag::NodeData::CondCode(cc) => cc,
        other => unreachable!("a condition code operand, not {other:?}"),
    }
}

/// `TranslateX86CC` for integers: the code, and a right-hand side that may
/// have been replaced (`x > -1` is `NS` on `x`).
fn translate_cc(dag: &mut SelectionDAG, cc: CondCode, rhs: &mut SDValue) -> u128 {
    if let Some(bits) = dag.constant(*rhs) {
        let vt = dag.value_type(*rhs);
        let all_ones = bits == super::fold::mask(vt.size_in_bits());
        match cc {
            CondCode::SetGt if all_ones => {
                *rhs = dag.get_constant(0, vt);
                return cond::NS;
            }
            CondCode::SetLt if bits == 0 => return cond::S,
            CondCode::SetGe if bits == 0 => return cond::NS,
            CondCode::SetLt if bits == 1 => {
                *rhs = dag.get_constant(0, vt);
                return cond::LE;
            }
            _ => {}
        }
    }
    match cc {
        CondCode::SetEq => cond::E,
        CondCode::SetGt => cond::G,
        CondCode::SetGe => cond::GE,
        CondCode::SetLt => cond::L,
        CondCode::SetLe => cond::LE,
        CondCode::SetNe => cond::NE,
        CondCode::SetULt => cond::B,
        CondCode::SetUGt => cond::A,
        CondCode::SetULe => cond::BE,
        CondCode::SetUGe => cond::AE,
        other => unreachable!("invalid integer condition {other:?}"),
    }
}

/// `EmitTest`, without the arithmetic-flag reuse (SD35): `cmp x, 0`.
fn emit_test(dag: &mut SelectionDAG, op: SDValue) -> SDValue {
    let zero = dag.get_constant(0, dag.value_type(op));
    dag.get_node(x86isd::CMP, EVT::I32, &[op, zero])
}

/// `EmitCmp`: the flags of `op0 - op1`.
fn emit_cmp(dag: &mut SelectionDAG, op0: SDValue, op1: SDValue) -> SDValue {
    if dag.is_null_constant(op1) {
        return emit_test(dag, op0);
    }
    let vt = dag.value_type(op0);
    let sub = dag.get_node_multi(x86isd::SUB, &[vt, EVT::I32], &[op0, op1]);
    SDValue { node: sub.node, res: 1 }
}

/// `emitFlagsForSetcc`: the flags and the condition-code constant.
fn emit_flags_for_setcc(dag: &mut SelectionDAG, op0: SDValue, op1: SDValue, cc: CondCode) -> (SDValue, SDValue) {
    if matches!(cc, CondCode::SetEq | CondCode::SetNe) && (dag.is_null_constant(op1) || dag.constant(op1) == Some(1)) && dag.opcode(op0) == x86isd::SETCC {
        // A test of a `setcc` is the `setcc`'s own flags, inverted if needed.
        let invert = (cc == CondCode::SetNe) ^ dag.is_null_constant(op1);
        let code = dag.constant(dag.operand(op0, 0)).expect("a SETCC reads a condition code");
        let code = if invert { code ^ 1 } else { code };
        return (dag.operand(op0, 1), cond_code(dag, code));
    }
    let mut rhs = op1;
    let code = translate_cc(dag, cc, &mut rhs);
    let flags = emit_cmp(dag, op0, rhs);
    (flags, cond_code(dag, code))
}

/// `LowerSETCC`, integers.
fn lower_setcc(dag: &mut SelectionDAG, node: SDValue) -> Option<SDValue> {
    let (op0, mut op1) = (dag.operand(node, 0), dag.operand(node, 1));
    if !dag.value_type(op0).is_integer() {
        return None;
    }
    let mut cc = cond_of(dag, dag.operand(node, 2));
    if let Some(bits) = dag.constant(op1).filter(|&bits| bits != 0) {
        let vt = dag.value_type(op1);
        let width = vt.size_in_bits();
        let max_signed = (1u128 << (width - 1)) - 1;
        let max_unsigned = super::fold::mask(width);
        let widens = (cc == CondCode::SetGt && bits != max_signed) || (cc == CondCode::SetUGt && bits != max_unsigned);
        // The constant plus one must be a 32-bit signed immediate, and a byte
        // one if the original was.
        let plus_one = (bits + 1) & max_unsigned;
        let signed = |value: u128, bits_wide: u32| {
            let extended = ((value << (128 - width)) as i128) >> (128 - width);
            extended >= -(1i128 << (bits_wide - 1)) && extended < (1i128 << (bits_wide - 1))
        };
        if widens && signed(plus_one, 32.min(width)) && (!signed(bits, 8.min(width)) || signed(plus_one, 8.min(width))) {
            op1 = dag.get_constant(plus_one, vt);
            cc = if cc == CondCode::SetGt { CondCode::SetGe } else { CondCode::SetUGe };
        }
    }
    let (flags, code) = emit_flags_for_setcc(dag, op0, op1, cc);
    Some(dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]))
}

/// `isX86LogicalCmp`: flags a `CMOV` can read directly.
fn is_logical_cmp(dag: &SelectionDAG, flags: SDValue) -> bool {
    let op = dag.opcode(flags);
    op == x86isd::CMP || (flags.res == 1 && [x86isd::ADD, x86isd::SUB, x86isd::OR, x86isd::XOR, x86isd::AND].contains(&op))
}

/// `X86::mayFoldLoad`: a plain load nothing else reads.
fn may_fold_load(dag: &SelectionDAG, value: SDValue) -> bool {
    dag.opcode(value) == ISD::Load && dag.users(value).count() == 1
}

/// `LowerSELECT`, integers: `cmov` of a compare's flags.
fn lower_select(dag: &mut SelectionDAG, node: SDValue, can_cmov: bool) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if !vt.is_integer() {
        return None;
    }
    let (mut cond, op1, op2) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    if dag.opcode(cond) == ISD::SetCC {
        if let Some(lowered) = lower_setcc(dag, cond) {
            cond = lowered;
        }
    }
    let (code, flags);
    if dag.opcode(cond) == x86isd::SETCC && is_logical_cmp(dag, dag.operand(cond, 1)) {
        code = dag.operand(cond, 0);
        flags = dag.operand(cond, 1);
    } else {
        code = cond_code(dag, cond::NE);
        flags = emit_test(dag, cond);
    }
    let (op1, op2, result_vt) = if (vt == EVT::I8 && can_cmov) || (vt == EVT::I16 && !may_fold_load(dag, op1) && !may_fold_load(dag, op2)) {
        (dag.get_node(ISD::AnyExtend, EVT::I32, &[op1]), dag.get_node(ISD::AnyExtend, EVT::I32, &[op2]), EVT::I32)
    } else {
        (op1, op2, vt)
    };
    let cmov = dag.get_node(x86isd::CMOV, result_vt, &[op2, op1, code, flags]);
    Some(if result_vt == vt { cmov } else { dag.get_node(ISD::Truncate, vt, &[cmov]) })
}

/// `LowerConditionalBranch`, integers: `X86ISD::BRCOND chain, dest, cc, flags`.
fn lower_brcond(dag: &mut SelectionDAG, node: SDValue) -> Option<SDValue> {
    let (chain, cond, dest) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let (code, flags) = if dag.opcode(cond) == ISD::SetCC && dag.value_type(dag.operand(cond, 0)).is_integer() {
        let (lhs, rhs) = (dag.operand(cond, 0), dag.operand(cond, 1));
        let cc = cond_of(dag, dag.operand(cond, 2));
        let (flags, code) = emit_flags_for_setcc(dag, lhs, rhs, cc);
        (code, flags)
    } else if dag.opcode(cond) == ISD::SetCC {
        return None;
    } else {
        // Anything else is tested: `and cond, 1`, then `!= 0`.
        let vt = dag.value_type(cond);
        let one = dag.get_constant(1, vt);
        let masked = if dag.opcode(cond) == ISD::And && dag.constant(dag.operand(cond, 1)) == Some(1) { cond } else { dag.get_node(ISD::And, vt, &[cond, one]) };
        let zero = dag.get_constant(0, vt);
        let (flags, code) = emit_flags_for_setcc(dag, masked, zero, CondCode::SetNe);
        (code, flags)
    };
    Some(dag.get_node(x86isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags]))
}
