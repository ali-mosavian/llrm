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
//! are B1d). A global address is the static, non-PIC form (`Wrapper` of the
//! target address); the GOT, PIC, DLL-import and TLS forms are not here, and
//! the action table makes it Custom at `i32` only (m16's `i16` is B0e's).
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

use super::dag::{MemBase, MemInfo, NodeData, SDNodeFlags, SDValue, SelectionDAG};
use super::dag_combiner::{CombineHooks, CombineLevel};
use super::isd::{CondCode, ISD};
use super::legalize_dag::Hooks;
use super::mvt::{EVT, MVT};

/// The pin's `X86ISD` numbers, as the matcher table (`llrm-dagisel`) names
/// them.
pub mod x86isd {
    use super::ISD;

    pub const ADC: ISD = ISD::Target(477);
    pub const ADD: ISD = ISD::Target(478);
    pub const AND: ISD = ISD::Target(485);
    pub const BRCOND: ISD = ISD::Target(493);
    pub const BSF: ISD = ISD::Target(495);
    pub const BSR: ISD = ISD::Target(496);
    pub const CMOV: ISD = ISD::Target(507);
    pub const CMP: ISD = ISD::Target(508);
    /// `X86ISD::MUL_IMM`: a multiply by 3, 5 or 9 that is one `lea`.
    pub const MUL_IMM: ISD = ISD::Target(694);
    /// Real mode's `les`/`lds` (`tools/dagisel/own_ids.txt`): the offset, a
    /// chain; the segment is read back from `ES`/`DS`.
    pub const LES: ISD = ISD::Target(24576);
    pub const LDS: ISD = ISD::Target(24577);
    /// A huge pointer plus an `i32` byte delta (a GEP the builder cannot
    /// prove stays in its segment): `(far, i32) -> far`. Expanded by the
    /// type legalizer, never selected.
    pub const HUGE_ADD: ISD = ISD::Target(24590);
    /// `llrm.ia16.window.p1.p3` as a node: a huge pointer with whole selector
    /// steps of its offset moved into the selector, `far -> far`. Expanded by
    /// the type legalizer, never selected (24578..24580 are the asm
    /// pass-through nodes).
    pub const LLRM_WINDOW: ISD = ISD::Target(24581);
    pub const OR: ISD = ISD::Target(697);
    pub const SETCC: ISD = ISD::Target(739);
    pub const SBB: ISD = ISD::Target(728);
    pub const SUB: ISD = ISD::Target(762);
    pub const WRAPPER: ISD = ISD::Target(953);
    /// `X86ISD::FP_TO_INT_IN_MEM`: `fist` of an x87 value to a stack slot.
    pub const FP_TO_INT_IN_MEM: ISD = ISD::Target(617);
    /// `X86ISD::FILD`: `fild` of an integer in memory to an x87 value.
    pub const FILD: ISD = ISD::Target(586);
    /// `X86ISD::FCMP`: the flags of an x87 compare (`fucom`, `fnstsw`,
    /// `sahf` on a 486).
    pub const FCMP: ISD = ISD::Target(576);
    pub const XOR: ISD = ISD::Target(955);
}

/// `X86::ES`, the segment `les` loads.
pub const ES: u32 = 31;

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
    pub const P: u128 = 10;
    pub const NP: u128 = 11;
    pub const L: u128 = 12;
    pub const GE: u128 = 13;
    pub const LE: u128 = 14;
    pub const G: u128 = 15;
}

/// The custom lowerings, for a CPU with or without CMOV.
pub struct X86Lowering {
    pub cmov: bool,
}

impl CombineHooks for X86Lowering {
    fn has_target_combine(
        &self,
        op: ISD,
    ) -> bool {
        op == ISD::Mul
    }

    fn perform_dag_combine(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
        level: CombineLevel,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::Mul => combine_mul(dag, node, level),
            _ => None,
        }
    }
}

impl Hooks for X86Lowering {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::SetCC => lower_setcc(dag, node),
            ISD::Select => lower_select(dag, node, self.cmov),
            ISD::BrCond => lower_brcond(dag, node),
            ISD::UAddO | ISD::USubO => Some(lower_xalu(dag, node)),
            ISD::UAddOCarry | ISD::USubOCarry => Some(lower_addsub_carry(dag, node)),
            ISD::GlobalAddress => lower_global_address(dag, node),
            ISD::VAStart => lower_vastart(dag, node),
            ISD::FpToSInt | ISD::FpToUInt => lower_fp_to_int(dag, node),
            ISD::SIntToFp | ISD::UIntToFp => lower_int_to_fp(dag, node),
            ISD::ConstantPool => lower_constant_pool(dag, node),
            ISD::ShlParts | ISD::SraParts | ISD::SrlParts => Some(lower_shift_parts(dag, node)),
            ISD::Abs => Some(lower_abs(dag, node)),
            ISD::CtLz | ISD::CtLzZeroPoison => Some(lower_ctlz(dag, node)),
            ISD::CtTz => Some(lower_cttz(dag, node)),
            ISD::Parity => Some(lower_parity(dag, node)),
            ISD::CtPop => lower_ctpop(dag, node),
            _ => None,
        }
    }
}

fn cond_code(
    dag: &mut SelectionDAG,
    code: u128,
) -> SDValue {
    dag.get_constant_with(code, EVT::I8, true, false)
}

fn cond_of(
    dag: &SelectionDAG,
    setcc: SDValue,
) -> CondCode {
    match dag.node(setcc).data {
        super::dag::NodeData::CondCode(cc) => cc,
        other => unreachable!("a condition code operand, not {other:?}"),
    }
}

/// `TranslateX86CC` for integers: the code, and a right-hand side that may
/// have been replaced (`x > -1` is `NS` on `x`).
fn translate_cc(
    dag: &mut SelectionDAG,
    cc: CondCode,
    rhs: &mut SDValue,
) -> u128 {
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

/// `isNON_EXTLoad`.
fn is_non_ext_load(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    dag.opcode(value) == ISD::Load
        && matches!(
            dag.node(value).data,
            super::dag::NodeData::Mem(info) if info.ext.is_none()
        )
}

/// `TranslateX86CC` for floats: the code and the operands, swapped where the
/// flags want the other order. `None` for `oeq` and `une`, which the
/// legalizer has already expanded to two compares.
fn translate_fp_cc(
    dag: &SelectionDAG,
    mut cc: CondCode,
    lhs: &mut SDValue,
    rhs: &mut SDValue,
) -> Option<u128> {
    if is_non_ext_load(dag, *lhs) && !is_non_ext_load(dag, *rhs) {
        cc = cc.swapped_operands();
        std::mem::swap(lhs, rhs);
    }
    if matches!(
        cc,
        CondCode::SetOLt | CondCode::SetOLe | CondCode::SetUGt | CondCode::SetUGe
    ) {
        std::mem::swap(lhs, rhs);
    }
    // ZF PF CF: greater 0 0 0, less 0 0 1, equal 1 0 0, unordered 1 1 1.
    Some(match cc {
        CondCode::SetUEq | CondCode::SetEq => cond::E,
        CondCode::SetOLt | CondCode::SetOGt | CondCode::SetGt => cond::A,
        CondCode::SetOLe | CondCode::SetOGe | CondCode::SetGe => cond::AE,
        CondCode::SetUGt | CondCode::SetULt | CondCode::SetLt => cond::B,
        CondCode::SetUGe | CondCode::SetULe | CondCode::SetLe => cond::BE,
        CondCode::SetONe | CondCode::SetNe => cond::NE,
        CondCode::SetUo => cond::P,
        CondCode::SetO => cond::NP,
        _ => return None,
    })
}

/// A float compare's flags and condition code (`LowerSETCC`, floating point).
fn float_flags(
    dag: &mut SelectionDAG,
    setcc: SDValue,
) -> Option<(SDValue, SDValue)> {
    let (mut lhs, mut rhs) = (dag.operand(setcc, 0), dag.operand(setcc, 1));
    let code = translate_fp_cc(dag, cond_of(dag, dag.operand(setcc, 2)), &mut lhs, &mut rhs)?;
    let flags = dag.get_node(x86isd::FCMP, EVT::I32, &[lhs, rhs]);
    Some((flags, cond_code(dag, code)))
}

/// `EmitTest`, without the arithmetic-flag reuse (SD35): `cmp x, 0`.
fn emit_test(
    dag: &mut SelectionDAG,
    op: SDValue,
) -> SDValue {
    let zero = dag.get_constant(0, dag.value_type(op));
    dag.get_node(x86isd::CMP, EVT::I32, &[op, zero])
}

/// `EmitCmp`: the flags of `op0 - op1`.
fn emit_cmp(
    dag: &mut SelectionDAG,
    op0: SDValue,
    op1: SDValue,
) -> SDValue {
    if dag.is_null_constant(op1) {
        return emit_test(dag, op0);
    }
    let vt = dag.value_type(op0);
    let sub = dag.get_node_multi(x86isd::SUB, &[vt, EVT::I32], &[op0, op1]);
    SDValue { node: sub.node, res: 1 }
}

/// `emitFlagsForSetcc`: the flags and the condition-code constant.
fn emit_flags_for_setcc(
    dag: &mut SelectionDAG,
    op0: SDValue,
    op1: SDValue,
    cc: CondCode,
) -> (SDValue, SDValue) {
    if matches!(cc, CondCode::SetEq | CondCode::SetNe)
        && (dag.is_null_constant(op1) || dag.constant(op1) == Some(1))
        && dag.opcode(op0) == x86isd::SETCC
    {
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
fn lower_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (op0, mut op1) = (dag.operand(node, 0), dag.operand(node, 1));
    if dag.value_type(op0).is_floating_point() {
        let (flags, code) = float_flags(dag, node)?;
        return Some(dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]));
    }
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
        if widens && signed(plus_one, 32.min(width)) && (!signed(bits, 8.min(width)) || signed(plus_one, 8.min(width)))
        {
            op1 = dag.get_constant(plus_one, vt);
            cc = if cc == CondCode::SetGt { CondCode::SetGe } else { CondCode::SetUGe };
        }
    }
    let (flags, code) = emit_flags_for_setcc(dag, op0, op1, cc);
    Some(dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]))
}

/// `isX86LogicalCmp`: flags a `CMOV` can read directly.
fn is_logical_cmp(
    dag: &SelectionDAG,
    flags: SDValue,
) -> bool {
    let op = dag.opcode(flags);
    op == x86isd::CMP
        || op == x86isd::FCMP
        || (flags.res == 1 && [x86isd::ADD, x86isd::SUB, x86isd::OR, x86isd::XOR, x86isd::AND].contains(&op))
}

/// `X86::mayFoldLoad`: a plain load nothing else reads.
fn may_fold_load(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    dag.opcode(value) == ISD::Load && dag.users(value).count() == 1
}

/// `LowerSELECT`, integers: `cmov` of a compare's flags.
fn lower_select(
    dag: &mut SelectionDAG,
    node: SDValue,
    can_cmov: bool,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if !vt.is_integer() && !vt.is_floating_point() {
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
    let (op1, op2, result_vt) =
        if (vt == EVT::I8 && can_cmov) || (vt == EVT::I16 && !may_fold_load(dag, op1) && !may_fold_load(dag, op2)) {
            (dag.get_node(ISD::AnyExtend, EVT::I32, &[op1]), dag.get_node(ISD::AnyExtend, EVT::I32, &[op2]), EVT::I32)
        } else {
            (op1, op2, vt)
        };
    let cmov = dag.get_node(x86isd::CMOV, result_vt, &[op2, op1, code, flags]);
    Some(if result_vt == vt { cmov } else { dag.get_node(ISD::Truncate, vt, &[cmov]) })
}

/// `LowerConditionalBranch`, integers: `X86ISD::BRCOND chain, dest, cc, flags`.
fn lower_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (chain, cond, dest) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let (code, flags) = if dag.opcode(cond) == ISD::SetCC && dag.value_type(dag.operand(cond, 0)).is_integer() {
        let (lhs, rhs) = (dag.operand(cond, 0), dag.operand(cond, 1));
        let cc = cond_of(dag, dag.operand(cond, 2));
        let (flags, code) = emit_flags_for_setcc(dag, lhs, rhs, cc);
        (code, flags)
    } else if dag.opcode(cond) == ISD::SetCC {
        let (flags, code) = float_flags(dag, cond)?;
        (code, flags)
    } else {
        // Anything else is tested: `and cond, 1`, then `!= 0`.
        let vt = dag.value_type(cond);
        let one = dag.get_constant(1, vt);
        let masked = if dag.opcode(cond) == ISD::And && dag.constant(dag.operand(cond, 1)) == Some(1) {
            cond
        } else {
            dag.get_node(ISD::And, vt, &[cond, one])
        };
        let zero = dag.get_constant(0, vt);
        let (flags, code) = emit_flags_for_setcc(dag, masked, zero, CondCode::SetNe);
        (code, flags)
    };
    Some(dag.get_node(x86isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags]))
}

/// `LowerGlobalOrExternal` for a static, non-PIC global: `Wrapper
/// (TargetGlobalAddress sym+offset)`. A non-negative offset folds into the
/// symbol; a negative one is added after.
fn lower_global_address(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let NodeData::Global { symbol, offset } = dag.node(node).data else { return None };
    let vt = dag.value_type(node);
    let (folded, rest) = if offset >= 0 && offset <= i64::from(i32::MAX) { (offset, 0) } else { (0, offset) };
    let target = dag.intern(
        ISD::TargetGlobalAddress,
        &[vt],
        &[],
        SDNodeFlags::default(),
        NodeData::Global { symbol, offset: folded },
    );
    let wrapped = dag.get_node(x86isd::WRAPPER, vt, &[target]);
    Some(if rest == 0 {
        wrapped
    } else {
        let delta = dag.get_constant(rest as u128, vt);
        dag.get_node(ISD::Add, vt, &[wrapped, delta])
    })
}

/// `LowerConstantPool`: `Wrapper (TargetConstantPool)`.
fn lower_constant_pool(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let NodeData::PoolIndex(index) = dag.node(node).data else { return None };
    let vt = dag.value_type(node);
    let target = dag.intern(ISD::TargetConstantPool, &[vt], &[], SDNodeFlags::default(), NodeData::PoolIndex(index));
    Some(dag.get_node(x86isd::WRAPPER, vt, &[target]))
}

/// `LowerXALUO` for the unsigned add and subtract with overflow: the flags of
/// `X86ISD::ADD`/`SUB`, and the carry (`B`; `E` for an add of one) as a
/// `setcc`, merged with the sum.
fn lower_xalu(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let (opcode, code) = match dag.opcode(node) {
        ISD::UAddO => (x86isd::ADD, if dag.constant(rhs) == Some(1) { cond::E } else { cond::B }),
        _ => (x86isd::SUB, cond::B),
    };
    let value = dag.get_node_multi(opcode, &[vt, EVT::I32], &[lhs, rhs]);
    let flags = SDValue { node: value.node, res: 1 };
    let code = cond_code(dag, code);
    let overflow = dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]);
    dag.get_node_multi(ISD::MergeValues, &[vt, EVT::I8], &[value, overflow])
}

/// `LowerADDSUBO_CARRY`: the carry in is put into the flags (`carry + -1`),
/// then `ADC`/`SBB` add or subtract it; the carry out is `B` of its flags.
fn lower_addsub_carry(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (lhs, rhs, carry) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let vt = dag.value_type(node);
    let carry_vt = dag.value_type(carry);
    let ones = dag.get_all_ones_constant(carry_vt);
    let set_carry = dag.get_node_multi(x86isd::ADD, &[carry_vt, EVT::I32], &[carry, ones]);
    let flags_in = SDValue { node: set_carry.node, res: 1 };
    let opcode = if dag.opcode(node) == ISD::UAddOCarry { x86isd::ADC } else { x86isd::SBB };
    let sum = dag.get_node_multi(opcode, &[vt, EVT::I32], &[lhs, rhs, flags_in]);
    let flags = SDValue { node: sum.node, res: 1 };
    let code = cond_code(dag, cond::B);
    let carry_out = dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]);
    dag.get_node_multi(ISD::MergeValues, &[vt, EVT::I8], &[sum, carry_out])
}

/// `FP_TO_INTHelper` for the x87: `fist` of the value to a stack slot of the
/// result's size, then a load of it. An unsigned `i32` goes through an `i64`
/// slot; an unsigned `i64` subtracts 2^63 from a value at or above it and
/// flips the sign bit of the result back.
fn lower_fp_to_int(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let signed = dag.opcode(node) == ISD::FpToSInt;
    let result_vt = dag.value_type(node);
    let mut value = dag.operand(node, 0);
    let value_vt = dag.value_type(value);
    let EVT::Simple(float @ (MVT::F32 | MVT::F64 | MVT::F80)) = value_vt else { return None };
    let unsigned_fixup = !signed && result_vt == EVT::I64;
    let stored_vt = if !signed && result_vt != EVT::I64 { EVT::I64 } else { result_vt };
    let size = stored_vt.size_in_bits() / 8;
    let index = dag.create_stack_temporary(size, size);
    let slot = dag.get_frame_index(dag.config().pointer, index);
    let chain = dag.entry_node();
    let mut adjust = None;
    if unsigned_fixup {
        // 2^63, the first value that does not fit a signed `i64`.
        let threshold = match float {
            MVT::F32 => 0x5F00_0000u128,
            MVT::F64 => 0x43E0_0000_0000_0000,
            _ => (0x403Eu128 << 64) | (1u128 << 63),
        };
        let threshold = dag.get_constant_fp(threshold, value_vt);
        let at_least = dag.get_setcc(EVT::I8, value, threshold, CondCode::SetGe);
        let wide = dag.get_node(ISD::ZeroExtend, EVT::I64, &[at_least]);
        let sixty_three = dag.get_constant(63, EVT::I8);
        adjust = Some(dag.get_node(ISD::Shl, EVT::I64, &[wide, sixty_three]));
        let zero = dag.get_constant_fp(0, value_vt);
        let offset = dag.get_select(value_vt, at_least, threshold, zero);
        value = dag.get_node(ISD::FSub, value_vt, &[value, offset]);
    }
    let info =
        MemInfo { mem_vt: stored_vt, align: size, volatile: false, base: MemBase::Stack(index), ext: None, space: 0 };
    let fist = dag.intern(
        x86isd::FP_TO_INT_IN_MEM,
        &[EVT::OTHER],
        &[chain, value, slot],
        SDNodeFlags::default(),
        NodeData::Mem(info),
    );
    let load_info = MemInfo { mem_vt: result_vt, ..info };
    let mut result = dag.get_load(result_vt, fist, slot, load_info);
    if let Some(adjust) = adjust {
        result = dag.get_node(ISD::Xor, EVT::I64, &[result, adjust]);
    }
    Some(result)
}

/// `BuildFILD` without SSE: `fild` of the `src_vt` integer at `slot`.
fn build_fild(
    dag: &mut SelectionDAG,
    dst: EVT,
    src_vt: EVT,
    chain: SDValue,
    slot: SDValue,
    info: MemInfo,
) -> SDValue {
    let info = MemInfo { mem_vt: src_vt, ..info };
    dag.intern(x86isd::FILD, &[dst, EVT::OTHER], &[chain, slot], SDNodeFlags::default(), NodeData::Mem(info))
}

/// `LowerSINT_TO_FP` and `LowerUINT_TO_FP` for the x87: the integer stored to
/// a stack slot and `fild`ed. An unsigned `i32` is stored with a zero high
/// dword and `fild`ed as an `i64`; an unsigned `i64` is `fild`ed signed and
/// 2^64 is added when the sign was set (the constant pool's fudge).
fn lower_int_to_fp(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let signed = dag.opcode(node) == ISD::SIntToFp;
    let dst = dag.value_type(node);
    let src = dag.operand(node, 0);
    let src_vt = dag.value_type(src);
    if !matches!(dst, EVT::Simple(MVT::F32 | MVT::F64 | MVT::F80)) || !src_vt.is_integer() {
        return None;
    }
    let pointer = dag.config().pointer;
    let chain = dag.entry_node();
    let store_info = |index: i32, vt: EVT, size: u32| MemInfo {
        mem_vt: vt,
        align: size,
        volatile: false,
        base: MemBase::Stack(index),
        ext: None,
        space: 0,
    };
    if signed || (src_vt != EVT::I32 && src_vt != EVT::I64) {
        let size = src_vt.size_in_bits() / 8;
        let index = dag.create_stack_temporary(size, size);
        let slot = dag.get_frame_index(pointer, index);
        let info = store_info(index, src_vt, size);
        let stored = dag.get_store(chain, src, slot, info);
        return Some(build_fild(dag, dst, src_vt, stored, slot, info));
    }
    let index = dag.create_stack_temporary(8, 8);
    let slot = dag.get_frame_index(pointer, index);
    let info = store_info(index, EVT::I32, 8);
    if src_vt == EVT::I32 {
        let high_slot = {
            let four = dag.get_constant(4, pointer);
            dag.get_node(ISD::Add, pointer, &[slot, four])
        };
        let low = dag.get_store(chain, src, slot, info);
        let zero = dag.get_constant(0, EVT::I32);
        let high = dag.get_store(low, zero, high_slot, info);
        return Some(build_fild(dag, dst, EVT::I64, high, slot, info));
    }
    let info = store_info(index, EVT::I64, 8);
    let stored = dag.get_store(chain, src, slot, info);
    let fild = build_fild(dag, EVT::Simple(MVT::F80), EVT::I64, stored, slot, info);
    let zero = dag.get_constant(0, EVT::I64);
    let sign_set = dag.get_setcc(EVT::I8, src, zero, CondCode::SetLt);
    // The fudge: 0.0f at +0 and 2^64 as a float at +4 of one pool entry.
    let fudge = dag.get_constant_pool(pointer, 0x5F80_0000_0000_0000, EVT::I64, false);
    let (zero_offset, four_offset) = (dag.get_constant(0, pointer), dag.get_constant(4, pointer));
    let offset = dag.get_select(pointer, sign_set, four_offset, zero_offset);
    let fudge_ptr = dag.get_node(ISD::Add, pointer, &[fudge, offset]);
    let NodeData::PoolIndex(entry) = dag.node(fudge).data else { return None };
    let fudge_info = MemInfo {
        mem_vt: EVT::Simple(MVT::F32),
        align: 8,
        volatile: false,
        base: MemBase::ConstantPool(entry),
        ext: Some(super::actions::LoadExt::ExtLoad),
        space: 0,
    };
    let fudge_value = dag.get_load(EVT::Simple(MVT::F80), SDValue { node: fild.node, res: 1 }, fudge_ptr, fudge_info);
    let sum = dag.get_node(ISD::FAdd, EVT::Simple(MVT::F80), &[fild, fudge_value]);
    let rounded = if dst == EVT::Simple(MVT::F80) {
        sum
    } else {
        let flag = dag.get_constant_with(0, pointer, true, false);
        dag.get_node(ISD::FpRound, dst, &[sum, flag])
    };
    Some(rounded)
}

/// `LowerVASTART` for the 32-bit ABI: a `va_list` is a pointer, so the start
/// is a store of the first variadic argument's address through it.
fn lower_vastart(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let index = dag.var_args_frame_index?;
    let (chain, list) = (dag.operand(node, 0), dag.operand(node, 1));
    let pointer = dag.config().pointer;
    let first = dag.get_frame_index(pointer, index);
    let base = match dag.node(list).data {
        NodeData::FrameIndex(slot) => MemBase::Stack(slot),
        _ => MemBase::Unknown,
    };
    let info =
        MemInfo { mem_vt: pointer, align: pointer.size_in_bits() / 8, volatile: false, base, ext: None, space: 0 };
    Some(dag.get_store(chain, first, list, info))
}

fn is_pow2(value: u128) -> bool {
    value != 0 && value & (value - 1) == 0
}

fn log2(value: u128) -> u128 {
    u128::from(127 - value.leading_zeros())
}

/// `combineMulSpecial`: `x * c` for the constants that are a `lea` and a
/// shift or an add.
fn combine_mul_special(
    dag: &mut SelectionDAG,
    x: SDValue,
    amount: u128,
    vt: EVT,
) -> Option<SDValue> {
    let shl_add_or_sub = |dag: &mut SelectionDAG, mult: u128, shift: u128, add: bool| {
        let by = dag.get_constant(mult, vt);
        let product = dag.get_node(x86isd::MUL_IMM, vt, &[x, by]);
        let amount = dag.get_constant(shift, EVT::I8);
        let shifted = dag.get_node(ISD::Shl, vt, &[product, amount]);
        dag.get_node(if add { ISD::Add } else { ISD::Sub }, vt, &[shifted, x])
    };
    let mul_mul_add = |dag: &mut SelectionDAG, first: u128, second: u128| {
        let a = dag.get_constant(first, vt);
        let once = dag.get_node(x86isd::MUL_IMM, vt, &[x, a]);
        let b = dag.get_constant(second, vt);
        let twice = dag.get_node(x86isd::MUL_IMM, vt, &[once, b]);
        dag.get_node(ISD::Add, vt, &[twice, x])
    };
    match amount {
        11 => return Some(shl_add_or_sub(dag, 5, 1, true)),
        21 => return Some(shl_add_or_sub(dag, 5, 2, true)),
        41 => return Some(shl_add_or_sub(dag, 5, 3, true)),
        22 => {
            let inner = shl_add_or_sub(dag, 5, 2, true);
            return Some(dag.get_node(ISD::Add, vt, &[x, inner]));
        }
        19 => return Some(shl_add_or_sub(dag, 9, 1, true)),
        37 => return Some(shl_add_or_sub(dag, 9, 2, true)),
        73 => return Some(shl_add_or_sub(dag, 9, 3, true)),
        13 => return Some(shl_add_or_sub(dag, 3, 2, true)),
        23 => return Some(shl_add_or_sub(dag, 3, 3, false)),
        26 => return Some(mul_mul_add(dag, 5, 5)),
        28 => return Some(mul_mul_add(dag, 9, 3)),
        29 => {
            let inner = mul_mul_add(dag, 9, 3);
            return Some(dag.get_node(ISD::Add, vt, &[x, inner]));
        }
        _ => {}
    }
    // Two shifts added: `x * (2^a + 2^b)` with the lower one a scale.
    if is_pow2(amount & (amount - 1)) {
        let scale_shift = u128::from(amount.trailing_zeros());
        if (1..4).contains(&scale_shift) {
            let high = log2(amount & (amount - 1));
            let (ha, sa) = (dag.get_constant(high, EVT::I8), dag.get_constant(scale_shift, EVT::I8));
            let (first, second) = (dag.get_node(ISD::Shl, vt, &[x, ha]), dag.get_node(ISD::Shl, vt, &[x, sa]));
            return Some(dag.get_node(ISD::Add, vt, &[first, second]));
        }
    }
    None
}

/// `combineMul`, the scalar integer part: `x * c` as `lea`s, shifts, adds.
fn combine_mul(
    dag: &mut SelectionDAG,
    node: SDValue,
    level: CombineLevel,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if vt != EVT::I32 {
        return None;
    }
    let (x, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let bits = dag.constant(rhs)?;
    let width = vt.size_in_bits();
    if bits == 0 {
        return Some(dag.get_constant(0, vt));
    }
    if bits == super::fold::mask(width) {
        return Some(dag.get_negative(x, vt));
    }
    if is_pow2(bits) || level == CombineLevel::BeforeLegalizeTypes {
        return None;
    }
    let signed = i128::from(bits as u32 as i32);
    let abs = signed.unsigned_abs();
    let negate = |dag: &mut SelectionDAG, value: SDValue| if signed < 0 { dag.get_negative(value, vt) } else { value };
    let shift = |dag: &mut SelectionDAG, value: SDValue, by: u128| {
        let by = dag.get_constant(by, EVT::I8);
        dag.get_node(ISD::Shl, vt, &[value, by])
    };
    let mul_imm = |dag: &mut SelectionDAG, value: SDValue, by: u128| {
        let by = dag.get_constant(by, vt);
        dag.get_node(x86isd::MUL_IMM, vt, &[value, by])
    };
    if matches!(abs, 3 | 5 | 9) {
        let product = mul_imm(dag, x, abs);
        return Some(negate(dag, product));
    }
    let (mut first, mut second) = (0u128, 0u128);
    for divisor in [9u128, 5, 3] {
        if abs % divisor == 0 {
            first = divisor;
            second = abs / divisor;
            break;
        }
    }
    let mut new_mul;
    if second != 0 && (is_pow2(second) || (signed >= 0 && matches!(second, 3 | 5 | 9))) {
        let single_add_user = dag.users(node).count() == 1
            && dag.users(node).next().map(|user| dag.opcode(SDValue { node: user, res: 0 })) == Some(ISD::Add);
        if is_pow2(second) && !(signed >= 0 && single_add_user) {
            std::mem::swap(&mut first, &mut second);
        }
        let mut value = if is_pow2(first) { shift(dag, x, log2(first)) } else { mul_imm(dag, x, first) };
        value = if is_pow2(second) { shift(dag, value, log2(second)) } else { mul_imm(dag, value, second) };
        new_mul = Some(negate(dag, value));
    } else {
        new_mul = combine_mul_special(dag, x, bits & super::fold::mask(width), vt);
    }
    if new_mul.is_none() {
        if is_pow2(abs - 1) {
            let shifted = shift(dag, x, log2(abs - 1));
            let sum = dag.get_node(ISD::Add, vt, &[x, shifted]);
            new_mul = Some(negate(dag, sum));
        } else if is_pow2(abs + 1) {
            let shifted = shift(dag, x, log2(abs + 1));
            new_mul = Some(if signed < 0 {
                dag.get_node(ISD::Sub, vt, &[x, shifted])
            } else {
                dag.get_node(ISD::Sub, vt, &[shifted, x])
            });
        } else if signed >= 0 && abs > 2 && is_pow2(abs - 2) {
            let shifted = shift(dag, x, log2(abs - 2));
            let twice = dag.get_node(ISD::Add, vt, &[x, x]);
            new_mul = Some(dag.get_node(ISD::Add, vt, &[shifted, twice]));
        } else if signed >= 0 && is_pow2(abs + 2) {
            let shifted = shift(dag, x, log2(abs + 2));
            let twice = dag.get_node(ISD::Add, vt, &[x, x]);
            new_mul = Some(dag.get_node(ISD::Sub, vt, &[shifted, twice]));
        }
    }
    new_mul
}

/// `LowerShiftParts` (`TargetLowering::expandShiftParts`): a double-width
/// shift by a variable amount as two shifts, a funnel shift and selects on
/// bit 5 of the amount.
fn lower_shift_parts(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (lo, hi, amount) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let vt = dag.value_type(lo);
    let bits = u128::from(vt.size_in_bits());
    let amount_vt = dag.value_type(amount);
    let is_shl = dag.opcode(node) == ISD::ShlParts;
    let is_sra = dag.opcode(node) == ISD::SraParts;
    let mask = dag.get_constant(bits - 1, amount_vt);
    let safe = dag.get_node(ISD::And, amount_vt, &[amount, mask]);
    let tmp1 = if is_sra {
        let sign = dag.get_constant(bits - 1, amount_vt);
        dag.get_node(ISD::Sra, vt, &[hi, sign])
    } else {
        dag.get_constant(0, vt)
    };
    let (tmp2, tmp3) = if is_shl {
        (dag.get_node(ISD::FShl, vt, &[hi, lo, amount]), dag.get_node(ISD::Shl, vt, &[lo, safe]))
    } else {
        (
            dag.get_node(ISD::FShr, vt, &[hi, lo, amount]),
            dag.get_node(if is_sra { ISD::Sra } else { ISD::Srl }, vt, &[hi, safe]),
        )
    };
    let width = dag.get_constant(bits, amount_vt);
    let and = dag.get_node(ISD::And, amount_vt, &[amount, width]);
    let zero = dag.get_constant(0, amount_vt);
    let cond = dag.get_setcc(EVT::I8, and, zero, CondCode::SetNe);
    let (out_lo, out_hi) = if is_shl {
        (dag.get_select(vt, cond, tmp1, tmp3), dag.get_select(vt, cond, tmp3, tmp2))
    } else {
        (dag.get_select(vt, cond, tmp3, tmp2), dag.get_select(vt, cond, tmp1, tmp3))
    };
    dag.get_node_multi(ISD::MergeValues, &[vt, vt], &[out_lo, out_hi])
}

/// `isKnownNeverZero`, the part known bits answer: some bit is known set.
/// DEVIATE SD304: the pin also looks through `or`, `shl`, `zext`, `smax` and
/// the like; a value they prove non-zero costs one `cmov` more here. Reason:
/// no general query yet. Exit: with the combiner's `isKnownNeverZero`.
/// Owner sdag-c. Review 2026-12-10.
fn is_known_never_zero(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    super::known_bits::known_bits(dag, value).one != 0
}

/// `KnownBits::countMinLeadingZeros`.
fn min_leading_zeros(known: &super::known_bits::KnownBits) -> u32 {
    (!known.zero & super::fold::mask(known.width)).leading_zeros() - (128 - known.width)
}

/// `LowerABS` for `i16`/`i32`: `x < 0 ? -x : x` as `cmov` on the flags of `0 -
/// x`.
fn lower_abs(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let vt = dag.value_type(node);
    let x = dag.operand(node, 0);
    let zero = dag.get_constant(0, vt);
    let neg = dag.get_node_multi(x86isd::SUB, &[vt, EVT::I32], &[zero, x]);
    let flags = SDValue { node: neg.node, res: 1 };
    let code = cond_code(dag, cond::NS);
    dag.get_node(x86isd::CMOV, vt, &[x, neg, code, flags])
}

/// `LowerCTLZ` ([L29730][pin]): `bsr`, a `cmov` for a zero source, and
/// `xor bits-1`. No pass-through `bsr` (a tuning of newer CPUs).
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L29730
fn lower_ctlz(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let vt = dag.value_type(node);
    let bits = u128::from(vt.size_in_bits());
    let mut op = dag.operand(node, 0);
    let mut op_vt = vt;
    if vt == EVT::I8 {
        // There is no 8-bit bsr.
        op_vt = EVT::I32;
        op = dag.get_node(ISD::ZeroExtend, op_vt, &[op]);
    }
    let pass_through = dag.get_undef(op_vt);
    let scan = dag.get_node_multi(x86isd::BSR, &[op_vt, EVT::I32], &[pass_through, op]);
    let mut result = SDValue { node: scan.node, res: 0 };
    if dag.opcode(node) == ISD::CtLz {
        // A zero source sets ZF: the answer is `bits`, which the xor leaves.
        let zero_answer = dag.get_constant(bits + bits - 1, op_vt);
        let code = cond_code(dag, cond::E);
        let flags = SDValue { node: scan.node, res: 1 };
        result = dag.get_node(x86isd::CMOV, op_vt, &[result, zero_answer, code, flags]);
    }
    let last = dag.get_constant(bits - 1, op_vt);
    result = dag.get_node(ISD::Xor, op_vt, &[result, last]);
    if vt == EVT::I8 { dag.get_node(ISD::Truncate, EVT::I8, &[result]) } else { result }
}

/// `LowerCTTZ` ([L29776][pin]) for scalars: `bsf`, then a `cmov` of `bits`
/// for a zero source unless the source cannot be zero.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L29776
fn lower_cttz(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let vt = dag.value_type(node);
    let src = dag.operand(node, 0);
    let pass_through = dag.get_undef(vt);
    let scan = dag.get_node_multi(x86isd::BSF, &[vt, EVT::I32], &[pass_through, src]);
    if is_known_never_zero(dag, src) {
        return SDValue { node: scan.node, res: 0 };
    }
    let zero_answer = dag.get_constant(u128::from(vt.size_in_bits()), vt);
    let code = cond_code(dag, cond::E);
    let flags = SDValue { node: scan.node, res: 1 };
    dag.get_node(x86isd::CMOV, vt, &[SDValue { node: scan.node, res: 0 }, zero_answer, code, flags])
}

/// `LowerPARITY` ([L33977][pin]) without POPCNT: the parity flag of an
/// 8-bit `test`, or of the xor of the two low bytes after folding the rest.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L33977
fn lower_parity(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let vt = dag.value_type(node);
    let mut x = dag.operand(node, 0);
    let known = super::known_bits::known_bits(dag, x);
    let active = known.width - min_leading_zeros(&known);
    let not_parity = |dag: &mut SelectionDAG, flags: SDValue| {
        let code = cond_code(dag, cond::NP);
        let setnp = dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]);
        if vt == EVT::I8 { setnp } else { dag.get_node(ISD::ZeroExtend, vt, &[setnp]) }
    };
    if vt == EVT::I8 || active <= 8 {
        x = dag.get_zext_or_trunc(x, EVT::I8);
        let zero = dag.get_constant(0, EVT::I8);
        let flags = dag.get_node(x86isd::CMP, EVT::I32, &[x, zero]);
        return not_parity(dag, flags);
    }
    if vt != EVT::I16 {
        let sixteen = dag.get_constant(16, EVT::I8);
        let hi16 = dag.get_node(ISD::Srl, EVT::I32, &[x, sixteen]);
        x = dag.get_node(ISD::Xor, EVT::I32, &[x, hi16]);
    } else {
        x = dag.get_node(ISD::AnyExtend, EVT::I32, &[x]);
    }
    let eight = dag.get_constant(8, EVT::I8);
    let shifted = dag.get_node(ISD::Srl, EVT::I32, &[x, eight]);
    let hi = dag.get_node(ISD::Truncate, EVT::I8, &[shifted]);
    let lo = dag.get_node(ISD::Truncate, EVT::I8, &[x]);
    let xor = dag.get_node_multi(x86isd::XOR, &[EVT::I8, EVT::I32], &[lo, hi]);
    not_parity(dag, SDValue { node: xor.node, res: 1 })
}

/// `LowerCTPOP` ([L33649][pin]) for scalars: a constant source, or a value
/// with few active bits, gets a short sequence; anything else is `None`,
/// the generic expansion.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L33649
fn lower_ctpop(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let mut op = dag.operand(node, 0);
    let known = super::known_bits::known_bits(dag, op);
    if known.is_constant() {
        return Some(dag.get_constant(u128::from(known.one.count_ones()), vt));
    }
    let width = known.width;
    let leading = min_leading_zeros(&known);
    let trailing = known.zero.trailing_ones();
    let active = width - leading;
    let shifted_active = width - (leading + trailing);
    let i8_amount = |dag: &mut SelectionDAG, n: u32| dag.get_constant(u128::from(n), EVT::I8);
    let shift_down = |dag: &mut SelectionDAG, op: SDValue| {
        let amount = i8_amount(dag, trailing);
        dag.get_node(ISD::Srl, vt, &[op, amount])
    };
    if shifted_active <= 2 {
        if active > 2 {
            op = shift_down(dag, op);
        }
        op = dag.get_zext_or_trunc(op, EVT::I32);
        let one = i8_amount(dag, 1);
        let half = dag.get_node(ISD::Srl, EVT::I32, &[op, one]);
        op = dag.get_node(ISD::Sub, EVT::I32, &[op, half]);
        return Some(dag.get_zext_or_trunc(op, vt));
    }
    if shifted_active <= 3 {
        if active > 3 {
            op = shift_down(dag, op);
        }
        op = dag.get_zext_or_trunc(op, EVT::I32);
        let one = i8_amount(dag, 1);
        op = dag.get_node(ISD::Shl, EVT::I32, &[op, one]);
        let table = dag.get_constant(0b1110_1001_1001_0100, EVT::I32);
        op = dag.get_node(ISD::Srl, EVT::I32, &[table, op]);
        let three = dag.get_constant(3, EVT::I32);
        op = dag.get_node(ISD::And, EVT::I32, &[op, three]);
        return Some(dag.get_zext_or_trunc(op, vt));
    }
    // The 4-bit table is an `i64` lookup, and `i64` is not legal here.
    if shifted_active <= 8 {
        let mask11 = dag.get_constant(0x1111_1111, EVT::I32);
        if active > 8 {
            op = shift_down(dag, op);
        }
        op = dag.get_zext_or_trunc(op, EVT::I32);
        let magic = dag.get_constant(0x0804_0201, EVT::I32);
        op = dag.get_node(ISD::Mul, EVT::I32, &[op, magic]);
        let three = i8_amount(dag, 3);
        op = dag.get_node(ISD::Srl, EVT::I32, &[op, three]);
        op = dag.get_node(ISD::And, EVT::I32, &[op, mask11]);
        op = dag.get_node(ISD::Mul, EVT::I32, &[op, mask11]);
        let top = i8_amount(dag, 28);
        op = dag.get_node(ISD::Srl, EVT::I32, &[op, top]);
        return Some(dag.get_zext_or_trunc(op, vt));
    }
    None
}
