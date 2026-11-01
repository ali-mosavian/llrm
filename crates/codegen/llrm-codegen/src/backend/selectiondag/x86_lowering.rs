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
//! reassociated and immediate forms of `LowerAndToBT`, the `SUB 0, x` and `XOR`
//! forms of `EmitCmp` (`EmitTest` does reuse an arithmetic result's flags), the
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

use super::actions::TargetLowering;
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
    pub const FSHL: ISD = ISD::Target(625);
    pub const FSHR: ISD = ISD::Target(626);
    pub const SMUL: ISD = ISD::Target(744);
    pub const UMUL: ISD = ISD::Target(776);
    pub const ADD: ISD = ISD::Target(478);
    pub const AND: ISD = ISD::Target(485);
    pub const BRCOND: ISD = ISD::Target(493);
    /// `X86ISD::BT`: the carry flag is bit `n` of `x`.
    pub const BT: ISD = ISD::Target(497);
    /// `X86ISD::SETCC_CARRY`: `-1` if the condition holds in the carry flag
    /// (`sbb r, r`), else `0`.
    pub const SETCC_CARRY: ISD = ISD::Target(740);
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
    /// `llrm.ia16.ptrdiff`: the bytes between two carried huge pointers,
    /// `(far, far) -> i32`. Expanded by the type legalizer, never selected.
    pub const LLRM_PTRDIFF: ISD = ISD::Target(24583);
    pub const OR: ISD = ISD::Target(697);
    pub const SETCC: ISD = ISD::Target(739);
    pub const SBB: ISD = ISD::Target(728);
    pub const SUB: ISD = ISD::Target(762);
    pub const WRAPPER: ISD = ISD::Target(953);
    /// `X86ISD::WrapperRIP`: a symbol addressed from `%rip`.
    pub const WRAPPER_RIP: ISD = ISD::Target(954);
    /// `X86ISD::FP_TO_INT_IN_MEM`: `fist` of an x87 value to a stack slot.
    pub const FP_TO_INT_IN_MEM: ISD = ISD::Target(617);
    /// `X86ISD::FILD`: `fild` of an integer in memory to an x87 value.
    pub const FILD: ISD = ISD::Target(586);
    /// `X86ISD::FIST`: `fist` of an x87 value in the current rounding mode.
    pub const FIST: ISD = ISD::Target(587);
    /// `X86ISD::FCMP`: the flags of an x87 compare (`fucom`, `fnstsw`,
    /// `sahf` on a 486).
    pub const FCMP: ISD = ISD::Target(576);
    pub const XOR: ISD = ISD::Target(955);
    /// `X86ISD::FAND`/`FOR`/`FXOR`: `andps`/`orps`/`xorps` on a scalar float
    /// and a 16-byte pool mask.
    pub const FAND: ISD = ISD::Target(574);
    pub const FOR: ISD = ISD::Target(615);
    pub const FXOR: ISD = ISD::Target(634);
}

/// `X86::ES`, the segment `les` loads.
pub const ES: u32 = 31;

/// `X86::CondCode`.
pub mod cond {
    pub const O: u128 = 0;
    pub const NO: u128 = 1;
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
pub struct X86Lowering<'a> {
    pub cmov: bool,
    /// Scalar floats live in xmm registers (`isScalarFPTypeInSSEReg`).
    pub sse2: bool,
    /// `hasMinSize`: llrm's `-Os` as well as `-Oz`.
    pub min_size: bool,
    /// The pin chains an `i16` multiply by a constant: it promotes it to `i32`
    /// ([L64277][promote]) and `combineMul` builds the `lea`/shift chain there.
    /// Where `i16` is not promoted (m16, `is_desirable_to_promote_op`) the same
    /// chain is built at `i16`, without the promotion's `66h`/`67h` prefixes.
    /// Consequence of DEVIATE SD441, which holds the exit.
    ///
    /// [promote]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L64277
    pub i16_mul_chains: bool,
    /// What prices a constant multiply that `combineMul` leaves an `imul`
    /// (DEVIATE SD446); none keeps the pin's `imul`.
    pub profile: Option<MulPricing<'a>>,
}

/// The sheet, the target and the block a constant multiply is priced against.
#[derive(Clone, Copy)]
pub struct MulPricing<'a> {
    pub cpu: &'a crate::backend::cpu::Profile,
    pub arch: &'a dyn llrm_target::Target,
    /// The block's frequency over the entry's.
    pub freq: f64,
    /// `Tti::speed_milliclocks_per_byte`.
    pub milliclocks_per_byte: i64,
    /// `Tuning::multiply_by_clocks`: the fewest clocks, no byte term.
    pub by_clocks: bool,
}

impl CombineHooks for X86Lowering<'_> {
    fn pack_word_pairs(&self) -> bool {
        true
    }

    fn is_compare_with_flags(
        &self,
        user: ISD,
    ) -> bool {
        user == x86isd::CMP
    }

    /// DEVIATE G1 (x87 only: with SSE the conversion is in registers): the
    /// `fist` to a scratch slot, the load of the slot and the store of it
    /// become one `fist` to the store's address.
    ///
    /// Taken only when the slot is the pair's alone (the `fist` is read by
    /// that load, the slot node by those two), the load's value is the store's
    /// only input, all three are plain accesses of one width, and the
    /// conversion does not widen (an unsigned `i32` goes through an `i64` slot
    /// and is left). The new `fist` takes the store's chain, so it runs where
    /// the store did and orders against every access as the store did.
    /// O(1) per store.
    fn store_of_converted_slot(
        &self,
        dag: &mut SelectionDAG,
        store: SDValue,
    ) -> Option<SDValue> {
        if self.sse2 {
            return None;
        }
        let (chain, value, address) = (dag.operand(store, 0), dag.operand(store, 1), dag.operand(store, 2));
        let NodeData::Mem(stored) = dag.node(store).data else { return None };
        let plain = |info: &MemInfo| !info.volatile && info.ordering.is_none() && info.ext.is_none();
        if dag.opcode(value) != ISD::Load || !dag.has_one_use(value.node) || !plain(&stored) {
            return None;
        }
        let NodeData::Mem(loaded) = dag.node(value).data else { return None };
        if !plain(&loaded) || loaded.mem_vt != stored.mem_vt || dag.value_type(value) != stored.mem_vt {
            return None;
        }
        let fist = dag.operand(value, 0);
        if fist.res != 0 || !matches!(dag.opcode(fist), x86isd::FIST | x86isd::FP_TO_INT_IN_MEM) {
            return None;
        }
        let NodeData::Mem(converted) = dag.node(fist).data else { return None };
        let slot = dag.operand(fist, 2);
        if !plain(&converted)
            || converted.mem_vt != loaded.mem_vt
            || dag.opcode(slot) != ISD::FrameIndex
            || dag.operand(value, 1) != slot
            || !dag.has_one_use(fist.node)
            || dag.use_count_up_to(slot.node, 3) != 2
        {
            return None;
        }
        let info = MemInfo { align: stored.align, base: stored.base, space: stored.space, ..converted };
        let opcode = dag.opcode(fist);
        let converted_value = dag.operand(fist, 1);
        // Chained where the store was, so every access that came before it in
        // the program still comes before the `fist`, whatever it aliases.
        Some(dag.intern(
            opcode,
            &[EVT::OTHER],
            &[chain, converted_value, address],
            SDNodeFlags::default(),
            NodeData::Mem(info),
        ))
    }

    fn has_target_combine(
        &self,
        op: ISD,
    ) -> bool {
        op == ISD::Mul
            || op == ISD::Add
            || op == ISD::Sub
            || op == x86isd::ADD
            || op == x86isd::SUB
            || op == x86isd::CMOV
            || op == x86isd::SETCC
            || op == x86isd::BRCOND
            || op == x86isd::ADC
            || op == x86isd::SBB
            || op == ISD::UIntToFp
    }

    fn perform_dag_combine(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
        level: CombineLevel,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::Mul => combine_mul(dag, node, level, self.min_size, self.i16_mul_chains, self.profile),
            ISD::Add => combine_add(dag, node),
            ISD::Sub => combine_sub(dag, node),
            x86isd::ADD | x86isd::SUB => combine_x86_add_sub(dag, node),
            x86isd::CMOV => combine_cmov(dag, node),
            x86isd::SETCC => combine_setcc(dag, node),
            x86isd::BRCOND => combine_brcond(dag, node),
            x86isd::ADC | x86isd::SBB => combine_adc_sbb(dag, node),
            ISD::UIntToFp => combine_uint_to_fp(dag, node),
            _ => None,
        }
    }

    fn target_shrink_demanded_constant(
        &mut self,
        dag: &mut SelectionDAG,
        op: SDValue,
        demanded: u128,
    ) -> Option<Option<SDValue>> {
        x86_shrink_demanded_constant(dag, op, demanded)
    }

    fn promote_type(
        &self,
        dag: &SelectionDAG,
        tli: &TargetLowering,
        op: SDValue,
    ) -> Option<EVT> {
        is_desirable_to_promote_op(dag, tli, op)
    }
}

/// `X86TargetLowering::targetShrinkDemandedConstant`, scalar part: an `and`
/// mask is not shrunk below what a `movzx` matches. The mask is rounded up to
/// a byte, then a power of two; if that is the mask, it stays; if it only
/// differs in undemanded bits, it becomes that.
fn x86_shrink_demanded_constant(
    dag: &mut SelectionDAG,
    op: SDValue,
    demanded: u128,
) -> Option<Option<SDValue>> {
    if dag.opcode(op) != ISD::And {
        return None;
    }
    let vt = dag.value_type(op);
    let mask = dag.constant(dag.operand(op, 1))?;
    let shrunk = mask & demanded;
    let active = 128 - shrunk.leading_zeros();
    if active == 0 {
        return None;
    }
    let width = active.max(8).next_power_of_two().min(vt.size_in_bits());
    let zero_extend = super::fold::mask(width);
    if zero_extend == mask {
        return Some(None);
    }
    if zero_extend & !(mask | !demanded) != 0 {
        return None;
    }
    let new_mask = dag.get_constant(zero_extend, vt);
    let x = dag.operand(op, 0);
    Some(Some(dag.get_node(ISD::And, vt, &[x, new_mask])))
}

/// Whether this target promotes `i16` (and `i8` multiply) operations to `i32`
/// (SD441: not at m16). `is_desirable_to_promote_op` and the `i16` multiply
/// chain both ask this, so the chain is built exactly where nothing promotes.
pub fn promotes_i16(tli: &TargetLowering) -> bool {
    !tli.types.has_far_pointer()
}

/// `isTypeDesirableForOp` (undesirable) and `IsDesirableToPromoteOp`: an `i16`
/// operation, or an `i8` multiply by a constant, is done in `i32` unless an
/// operand is a load the operation could fold.
///
/// DEVIATE SD441: not at m16. The pin promotes there too; in real mode each
/// promoted op carries a `66h` prefix (a `lea` of it `67h` as well), which
/// grows every `i16` op and breaks the m16 -Os rule. Exit: promotion priced
/// against those prefix bytes and clocks. Until then `i16_mul_chains` builds
/// the multiply's chain at `i16`.
fn is_desirable_to_promote_op(
    dag: &SelectionDAG,
    tli: &TargetLowering,
    op: SDValue,
) -> Option<EVT> {
    let vt = dag.value_type(op);
    let opcode = dag.opcode(op);
    if !promotes_i16(tli) || !tli.types.is_type_legal(vt) {
        return None;
    }
    let is8_mul_by_constant = vt == EVT::I8 && opcode == ISD::Mul && dag.constant(dag.operand(op, 1)).is_some();
    let undesirable = match vt {
        EVT::I8 => matches!(opcode, ISD::Mul | ISD::Shl),
        EVT::I16 => matches!(
            opcode,
            ISD::SignExtend
                | ISD::ZeroExtend
                | ISD::AnyExtend
                | ISD::Mul
                | ISD::Shl
                | ISD::Sra
                | ISD::Srl
                | ISD::Sub
                | ISD::Add
                | ISD::And
                | ISD::Or
                | ISD::Xor
        ),
        _ => false,
    };
    if !undesirable || (vt != EVT::I16 && !is8_mul_by_constant) {
        return None;
    }
    // `X86::mayFoldLoad`: a single-use plain load.
    let may_fold_load = |v: SDValue| {
        dag.opcode(v) == ISD::Load
            && dag.users(v).count() == 1
            && matches!(dag.node(v).data, NodeData::Mem(info) if info.ext.is_none())
    };
    // `IsFoldableRMW`: the only user is a store to the load's address.
    let foldable_rmw = |load: SDValue| {
        let mut users = dag.users(op);
        let (Some(user), None) = (users.next(), users.next()) else { return false };
        let user = SDValue { node: user, res: 0 };
        dag.opcode(user) == ISD::Store && dag.operand(user, 2) == dag.operand(load, 1)
    };
    let (n0, n1) =
        (dag.operand(op, 0), if dag.node(op).ops.len() > 1 { dag.operand(op, 1) } else { dag.operand(op, 0) });
    match opcode {
        ISD::SignExtend | ISD::ZeroExtend | ISD::AnyExtend => {}
        ISD::Shl | ISD::Sra | ISD::Srl => {
            if may_fold_load(n0) && foldable_rmw(n0) {
                return None;
            }
        }
        ISD::Add | ISD::And | ISD::Or | ISD::Xor | ISD::Mul | ISD::Sub => {
            let commute = opcode != ISD::Sub;
            let is_const = |v: SDValue| dag.constant(v).is_some();
            if may_fold_load(n1) && (!commute || !is_const(n0) || (opcode != ISD::Mul && foldable_rmw(n1))) {
                return None;
            }
            if may_fold_load(n0) && ((commute && !is_const(n1)) || (opcode != ISD::Mul && foldable_rmw(n0))) {
                return None;
            }
        }
        _ => return None,
    }
    Some(EVT::I32)
}

impl Hooks for X86Lowering<'_> {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::SetCC => lower_setcc(dag, node),
            ISD::Select => lower_select(dag, node, self.cmov),
            ISD::BrCond => lower_brcond(dag, node),
            ISD::UAddO | ISD::USubO | ISD::SAddO | ISD::SSubO | ISD::UMulO | ISD::SMulO => Some(lower_xalu(dag, node)),
            ISD::UAddOCarry | ISD::USubOCarry => Some(lower_addsub_carry(dag, node)),
            ISD::SetCCCarry => Some(lower_setcc_carry(dag, node)),
            ISD::GlobalAddress => lower_global_address(dag, node),
            ISD::VAStart => lower_vastart(dag, node),
            ISD::FpToSInt | ISD::FpToUInt => lower_fp_to_int(dag, node, self.sse2),
            ISD::LRint => lower_lrint(dag, node),
            ISD::SIntToFp | ISD::UIntToFp => lower_int_to_fp(dag, node, self.sse2),
            ISD::ConstantPool => lower_constant_pool(dag, node),
            ISD::JumpTable => lower_jump_table(dag, node),
            ISD::ShlParts | ISD::SraParts | ISD::SrlParts => Some(lower_shift_parts(dag, node)),
            ISD::Abs => Some(lower_abs(dag, node)),
            ISD::SSubSat => Some(lower_ssubsat(dag, node)),
            ISD::FShl | ISD::FShr => lower_funnel_shift(dag, node),
            ISD::FNeg | ISD::FAbs => lower_fabs_or_fneg(dag, node),
            ISD::FCopySign => lower_fcopysign(dag, node),
            ISD::DynamicStackAlloc => lower_dynamic_stackalloc(dag, node),
            ISD::StackSave => {
                let (chain, vt) = (dag.operand(node, 0), dag.value_type(node));
                Some(dag.get_copy_from_phys_reg_plain(chain, stack_register(vt), vt))
            }
            ISD::StackRestore => {
                let (chain, saved) = (dag.operand(node, 0), dag.operand(node, 1));
                let sp = dag.get_phys_register(dag.value_type(saved), stack_register(dag.value_type(saved)));
                Some(dag.get_node(ISD::CopyToReg, EVT::OTHER, &[chain, sp, saved]))
            }
            // `LowerFP_EXTEND`: the Custom arm exists for `f16`; a scalar
            // `f32` to `f64` is returned as is and selected by pattern.
            // DEVIATE SD107. `lowerFaddFsub`: the node itself unless an operand
            // pair is a horizontal add (SSE3 `haddps`), which the target does
            // not describe; so the scalar op is selected by pattern.
            ISD::FAdd | ISD::FSub => Some(node),
            ISD::FpExtend if dag.value_type(dag.operand(node, 0)) == EVT::Simple(MVT::F32) => Some(node),
            ISD::CtLz | ISD::CtLzZeroPoison => Some(lower_ctlz(dag, node)),
            ISD::CtTz => Some(lower_cttz(dag, node)),
            ISD::Parity => Some(lower_parity(dag, node)),
            ISD::CtPop => lower_ctpop(dag, node),
            _ => None,
        }
    }
}

pub(super) fn cond_code(
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
            // The pin gets this from `x <= C` -> `x < C + 1` ([L5358][pin]);
            // x86's
            // `canonicalize_ge_le` is off (DEVIATE in `dag_combiner`), so this
            // is that rewrite's sign-test case. Exit: that hook on.
            //
            // [pin]:
            // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L5358
            CondCode::SetLe if all_ones => {
                *rhs = dag.get_constant(0, vt);
                return cond::S;
            }
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

/// `hasNonFlagsUse`: whether `op` has a reader other than a branch, a compare
/// or a select condition (looking past a truncate nothing else reads).
fn has_non_flags_use(
    dag: &SelectionDAG,
    op: SDValue,
) -> bool {
    dag.users(op).any(|user| {
        let mut user = SDValue { node: user, res: 0 };
        let mut through = op;
        if dag.opcode(user) == ISD::Truncate {
            let mut readers = dag.users(user);
            if let (Some(only), None) = (readers.next(), readers.next()) {
                through = user;
                user = SDValue { node: only, res: 0 };
            }
        }
        let condition = dag.opcode(user) == ISD::Select && dag.operand(user, 0) == through;
        !matches!(dag.opcode(user), ISD::BrCond | ISD::SetCC) && !condition
    })
}

/// `isProfitableToUseFlagOp`: every reader of `op` is a register copy, a
/// compare or a store.
fn is_profitable_to_use_flag_op(
    dag: &SelectionDAG,
    op: SDValue,
) -> bool {
    dag.users(op)
        .all(|user| matches!(
            dag.opcode(SDValue { node: user, res: 0 }),
            ISD::CopyToReg | ISD::SetCC | ISD::Store
        ))
}

/// `EmitTest`: the flags of `op` against zero. An arithmetic result that only
/// feeds registers, stores and compares is made by the flag-setting x86 form
/// and the compare reads its flags, when the condition needs neither the carry
/// nor the overflow flag (a `test` clears them).
fn emit_test(
    dag: &mut SelectionDAG,
    op: SDValue,
    code: u128,
) -> SDValue {
    let vt = dag.value_type(op);
    let zero = dag.get_constant(0, vt);
    let test = |dag: &mut SelectionDAG| dag.get_node(x86isd::CMP, EVT::I32, &[op, zero]);
    let need_carry = matches!(code, cond::A | cond::AE | cond::B | cond::BE);
    let wraps = !matches!(dag.opcode(op), ISD::Add | ISD::Sub | ISD::Mul | ISD::Shl)
        || dag.node(op).flags.0 & SDNodeFlags::NO_SIGNED_WRAP.0 == 0;
    let need_overflow = matches!(
        code,
        cond::G | cond::GE | cond::L | cond::LE | cond::O | cond::NO
    ) && wraps;
    if op.res != 0 || need_carry || need_overflow {
        return test(dag);
    }
    let opcode = match dag.opcode(op) {
        ISD::And if !has_non_flags_use(dag, op) => return test(dag),
        ISD::And => x86isd::AND,
        ISD::Add => x86isd::ADD,
        ISD::Sub => x86isd::SUB,
        ISD::Or => x86isd::OR,
        ISD::Xor => x86isd::XOR,
        x86isd::ADD | x86isd::SUB | x86isd::OR | x86isd::XOR | x86isd::AND => {
            return SDValue { node: op.node, res: 1 };
        }
        _ => return test(dag),
    };
    if !is_profitable_to_use_flag_op(dag, op) {
        return test(dag);
    }
    let (lhs, rhs) = (dag.operand(op, 0), dag.operand(op, 1));
    let new = dag.get_node_multi(opcode, &[vt, EVT::I32], &[lhs, rhs]);
    dag.replace_all_uses_with(op, SDValue { node: new.node, res: 0 });
    SDValue { node: new.node, res: 1 }
}

/// `EmitCmp`: the flags of `op0 - op1`.
fn emit_cmp(
    dag: &mut SelectionDAG,
    op0: SDValue,
    op1: SDValue,
    code: u128,
) -> SDValue {
    if dag.is_null_constant(op1) {
        // a runtime routine's answer in the flags: they are already set
        if dag.opcode(op0) == super::x86isd::LLRM_CALL_FLAGS {
            return dag.operand(op0, 0);
        }
        return emit_test(dag, op0, code);
    }
    let vt = dag.value_type(op0);
    let sub = dag.get_node_multi(x86isd::SUB, &[vt, EVT::I32], &[op0, op1]);
    SDValue { node: sub.node, res: 1 }
}

/// `getBT`: `bt src, bit` as flags; the source is any-extended to a dword
/// (there is no byte `bt`, the word form is longer) and the bit number follows
/// its width.
fn get_bt(
    dag: &mut SelectionDAG,
    mut src: SDValue,
    bit: SDValue,
) -> SDValue {
    if dag.value_type(src).size_in_bits() < 32 {
        src = dag.get_node(ISD::AnyExtend, EVT::I32, &[src]);
    }
    let vt = dag.value_type(src);
    let bit = match dag.value_type(bit).size_in_bits().cmp(&vt.size_in_bits()) {
        std::cmp::Ordering::Equal => bit,
        std::cmp::Ordering::Less => dag.get_node(ISD::AnyExtend, vt, &[bit]),
        std::cmp::Ordering::Greater => dag.get_node(ISD::Truncate, vt, &[bit]),
    };
    dag.get_node(x86isd::BT, EVT::I32, &[src, bit])
}

/// `LowerAndToBT`: `(x & (1 << n)) == 0` and `((x >> n) & 1) != 0` test bit `n`
/// of `x`; the carry flag is the bit, so `!= 0` is `B` and `== 0` is `AE`.
/// (The reassociated three-way `and` and the immediate forms are not taken,
/// SD200.)
pub(super) fn lower_and_to_bt(
    dag: &mut SelectionDAG,
    and: SDValue,
    cc: CondCode,
) -> Option<(SDValue, u128)> {
    let (a, b) = (dag.operand(and, 0), dag.operand(and, 1));
    let is_bit_shift =
        |dag: &SelectionDAG, v: SDValue| dag.opcode(v) == ISD::Shl && dag.constant(dag.operand(v, 0)) == Some(1);
    let (mut src, bit) = if is_bit_shift(dag, b) {
        (a, dag.operand(b, 1))
    } else if is_bit_shift(dag, a) {
        (b, dag.operand(a, 1))
    } else if dag.constant(b) == Some(1) && dag.opcode(a) == ISD::Srl {
        (dag.operand(a, 0), dag.operand(a, 1))
    } else if dag.constant(a) == Some(1) && dag.opcode(b) == ISD::Srl {
        (dag.operand(b, 0), dag.operand(b, 1))
    } else {
        return None;
    };
    let mut cc = cc;
    // A bit flip folds into the condition.
    let ones = super::fold::mask(dag.value_type(src).size_in_bits());
    if dag.opcode(src) == ISD::Xor && dag.constant(dag.operand(src, 1)) == Some(ones) {
        src = dag.operand(src, 0);
        cc = if cc == CondCode::SetEq { CondCode::SetNe } else { CondCode::SetEq };
    }
    if dag.value_type(src).size_in_bits() > 32 {
        return None;
    }
    let flags = get_bt(dag, src, bit);
    Some((flags, if cc == CondCode::SetEq { cond::AE } else { cond::B }))
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
    if matches!(cc, CondCode::SetEq | CondCode::SetNe)
        && dag.opcode(op0) == ISD::And
        && dag.users(op0).count() == 1
        && dag.is_null_constant(op1)
    {
        if let Some((flags, code)) = lower_and_to_bt(dag, op0, cc) {
            return (flags, cond_code(dag, code));
        }
    }
    // The carry of the add stands for the compare: `(seteq (add x, -1), -1)`
    // is `x == 0`, which `x + -1` carries out of unless it is 0 (`dec; cmp -1`
    // was two instructions, `add -1; jb` is one).
    if matches!(cc, CondCode::SetEq | CondCode::SetNe)
        && dag.constant(op1) == Some(super::fold::mask(dag.value_type(op1).size_in_bits()))
        && dag.opcode(op0) == ISD::Add
        && dag.operand(op0, 1) == op1
        && is_profitable_to_use_flag_op(dag, op0)
    {
        let vt = dag.value_type(op0);
        let x = dag.operand(op0, 0);
        let new = dag.get_node_multi(x86isd::ADD, &[vt, EVT::I32], &[x, op1]);
        dag.replace_all_uses_with(op0, SDValue { node: new.node, res: 0 });
        let code = if cc == CondCode::SetEq { cond::AE } else { cond::B };
        return (SDValue { node: new.node, res: 1 }, cond_code(dag, code));
    }
    let mut rhs = op1;
    let code = translate_cc(dag, cc, &mut rhs);
    let flags = emit_cmp(dag, op0, rhs, code);
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

/// `LowerSELECTWithCmpZero`, the `(x & 1) == 0` part: a select on the low bit
/// is arithmetic on the bit's mask `-(x & 1)`. Without `cmov`, a select of two
/// constants and a select between `y` and `y op z` become a mask, an `and` and
/// the operation. The `x - 1` / `0 - x` carry forms (`sbb`) are not translated.
fn select_with_cmp_zero(
    dag: &mut SelectionDAG,
    cmp_val: SDValue,
    lhs: SDValue,
    rhs: SDValue,
    code: u128,
    can_cmov: bool,
) -> Option<SDValue> {
    let (cmp_vt, vt) = (dag.value_type(cmp_val), dag.value_type(lhs));
    if !cmp_vt.is_integer() || !vt.is_integer() {
        return None;
    }
    if code != cond::E || dag.opcode(cmp_val) != ISD::And || dag.constant(dag.operand(cmp_val, 1)) != Some(1) {
        return None;
    }
    // `-(x & 1)` in `splat_vt`.
    let splat_lsb = |dag: &mut SelectionDAG, splat_vt: EVT| {
        let neg = if cmp_vt.size_in_bits() > splat_vt.size_in_bits() {
            dag.get_node(ISD::Truncate, splat_vt, &[cmp_val])
        } else if cmp_vt.size_in_bits() < splat_vt.size_in_bits() {
            let wide = dag.get_node(ISD::AnyExtend, splat_vt, &[dag.operand(cmp_val, 0)]);
            let one = dag.get_constant(1, splat_vt);
            dag.get_node(ISD::And, splat_vt, &[wide, one])
        } else {
            cmp_val
        };
        dag.get_negative(neg, splat_vt)
    };
    let all_ones = |dag: &SelectionDAG, v: SDValue| dag.constant(v) == Some(super::fold::mask(dag.width(v)));
    if dag.is_null_constant(lhs) && all_ones(dag, rhs) {
        return Some(splat_lsb(dag, vt));
    }
    if !can_cmov && dag.constant(lhs).is_some() && dag.constant(rhs).is_some() {
        let mask = splat_lsb(dag, vt);
        let diff = dag.get_node(ISD::Xor, vt, &[lhs, rhs]);
        let flip = dag.get_node(ISD::And, vt, &[mask, diff]);
        return Some(dag.get_node(ISD::Xor, vt, &[lhs, flip]));
    }
    if can_cmov {
        return None;
    }
    // `select (x & 1) == 0, y, (y op z)` is `y op (-(x & 1) & z)`.
    let rop = dag.opcode(rhs);
    let (r0, r1) = if dag.node(rhs).ops.len() == 2 { (dag.operand(rhs, 0), dag.operand(rhs, 1)) } else { (rhs, rhs) };
    let identity = match rop {
        ISD::Or | ISD::Xor | ISD::Add if r0 == lhs || r1 == lhs => Some((if r0 == lhs { r1 } else { r0 }, lhs)),
        ISD::Shl | ISD::Sra | ISD::Srl | ISD::Sub if r0 == lhs => Some((r1, lhs)),
        _ => None,
    };
    if let Some((src1, src2)) = identity {
        let src_vt = dag.value_type(src1);
        let mask = splat_lsb(dag, src_vt);
        let and = dag.get_node(ISD::And, src_vt, &[mask, src1]);
        return Some(dag.get_node(rop, vt, &[src2, and]));
    }
    // `select (x & 1) == 0, (y & z), y` is `y & (-(x & 1) | z)`.
    if dag.opcode(lhs) == ISD::And {
        let (l0, l1) = (dag.operand(lhs, 0), dag.operand(lhs, 1));
        if l0 == rhs || l1 == rhs {
            let src1 = if l0 == rhs { l1 } else { l0 };
            let mask = splat_lsb(dag, vt);
            let or = dag.get_node(ISD::Or, vt, &[mask, src1]);
            return Some(dag.get_node(ISD::And, vt, &[rhs, or]));
        }
    }
    None
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
    // A raw condition `x & 1` is `(x & 1) == 0` with the arms swapped.
    if dag.opcode(cond) == ISD::And && dag.constant(dag.operand(cond, 1)) == Some(1) {
        if let Some(lowered) = select_with_cmp_zero(dag, cond, op2, op1, cond::E, can_cmov) {
            return Some(lowered);
        }
    }
    if dag.opcode(cond) == x86isd::SETCC && dag.opcode(dag.operand(cond, 1)) == x86isd::CMP {
        let cmp = dag.operand(cond, 1);
        if dag.is_null_constant(dag.operand(cmp, 1)) {
            if let Some(code) = dag.constant(dag.operand(cond, 0)) {
                if let Some(lowered) = select_with_cmp_zero(dag, dag.operand(cmp, 0), op1, op2, code, can_cmov) {
                    return Some(lowered);
                }
            }
        }
    }
    let (code, flags);
    if dag.opcode(cond) == x86isd::SETCC && is_logical_cmp(dag, dag.operand(cond, 1)) {
        code = dag.operand(cond, 0);
        flags = dag.operand(cond, 1);
    } else {
        code = cond_code(dag, cond::NE);
        flags = emit_test(dag, cond, cond::NE);
    }
    // DEVIATE SD239: where the pointer is a word (m16) an `i16` select stays
    // a word: widened to `i32` every move takes an operand-size prefix. Owner
    // w10. Exit: none (machine fact). Review 2026-12-10.
    let word16 = dag.config.pointer == EVT::I16;
    let (op1, op2, result_vt) = if (vt == EVT::I8 && can_cmov)
        || (vt == EVT::I16 && !word16 && !may_fold_load(dag, op1) && !may_fold_load(dag, op2))
    {
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
    if dag.opcode(cond) == ISD::SetCC && dag.value_type(dag.operand(cond, 0)).is_floating_point() {
        if let Some(branches) = lower_float_brcond(dag, node, cond) {
            return Some(branches);
        }
    }
    let (code, flags) = if dag.opcode(cond) == ISD::SetCC && dag.value_type(dag.operand(cond, 0)).is_integer() {
        let (lhs, rhs) = (dag.operand(cond, 0), dag.operand(cond, 1));
        let cc = cond_of(dag, dag.operand(cond, 2));
        let (flags, code) = emit_flags_for_setcc(dag, lhs, rhs, cc);
        (code, flags)
    } else {
        // Anything else is tested: `and cond, 1`, then `!= 0`; an `and` is
        // tried as a bit test first.
        if dag.opcode(cond) == ISD::And && dag.users(cond).count() == 1 {
            if let Some((flags, code)) = lower_and_to_bt(dag, cond, CondCode::SetNe) {
                let code = cond_code(dag, code);
                return Some(dag.get_node(x86isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags]));
            }
        }
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

/// `LowerBRCOND` of a float compare. `une` is two branches to the target
/// (not equal, or unordered); `oeq` followed by a `br` is two branches to the
/// other successor and the `br` to this one. Other codes are one branch on
/// the compare's flags; `oeq` with no `br` after it is left to the generic
/// path (the compare expands to two).
fn lower_float_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
    cond: SDValue,
) -> Option<SDValue> {
    let (chain, dest) = (dag.operand(node, 0), dag.operand(node, 2));
    let (lhs, rhs) = (dag.operand(cond, 0), dag.operand(cond, 1));
    let branch = |dag: &mut SelectionDAG, chain, dest, code, flags| {
        let code = cond_code(dag, code);
        dag.get_node(x86isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags])
    };
    match cond_of(dag, dag.operand(cond, 2)) {
        CondCode::SetUNe => {
            let flags = dag.get_node(x86isd::FCMP, EVT::I32, &[lhs, rhs]);
            let first = branch(dag, chain, dest, cond::NE, flags);
            Some(branch(dag, first, dest, cond::P, flags))
        }
        CondCode::SetOEq => {
            let users: Vec<u32> = dag.users(node).collect();
            let [user] = users[..] else { return None };
            let user = SDValue { node: user, res: 0 };
            if dag.opcode(user) != ISD::Br {
                return None;
            }
            let false_block = dag.operand(user, 1);
            dag.update_node_operands(user.node, &[dag.operand(user, 0), dest]);
            let flags = dag.get_node(x86isd::FCMP, EVT::I32, &[lhs, rhs]);
            let first = branch(dag, chain, false_block, cond::NE, flags);
            Some(branch(dag, first, false_block, cond::P, flags))
        }
        _ => {
            let (flags, code) = float_flags(dag, cond)?;
            Some(dag.get_node(x86isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags]))
        }
    }
}

/// `checkBoolTestSetCCCombine`: `cmp (setcc c, f), 0` with `==` or `!=` (or
/// against 1, through zero extends, truncates and `and 1`) tests the flags `f`
/// of the inner compare: its condition, or the opposite one.
fn check_bool_test(
    dag: &SelectionDAG,
    cmp: SDValue,
    code: u128,
) -> Option<(SDValue, u128)> {
    let is_cmp = dag.opcode(cmp) == x86isd::CMP
        || (dag.opcode(cmp) == x86isd::SUB
            && cmp.res == 1
            && dag.users(SDValue { node: cmp.node, res: 0 }).next().is_none());
    if !is_cmp || !matches!(code, cond::E | cond::NE) {
        return None;
    }
    let (a, b) = (dag.operand(cmp, 0), dag.operand(cmp, 1));
    let (constant, mut test) = match (dag.constant(a), dag.constant(b)) {
        (Some(c), _) => (c, b),
        (None, Some(c)) => (c, a),
        _ => return None,
    };
    let mut opposite = code == cond::E;
    let against_true = constant == 1;
    if against_true {
        opposite = !opposite;
    } else if constant != 0 {
        return None;
    }
    while matches!(dag.opcode(test), ISD::ZeroExtend | ISD::Truncate | ISD::And) {
        if dag.opcode(test) == ISD::And {
            let at = if dag.constant(dag.operand(test, 0)) == Some(1) {
                1
            } else if dag.constant(dag.operand(test, 1)) == Some(1) {
                0
            } else {
                break;
            };
            test = dag.operand(test, at);
        } else {
            test = dag.operand(test, 0);
        }
    }
    if dag.opcode(test) != x86isd::SETCC {
        return None;
    }
    let inner = dag.constant(dag.operand(test, 0))?;
    Some((dag.operand(test, 1), if opposite { inner ^ 1 } else { inner }))
}

/// `combineX86SetCC`: a `setcc` of a boolean test reads the inner flags.
fn combine_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let code = dag.constant(dag.operand(node, 0))?;
    let (flags, code) = check_bool_test(dag, dag.operand(node, 1), code)?;
    let code = cond_code(dag, code);
    Some(dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]))
}

/// `combineCarryThroughADD`: flags of `(add carry, -1)`, where the carry is a
/// `setb` (through extends, truncates and `and 1`), are the flags the `setb`
/// read. DEVIATE SD242: the `COND_A` form (a swapped `sub`) is not
/// translated. Reason: nothing produces it at 32 bits yet. Exit: with
/// `combineSBB`'s remaining folds. Owner: sdag-b. Review 2026-12-10.
fn carry_through_add(
    dag: &SelectionDAG,
    flags: SDValue,
) -> Option<SDValue> {
    if dag.opcode(flags) != x86isd::ADD {
        return None;
    }
    let minus_one = dag.operand(flags, 1);
    if dag.constant(minus_one) != Some(super::fold::mask(dag.value_type(minus_one).size_in_bits())) {
        return None;
    }
    let mut carry = dag.operand(flags, 0);
    while matches!(
        dag.opcode(carry),
        ISD::Truncate | ISD::ZeroExtend | ISD::AnyExtend
    )
        || (dag.opcode(carry) == ISD::And && dag.constant(dag.operand(carry, 1)) == Some(1))
    {
        carry = dag.operand(carry, 0);
    }
    if !matches!(dag.opcode(carry), x86isd::SETCC | x86isd::SETCC_CARRY) {
        return None;
    }
    (dag.constant(dag.operand(carry, 0))? == cond::B).then(|| dag.operand(carry, 1))
}

/// `combineADC` and `combineSBB`, the carry-through-add part: the carry in
/// comes from the flags the `setb` read, so `setb; add al, -1; adc` is `adc`.
/// (The `ADC 0, 0` strength reduction is not translated: it replaces both
/// results.)
fn combine_adc_sbb(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (opcode, vt) = (dag.opcode(node), dag.node(node).values.clone());
    let (lhs, rhs, carry) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let flags_used = dag.has_users(SDValue { node: node.node, res: 1 });
    if opcode == x86isd::ADC {
        let (lhs_c, rhs_c) = (dag.constant(lhs), dag.constant(rhs));
        // Canonicalize constant to RHS.
        if lhs_c.is_some() && rhs_c.is_none() {
            return Some(dag.get_node_multi(opcode, &vt, &[rhs, lhs, carry]));
        }
        // Fold ADC(C1,C2,Carry) -> ADC(0,C1+C2,Carry) iff the flag result is
        // dead or C1+C2 doesn't signed/unsigned overflow.
        if let (Some(c1), Some(c2)) = (lhs_c, rhs_c)
            && c1 != 0
        {
            let ty = dag.value_type(lhs);
            let width = ty.size_in_bits();
            let mask = super::fold::mask(width);
            let sum = (c1 + c2) & mask;
            let signed = |v: u128| ((v << (128 - width)) as i128) >> (128 - width);
            let unsigned_overflow = c1 + c2 > mask;
            let signed_overflow = signed(c1).checked_add(signed(c2)).is_none_or(|s| s != signed(sum));
            if !flags_used || (!unsigned_overflow && !signed_overflow) {
                let zero = dag.get_constant(0, ty);
                let sum = dag.get_constant(sum, ty);
                return Some(dag.get_node_multi(opcode, &vt, &[zero, sum, carry]));
            }
        }
    }
    if let Some(flags) = carry_through_add(dag, carry) {
        return Some(dag.get_node_multi(opcode, &vt, &[lhs, rhs, flags]));
    }
    // Fold SBB(SUB(X,Y),0,Carry) -> SBB(X,Y,Carry) iff the flag result is dead.
    if opcode == x86isd::SBB && dag.opcode(lhs) == ISD::Sub && dag.constant(rhs) == Some(0) && !flags_used {
        let (x, y) = (dag.operand(lhs, 0), dag.operand(lhs, 1));
        return Some(dag.get_node_multi(opcode, &vt, &[x, y, carry]));
    }
    None
}

/// `combineBrCond`: the same for a branch.
fn combine_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let code = dag.constant(dag.operand(node, 2))?;
    let (flags, code) = check_bool_test(dag, dag.operand(node, 3), code)?;
    let code = cond_code(dag, code);
    let (chain, dest) = (dag.operand(node, 0), dag.operand(node, 1));
    Some(dag.get_node(x86isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags]))
}

/// `combineAdd`, the hidden LEA pattern: `(add (sub (shl x, c), y), z)` with
/// `c < 4` is `(sub (add (shl x, c), z), y)`, so the `shl` and the `add` are
/// one `lea` (`y * 7 + z` is `lea z + y*8; sub y`). (The vector, `cmov`,
/// `psadbw` and `sbb` folds are not translated.)
fn combine_add(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if vt != EVT::I32 {
        return super::x86_carry::add_or_sub_node_to_adc_or_sbb(dag, node);
    }
    // `m_Add` matches either operand as the difference.
    let hidden_lea = |difference: SDValue, z: SDValue| {
        if dag.opcode(difference) != ISD::Sub || !dag.has_one_use(difference.node) || dag.constant(z).is_some() {
            return None;
        }
        let (shift, y) = (dag.operand(difference, 0), dag.operand(difference, 1));
        if dag.opcode(shift) != ISD::Shl || !dag.constant(dag.operand(shift, 1)).is_some_and(|amount| amount < 4) {
            return None;
        }
        Some((shift, y, z))
    };
    let (a, b) = (dag.operand(node, 0), dag.operand(node, 1));
    let Some((shift, y, z)) = hidden_lea(a, b).or_else(|| hidden_lea(b, a)) else {
        return super::x86_carry::add_or_sub_node_to_adc_or_sbb(dag, node);
    };
    let sum = dag.get_node(ISD::Add, vt, &[shift, z]);
    Some(dag.get_node(ISD::Sub, vt, &[sum, y]))
}

/// `matchGenericOp`: a generic node made of the same operands reads the
/// result of `node` instead (`negate`: its negation).
fn match_generic_op(
    dag: &mut SelectionDAG,
    opcode: ISD,
    node: SDValue,
    lhs: SDValue,
    rhs: SDValue,
    negate: bool,
) {
    let vt = dag.value_type(node);
    let Some(found) = dag.get_node_if_exists(opcode, &[vt], &[lhs, rhs]) else { return };
    let generic = SDValue { node: found, res: 0 };
    if !dag.has_users(generic) {
        return;
    }
    let mut value = SDValue { node: node.node, res: 0 };
    if negate {
        // A generic node whose one user is also the only user of `node` would
        // make a cycle, or an unprofitable replacement.
        let users: Vec<u32> = dag.users(generic).collect();
        if let ([user], 1) = (&users[..], dag.use_count(found)) {
            let all = (0..dag.node(node).values.len() as u8)
                .all(|res| dag.users(SDValue { node: node.node, res }).all(|other| other == *user));
            if all {
                return;
            }
        }
        value = dag.get_negative(value, vt);
    }
    dag.combine_to(generic, value);
}

/// `combineX86AddSub` (the flags and generic-node parts): an `X86ISD::ADD` or
/// `SUB` whose flags nothing reads is the generic node; one whose flags are
/// read takes over the generic `add`/`sub` of the same operands, and
/// `add x, -c` is `sub x, c`, so `a >= c ? a - c : a` is one `sub` and a
/// branch where it was a `lea` and a `cmp`. (The `ADC`/`SBB` folds and the
/// `combineX86SubCmpForFlags` are not translated.)
fn combine_x86_add_sub(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if !vt.is_integer() {
        return None;
    }
    let is_sub = dag.opcode(node) == x86isd::SUB;
    let generic = if is_sub { ISD::Sub } else { ISD::Add };
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    if !dag.has_users(SDValue { node: node.node, res: 1 }) {
        return Some(dag.get_node(generic, vt, &[lhs, rhs]));
    }
    match_generic_op(dag, generic, node, lhs, rhs, false);
    match_generic_op(dag, generic, node, rhs, lhs, is_sub);
    let mask = super::fold::mask(vt.size_in_bits());
    if let Some(c) = dag.constant(rhs) {
        let negated = dag.get_constant(c.wrapping_neg() & mask, vt);
        if is_sub {
            match_generic_op(dag, ISD::Add, node, lhs, negated, false);
        } else {
            match_generic_op(dag, ISD::Sub, node, negated, lhs, true);
        }
    } else if let (Some(c), true) = (dag.constant(lhs), is_sub) {
        let negated = dag.get_constant(c.wrapping_neg() & mask, vt);
        match_generic_op(dag, ISD::Add, node, rhs, negated, true);
    }
    None
}

/// `combineUIntToFP`: `UINT_TO_FP` is custom, so the generic combine never
/// makes it a `SINT_TO_FP` when the sign bit is clear; here a `nneg` flag or a
/// known clear sign bit does, which is one `fild` of the value itself and not
/// of a 64-bit buffer holding it. (Vector forms are not translated.)
fn combine_uint_to_fp(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let value = dag.operand(node, 0);
    let vt = dag.value_type(node);
    let bits = dag.value_type(value).size_in_bits();
    let non_neg = dag.node(node).flags.has(SDNodeFlags::NON_NEG)
        || super::known_bits::known_bits(dag, value).zero >> (bits - 1) & 1 == 1;
    non_neg.then(|| dag.get_node(ISD::SIntToFp, vt, &[value]))
}

/// `combineSub`: `combineAddOrSubToADCOrSBB` ([`super::x86_carry`]), then
/// `combineSubSetcc`. (The `SUB(…, SBB)` and `~A - ~B` folds are not
/// translated, SD219.)
fn combine_sub(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    super::x86_carry::add_or_sub_node_to_adc_or_sbb(dag, node).or_else(|| combine_sub_setcc(dag, node))
}

/// `combineSubSetcc` ([L60838][pin]): `sub C, zext(setcc)` is
/// `zext(setcc inverted) + (C - 1)` for a nonzero `C` (`2 - (a < b)` is
/// `(a >= b) + 1`: a `setge` and an `inc`, not a load of the constant and a
/// `sub`).
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L60838
fn combine_sub_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (op0, op1) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let constant = dag.constant(op0).filter(|c| *c != 0)?;
    if dag.opcode(op1) != ISD::ZeroExtend || !dag.has_one_use(op1.node) {
        return None;
    }
    let setcc = dag.operand(op1, 0);
    if dag.opcode(setcc) != x86isd::SETCC || !dag.has_one_use(setcc.node) {
        return None;
    }
    let code = dag.constant(dag.operand(setcc, 0))?;
    let opposite = cond_code(dag, code ^ 1);
    let inverted = dag.get_node(x86isd::SETCC, EVT::I8, &[opposite, dag.operand(setcc, 1)]);
    let widened = dag.get_node(ISD::ZeroExtend, vt, &[inverted]);
    let adjusted = dag.get_constant(constant.wrapping_sub(1), vt);
    Some(dag.get_node(ISD::Add, vt, &[widened, adjusted]))
}

/// `combineCMov`, up to the constant selects: `cmov x, x` is `x`; a select
/// between two integer constants becomes the condition, widened, shifted,
/// added to or scaled: `c ? 8 : 0` is `zext(setcc) << 3`, `c ? k+1 : k` is
/// `zext(setcc) + k`, and a difference of 2, 3, 4, 5, 8 or 9 (one `lea`) is
/// `zext(setcc) * d + k`. (`combineSetCCEFLAGS`, the `x == c` register form,
/// the `adc` form and the double `cmov` are not translated, SD200.)
fn combine_cmov(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let (mut false_op, mut true_op) = (dag.operand(node, 0), dag.operand(node, 1));
    let mut code = dag.constant(dag.operand(node, 2))?;
    let flags = dag.operand(node, 3);
    if true_op == false_op {
        return Some(true_op);
    }
    // The flags of a boolean test are the inner compare's.
    if let Some((flags, simpler)) = check_bool_test(dag, flags, code) {
        let simpler = cond_code(dag, simpler);
        return Some(dag.get_node(x86isd::CMOV, vt, &[false_op, true_op, simpler, flags]));
    }
    let (Some(mut true_c), Some(mut false_c)) = (dag.constant(true_op), dag.constant(false_op)) else { return None };
    if !vt.is_integer() {
        return None;
    }
    // The true value is the larger one.
    if true_c < false_c {
        code ^= 1;
        std::mem::swap(&mut true_c, &mut false_c);
        std::mem::swap(&mut true_op, &mut false_op);
    }
    let mask = super::fold::mask(vt.size_in_bits());
    let difference = true_c.wrapping_sub(false_c) & mask;
    let widened = |dag: &mut SelectionDAG| {
        let code = cond_code(dag, code);
        let test = dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]);
        if vt == EVT::I8 { test } else { dag.get_node(ISD::ZeroExtend, vt, &[test]) }
    };
    if false_c == 0 && true_c.is_power_of_two() {
        let test = widened(dag);
        let by = dag.get_constant(u128::from(true_c.trailing_zeros()), EVT::I8);
        return Some(dag.get_node(ISD::Shl, vt, &[test, by]));
    }
    if false_c + 1 == true_c {
        let test = widened(dag);
        return Some(dag.get_node(ISD::Add, vt, &[test, false_op]));
    }
    if vt == EVT::I32 && matches!(difference, 1 | 2 | 3 | 4 | 5 | 8 | 9) {
        let mut result = widened(dag);
        if difference != 1 {
            let scale = dag.get_constant(difference, vt);
            result = dag.get_node(ISD::Mul, vt, &[result, scale]);
        }
        if false_c != 0 {
            result = dag.get_node(ISD::Add, vt, &[result, false_op]);
        }
        return Some(result);
    }
    None
}

/// `getGlobalWrapperKind`: every symbol is addressed from `%rip` in 64-bit
/// mode (small code model), the absolute `Wrapper` elsewhere.
fn wrapper_kind(vt: EVT) -> ISD {
    if vt == EVT::I64 { x86isd::WRAPPER_RIP } else { x86isd::WRAPPER }
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
    let wrapped = dag.get_node(wrapper_kind(vt), vt, &[target]);
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
    let NodeData::PoolIndex(index, offset) = dag.node(node).data else { return None };
    let vt = dag.value_type(node);
    let target =
        dag.intern(ISD::TargetConstantPool, &[vt], &[], SDNodeFlags::default(), NodeData::PoolIndex(index, offset));
    Some(dag.get_node(wrapper_kind(vt), vt, &[target]))
}

/// `LowerJumpTable`: `Wrapper (TargetJumpTable)`.
fn lower_jump_table(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let NodeData::JumpTable(index) = dag.node(node).data else { return None };
    let vt = dag.value_type(node);
    let target = dag.intern(ISD::TargetJumpTable, &[vt], &[], SDNodeFlags::default(), NodeData::JumpTable(index));
    Some(dag.get_node(wrapper_kind(vt), vt, &[target]))
}

/// `LowerXALUO`: the flags of `X86ISD::ADD`/`SUB`/`UMUL`/`SMUL`, and the
/// overflow as a `setcc` of them (`B` for the unsigned add and subtract, `E`
/// for an add of one, `O` for the rest), merged with the result.
fn lower_xalu(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let (opcode, code) = match dag.opcode(node) {
        ISD::UAddO => (x86isd::ADD, if dag.constant(rhs) == Some(1) { cond::E } else { cond::B }),
        ISD::USubO => (x86isd::SUB, cond::B),
        ISD::SAddO => (x86isd::ADD, cond::O),
        ISD::SSubO => (x86isd::SUB, cond::O),
        ISD::UMulO => (x86isd::UMUL, cond::B),
        _ => (x86isd::SMUL, cond::B),
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

/// `LowerSETCCCARRY`: the carry in is put into the flags (`carry + -1`), the
/// high halves are subtracted with it (`SBB`), and the condition is read from
/// the flags.
fn lower_setcc_carry(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (lhs, rhs, carry) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let NodeData::CondCode(cc) = dag.node(dag.operand(node, 3)).data else { unreachable!("a SETCCCARRY condition") };
    // `TranslateIntegerX86CC` has no constant rewrites: a stand-in that is
    // no constant keeps `translate_cc` from making any.
    let mut plain = carry;
    let x86_cc = translate_cc(dag, cc, &mut plain);
    let code = cond_code(dag, x86_cc);
    let carry_vt = dag.value_type(carry);
    let ones = dag.get_all_ones_constant(carry_vt);
    let set_carry = dag.get_node_multi(x86isd::ADD, &[carry_vt, EVT::I32], &[carry, ones]);
    let flags_in = SDValue { node: set_carry.node, res: 1 };
    let vt = dag.value_type(lhs);
    let sub = dag.get_node_multi(x86isd::SBB, &[vt, EVT::I32], &[lhs, rhs, flags_in]);
    let flags = SDValue { node: sub.node, res: 1 };
    dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags])
}

/// The two halves of `expandFP_TO_UINT` for an `i64`: `value` less 2^63 when it
/// is at least that, and the sign bit (`1 << 63`) to put back when it was.
fn unsigned_fixup_operands(
    dag: &mut SelectionDAG,
    value: SDValue,
    float: MVT,
) -> (SDValue, SDValue) {
    let value_vt = dag.value_type(value);
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
    let adjust = dag.get_node(ISD::Shl, EVT::I64, &[wide, sixty_three]);
    let zero = dag.get_constant_fp(0, value_vt);
    let offset = dag.get_select(value_vt, at_least, threshold, zero);
    (dag.get_node(ISD::FSub, value_vt, &[value, offset]), adjust)
}

/// `FP_TO_INTHelper` for the x87: `fist` of the value to a stack slot of the
/// result's size, then a load of it. An unsigned `i32` goes through an `i64`
/// slot; an unsigned `i64` subtracts 2^63 from a value at or above it and
/// flips the sign bit of the result back.
fn lower_fp_to_int(
    dag: &mut SelectionDAG,
    node: SDValue,
    sse2: bool,
) -> Option<SDValue> {
    let signed = dag.opcode(node) == ISD::FpToSInt;
    let result_vt = dag.value_type(node);
    let mut value = dag.operand(node, 0);
    let value_vt = dag.value_type(value);
    let EVT::Simple(float @ (MVT::F32 | MVT::F64 | MVT::F80)) = value_vt else { return None };
    if sse2 && matches!(float, MVT::F32 | MVT::F64) {
        // `cvttss2si`/`cvttsd2si` truncate to i32 (and i64 at m64); the other
        // widths convert through the next one up.
        let pointer = dag.config().pointer;
        let via = match (signed, result_vt) {
            (true, EVT::I32) => return Some(node),
            (true, EVT::I64) if pointer == EVT::I64 => return Some(node),
            (true, _) => Some(EVT::I32),
            (false, EVT::I64) => None,
            (false, _) if pointer == EVT::I64 => Some(EVT::I64),
            (false, _) => None,
        };
        if let Some(via) = via {
            let wide = dag.get_node(ISD::FpToSInt, via, &[value]);
            return Some(dag.get_node(ISD::Truncate, result_vt, &[wide]));
        }
        // An unsigned `i64` at m64 (`expandFP_TO_UINT`): the value less 2^63
        // when it is at least that, converted signed, with the sign bit put
        // back.
        if !signed && result_vt == EVT::I64 && pointer == EVT::I64 {
            let (shifted, adjust) = unsigned_fixup_operands(dag, value, float);
            let wide = dag.get_node(ISD::FpToSInt, EVT::I64, &[shifted]);
            return Some(dag.get_node(ISD::Xor, EVT::I64, &[wide, adjust]));
        }
    }
    let unsigned_fixup = !signed && result_vt == EVT::I64;
    let stored_vt = if !signed && result_vt != EVT::I64 { EVT::I64 } else { result_vt };
    let size = stored_vt.size_in_bits() / 8;
    let index = dag.create_stack_temporary(size, size);
    let slot = dag.get_frame_index(dag.config().pointer, index);
    let chain = dag.entry_node();
    let mut adjust = None;
    if unsigned_fixup {
        let (shifted, flip) = unsigned_fixup_operands(dag, value, float);
        value = shifted;
        adjust = Some(flip);
    }
    let info = MemInfo {
        mem_vt: stored_vt,
        align: size,
        volatile: false,
        base: MemBase::Stack(index),
        ext: None,
        space: 0,
        ordering: None,
    };
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

/// `LRINT_LLRINTHelper` for the x87: `fist` of the value, in the current
/// rounding mode, to a stack slot of the result's size, then a load of it.
fn lower_lrint(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let result_vt = dag.value_type(node);
    let value = dag.operand(node, 0);
    let EVT::Simple(MVT::F32 | MVT::F64 | MVT::F80) = dag.value_type(value) else { return None };
    let size = result_vt.size_in_bits() / 8;
    let index = dag.create_stack_temporary(size, size);
    let slot = dag.get_frame_index(dag.config().pointer, index);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: result_vt,
        align: size,
        volatile: false,
        base: MemBase::Stack(index),
        ext: None,
        space: 0,
        ordering: None,
    };
    let fist =
        dag.intern(x86isd::FIST, &[EVT::OTHER], &[chain, value, slot], SDNodeFlags::default(), NodeData::Mem(info));
    Some(dag.get_load(result_vt, fist, slot, info))
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
    sse2: bool,
) -> Option<SDValue> {
    let signed = dag.opcode(node) == ISD::SIntToFp;
    let dst = dag.value_type(node);
    let src = dag.operand(node, 0);
    let src_vt = dag.value_type(src);
    if !matches!(dst, EVT::Simple(MVT::F32 | MVT::F64 | MVT::F80)) || !src_vt.is_integer() {
        return None;
    }
    // The combiner leaves a custom `UINT_TO_FP` alone, so `LowerUINT_TO_FP`
    // makes it signed itself when the sign bit is known clear: an `i16`
    // promoted by zero extension is one `fild` of a dword.
    if !signed && combine_uint_to_fp(dag, node).is_some() {
        return Some(dag.get_node(ISD::SIntToFp, dst, &[src]));
    }
    let pointer = dag.config().pointer;
    if sse2 && matches!(dst, EVT::Simple(MVT::F32 | MVT::F64)) {
        let in_reg = |bits| src_vt.size_in_bits() == bits;
        if signed && (in_reg(32) || (in_reg(64) && pointer == EVT::I64)) {
            return Some(node);
        }
        if signed && in_reg(16) {
            let wide = dag.get_node(ISD::SignExtend, EVT::I32, &[src]);
            return Some(dag.get_node(ISD::SIntToFp, dst, &[wide]));
        }
        if !signed && in_reg(32) && pointer == EVT::I64 {
            let wide = dag.get_node(ISD::ZeroExtend, EVT::I64, &[src]);
            return Some(dag.get_node(ISD::SIntToFp, dst, &[wide]));
        }
    }
    // DEVIATE SD332 (exit: the pin gains it; re-check at each pin bump): a
    // plain one-use load of an `i16` or `i32` is `fild`ed from its own
    // address, as combineSIntToFP does for `i64` only. The stack slot's store
    // and reload are gone and `fimul`/`fiadd` read the operand in place.
    if signed && matches!(src_vt, EVT::I16 | EVT::I32) && is_non_ext_load(dag, src) && dag.users(src).count() == 1 {
        let NodeData::Mem(info) = dag.node(src).data else { return None };
        if !info.volatile && info.ordering.is_none() && info.mem_vt == src_vt {
            let (load_chain, address) = (dag.operand(src, 0), dag.operand(src, 1));
            let fild = build_fild(dag, dst, src_vt, load_chain, address, info);
            dag.replace_all_uses_with(SDValue { node: src.node, res: 1 }, SDValue { node: fild.node, res: 1 });
            return Some(fild);
        }
    }
    let chain = dag.entry_node();
    let store_info = |index: i32, vt: EVT, size: u32| MemInfo {
        mem_vt: vt,
        align: size,
        volatile: false,
        base: MemBase::Stack(index),
        ext: None,
        space: 0,
        ordering: None,
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
    let NodeData::PoolIndex(entry, _) = dag.node(fudge).data else { return None };
    let fudge_info = MemInfo {
        mem_vt: EVT::Simple(MVT::F32),
        align: 8,
        volatile: false,
        base: MemBase::ConstantPool(entry),
        ext: Some(super::actions::LoadExt::ExtLoad),
        space: 0,
        ordering: None,
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

/// `LowerVASTART`: at 32 bits a `va_list` is a pointer, so the start is a
/// store of the first variadic argument's address through it; the SysV
/// x86-64 one is `{gp_offset, fp_offset, overflow_arg_area, reg_save_area}`.
fn lower_vastart(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let index = dag.var_args_frame_index?;
    let (chain, list) = (dag.operand(node, 0), dag.operand(node, 1));
    let pointer = dag.config().pointer;
    let base = match dag.node(list).data {
        NodeData::FrameIndex(slot) => MemBase::Stack(slot),
        _ => MemBase::Unknown,
    };
    if pointer != EVT::I64 {
        let first = dag.get_frame_index(pointer, index);
        let info = MemInfo {
            mem_vt: pointer,
            align: pointer.size_in_bits() / 8,
            volatile: false,
            base,
            ext: None,
            space: 0,
            ordering: None,
        };
        return Some(dag.get_store(chain, first, list, info));
    }
    let (save, gp_offset, fp_offset) = dag.var_args_save?;
    let mut stores = Vec::new();
    let fields: [(u128, EVT, SDValue); 4] = [
        (0, EVT::I32, dag.get_constant(u128::from(gp_offset), EVT::I32)),
        (4, EVT::I32, dag.get_constant(u128::from(fp_offset), EVT::I32)),
        (8, pointer, dag.get_frame_index(pointer, index)),
        (16, pointer, dag.get_frame_index(pointer, save)),
    ];
    for (offset, vt, value) in fields {
        let at = dag.get_constant(offset, pointer);
        let address = dag.get_node(ISD::Add, pointer, &[list, at]);
        let info = MemInfo { mem_vt: vt, align: 4, volatile: false, base, ext: None, space: 0, ordering: None };
        stores.push(dag.get_store(chain, value, address, info));
    }
    Some(dag.get_node(ISD::TokenFactor, EVT::OTHER, &stores))
}

fn is_pow2(value: u128) -> bool {
    value != 0 && value & (value - 1) == 0
}

fn log2(value: u128) -> u128 {
    u128::from(127 - value.leading_zeros())
}

/// `X86ISD::MUL_IMM` by 3, 5 or 9: the `lea` the matcher folds it into. An
/// `i16` has no such `lea` pattern, so it is written as the add of a shift
/// (`x + (x << 1)`), which the same matcher turns into one.
fn mul_imm_node(
    dag: &mut SelectionDAG,
    x: SDValue,
    by: u128,
    vt: EVT,
) -> SDValue {
    if vt == EVT::I16 && matches!(by, 3 | 5 | 9) {
        let amount = dag.get_constant(log2(by - 1), EVT::I8);
        let shifted = dag.get_node(ISD::Shl, vt, &[x, amount]);
        return dag.get_node(ISD::Add, vt, &[x, shifted]);
    }
    let by = dag.get_constant(by, vt);
    dag.get_node(x86isd::MUL_IMM, vt, &[x, by])
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
        let product = mul_imm_node(dag, x, mult, vt);
        let amount = dag.get_constant(shift, EVT::I8);
        let shifted = dag.get_node(ISD::Shl, vt, &[product, amount]);
        dag.get_node(if add { ISD::Add } else { ISD::Sub }, vt, &[shifted, x])
    };
    let mul_mul_add = |dag: &mut SelectionDAG, first: u128, second: u128| {
        let once = mul_imm_node(dag, x, first, vt);
        let twice = mul_imm_node(dag, once, second, vt);
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
    min_size: bool,
    i16_chains: bool,
    profile: Option<MulPricing<'_>>,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if vt != EVT::I32 && !(vt == EVT::I16 && i16_chains) {
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
    // An imul is usually smaller than the alternative sequence.
    if is_pow2(bits) || min_size || level == CombineLevel::BeforeLegalizeTypes {
        return None;
    }
    let signed = if vt == EVT::I16 { i128::from(bits as u16 as i16) } else { i128::from(bits as u32 as i32) };
    let abs = signed.unsigned_abs();
    let negate = |dag: &mut SelectionDAG, value: SDValue| if signed < 0 { dag.get_negative(value, vt) } else { value };
    let shift = |dag: &mut SelectionDAG, value: SDValue, by: u128| {
        let by = dag.get_constant(by, EVT::I8);
        dag.get_node(ISD::Shl, vt, &[value, by])
    };
    let mul_imm = |dag: &mut SelectionDAG, value: SDValue, by: u128| mul_imm_node(dag, value, by, vt);
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
    // DEVIATE SD446: where the pin keeps `imul`, the sheet's cheaper chain.
    if new_mul.is_none() {
        new_mul = profile.and_then(|profile| priced_chain(dag, x, signed as i64, vt, profile));
    }
    new_mul
}

/// `x * c` as the chain `arithmetic::chain_at_speed` accepts (the clocks it
/// saves pay for its bytes at the block's frequency; the search is cached per
/// constant, O(1) here), built from shifts, adds, subtracts and
/// the `lea` forms. An `i16` has no `lea`: the narrow chain.
///
/// DEVIATE SD446: the pin's `combineMul` leaves an `imul` for a constant none
/// of its cases covers (75, 251, 10000); on the 486 that is 13-42 clocks
/// (Intel 240440-002 Table 10.1) against 2-4 for a chain. The search is
/// GCC's `synth_mult`
/// ([expmed.cc](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/expmed.cc)),
/// the same as DEVIATE M1 in `arithmetic.rs`. Only where the target trades
/// bytes for clocks at speed (m16: `speed_milliclocks_per_byte`); the block's
/// frequency is the IR `BlockFrequencies` (the machine one does not exist at
/// DAG time). Exit: none while the clocks decide. Owner sdag-d.
fn priced_chain(
    dag: &mut SelectionDAG,
    x: SDValue,
    c: i64,
    vt: EVT,
    pricing: MulPricing<'_>,
) -> Option<SDValue> {
    use crate::backend::arithmetic;
    let width = if vt == EVT::I16 { 2 } else { 4 };
    let MulPricing { cpu, arch, freq, milliclocks_per_byte, by_clocks } = pricing;
    let chain = if by_clocks {
        arithmetic::chain_by_clocks(c, width, cpu)
    } else {
        arithmetic::chain_at_speed(c, width, cpu, arch, freq, milliclocks_per_byte)
    }
    .ok()??;
    let shift = |dag: &mut SelectionDAG, value: SDValue, by: i64| {
        let by = dag.get_constant(by as u128, EVT::I8);
        dag.get_node(ISD::Shl, vt, &[value, by])
    };
    let mut value = x;
    for (step, count) in chain {
        value = match step {
            "shl" => shift(dag, value, count),
            "add" => dag.get_node(ISD::Add, vt, &[value, x]),
            "sub" => dag.get_node(ISD::Sub, vt, &[value, x]),
            "rsub" => dag.get_node(ISD::Sub, vt, &[x, value]),
            "neg" => dag.get_negative(value, vt),
            // `x + value * count`
            "lea" => {
                let scaled = shift(dag, value, i64::from(count.trailing_zeros()));
                dag.get_node(ISD::Add, vt, &[scaled, x])
            }
            // `value + value * count`
            "flea" => mul_imm_node(dag, value, count as u128 + 1, vt),
            "fadd" => {
                let scaled = shift(dag, value, count);
                dag.get_node(ISD::Add, vt, &[value, scaled])
            }
            "fsub" => {
                let scaled = shift(dag, value, count);
                dag.get_node(ISD::Sub, vt, &[scaled, value])
            }
            _ => return None,
        };
    }
    Some(value)
}

/// `LowerFunnelShift`, the scalar part ([L32188][pin]). An `i8` by a variable
/// is done in a dword: `fshl(x,y,z)` is `(((aext x << 8) | zext y) << (z & 7))
/// >> 8`. An `i8` by a constant is two shifts and an `or`
/// (`expandFunnelShift`). An `i16` is the target node with the amount modulo
/// 16 (`SHLD16`/`SHRD16`); the dword is already legal.
///
/// DEVIATE SD247: the `isSHLDSlow` expansion of `i16` is not translated (no
/// target of ours sets it). Owner sdag-b.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L32188
fn lower_funnel_shift(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let is_fshr = dag.opcode(node) == ISD::FShr;
    let (op0, op1, amount) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let amount_vt = dag.value_type(amount);
    let bits = vt.size_in_bits();
    if vt == EVT::I8 {
        if let Some(by) = dag.constant(amount) {
            let by = (by % u128::from(bits)) as u32;
            if by == 0 {
                return Some(if is_fshr { op1 } else { op0 });
            }
            let (left, right) = if is_fshr { (bits - by, by) } else { (by, bits - by) };
            let high = dag.get_constant(u128::from(left), amount_vt);
            let low = dag.get_constant(u128::from(right), amount_vt);
            let high = dag.get_node(ISD::Shl, vt, &[op0, high]);
            let low = dag.get_node(ISD::Srl, vt, &[op1, low]);
            return Some(dag.get_node(ISD::Or, vt, &[high, low]));
        }
        let wide = EVT::I32;
        let mask = dag.get_constant(u128::from(bits - 1), amount_vt);
        let shift = dag.get_constant(u128::from(bits), amount_vt);
        let hi = dag.get_node(ISD::AnyExtend, wide, &[op0]);
        let lo = dag.get_zext_or_trunc(op1, wide);
        let amount = dag.get_node(ISD::And, amount_vt, &[amount, mask]);
        let hi = dag.get_node(ISD::Shl, wide, &[hi, shift]);
        let mut result = dag.get_node(ISD::Or, wide, &[hi, lo]);
        if is_fshr {
            result = dag.get_node(ISD::Srl, wide, &[result, amount]);
        } else {
            result = dag.get_node(ISD::Shl, wide, &[result, amount]);
            result = dag.get_node(ISD::Srl, wide, &[result, shift]);
        }
        return Some(dag.get_node(ISD::Truncate, vt, &[result]));
    }
    if vt == EVT::I16 {
        let mask = dag.get_constant(15, amount_vt);
        let amount = dag.get_node(ISD::And, amount_vt, &[amount, mask]);
        let op = if is_fshr { x86isd::FSHR } else { x86isd::FSHL };
        return Some(dag.get_node(op, vt, &[op0, op1, amount]));
    }
    None
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

/// `LowerFABSorFNEG` for a scalar: no scalar bitwise SSE instruction exists, so
/// the sign bit is cleared (`FAND`), flipped (`FXOR`) or set (`FOR`, for
/// `fneg (fabs x)`) by a 16-byte pool mask, the sign mask in every lane.
fn lower_fabs_or_fneg(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let sign = match vt {
        EVT::Simple(MVT::F32) => 1u128 << 31,
        EVT::Simple(MVT::F64) => 1u128 << 63,
        _ => return None,
    };
    let is_fabs = dag.opcode(node) == ISD::FAbs;
    // `fabs` under an `fneg` is left to the `fneg`, which sets the sign.
    if is_fabs && dag.users(node).any(|user| dag.opcode(SDValue { node: user, res: 0 }) == ISD::FNeg) {
        return Some(node);
    }
    let mut x = dag.operand(node, 0);
    let negated_abs = !is_fabs && dag.opcode(x) == ISD::FAbs;
    if negated_abs {
        x = dag.operand(x, 0);
    }
    let (op, mask) = match (is_fabs, negated_abs) {
        (true, _) => (x86isd::FAND, sign - 1),
        (false, true) => (x86isd::FOR, sign),
        (false, false) => (x86isd::FXOR, sign),
    };
    let address = mask_address(dag, mask, vt);
    Some(dag.get_node(op, vt, &[x, address]))
}

/// The address of the 16-byte constant `mask` of a scalar `vt` repeated in each
/// lane: the pin's `getConstantFP` of a `v4f32` or `v2f64` splat, a pool entry
/// the bitwise instruction reads whole.
fn mask_address(
    dag: &mut SelectionDAG,
    mask: u128,
    vt: EVT,
) -> SDValue {
    let lane = vt.size_in_bits();
    let splat = (0..128 / lane).fold(0u128, |all, at| all | mask << (at * lane));
    let pointer = dag.config().pointer;
    let pool = dag.get_constant_pool(pointer, splat, EVT::Simple(MVT::I128), true);
    dag.get_node(wrapper_kind(pointer), pointer, &[pool])
}

/// `LowerFCOPYSIGN` for a scalar: `(mag & ~sign) | (sgn & sign)`, the sign
/// operand first extended or rounded to the magnitude's type.
fn lower_fcopysign(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let sign = match vt {
        EVT::Simple(MVT::F32) => 1u128 << 31,
        EVT::Simple(MVT::F64) => 1u128 << 63,
        _ => return None,
    };
    let (mag, mut sgn) = (dag.operand(node, 0), dag.operand(node, 1));
    let sgn_vt = dag.value_type(sgn);
    if sgn_vt != vt {
        sgn = if sgn_vt.size_in_bits() < vt.size_in_bits() {
            dag.get_node(ISD::FpExtend, vt, &[sgn])
        } else {
            let zero = dag.get_constant_with(0, EVT::I32, true, false);
            dag.get_node(ISD::FpRound, vt, &[sgn, zero])
        };
    }
    let masked = |dag: &mut SelectionDAG, op, value, mask| {
        let address = mask_address(dag, mask, vt);
        dag.get_node(op, vt, &[value, address])
    };
    let magnitude = masked(dag, x86isd::FAND, mag, sign - 1);
    let sign_bit = masked(dag, x86isd::FAND, sgn, sign);
    Some(dag.get_node(x86isd::FOR, vt, &[magnitude, sign_bit]))
}

/// The stack pointer a pointer of type `vt` is saved from and restored to
/// (`getStackPointerRegisterToSaveRestore`): `rsp` at 64 bits.
fn stack_register(vt: EVT) -> llrm_lir::registers::RegId {
    if vt == EVT::I64 { llrm_lir::registers::RegId::RSP } else { llrm_lir::registers::RegId::SP }
}

/// `LowerDYNAMIC_STACKALLOC` without probes or split stacks: the stack pointer
/// lowered by the size, rounded down to the alignment when that is more than
/// the stack's (operand 2, else 0), is the address. Bracketed by a
/// `CALLSEQ` pair so nothing else moves the stack pointer meanwhile.
fn lower_dynamic_stackalloc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let (chain, size) = (dag.operand(node, 0), dag.operand(node, 1));
    let alignment = dag.constant(dag.operand(node, 2)).unwrap_or(0);
    let start = dag.get_callseq_start(chain, 0);
    let sp = dag.get_copy_from_phys_reg_plain(SDValue { node: start.node, res: 0 }, stack_register(vt), vt);
    let mut result = dag.get_node(ISD::Sub, vt, &[sp, size]);
    let mut chain = SDValue { node: sp.node, res: 1 };
    if alignment != 0 {
        let mask = dag.get_constant(!(alignment - 1) & ((1u128 << vt.size_in_bits()) - 1), vt);
        result = dag.get_node(ISD::And, vt, &[result, mask]);
    }
    chain = dag.get_copy_to_phys_reg(chain, stack_register(vt), result, None);
    let chain = SDValue { node: chain.node, res: 0 };
    let end = dag.get_callseq_end(chain, 0, 0, SDValue { node: chain.node, res: 1 });
    Some(dag.get_node_multi(ISD::MergeValues, &[vt, EVT::OTHER], &[result, SDValue { node: end.node, res: 0 }]))
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

/// `LowerADDSAT_SUBSAT` for a scalar `SSUBSAT` ([L29900][pin]): the signed
/// difference with overflow, and on overflow the maximum if the wrapped
/// difference is negative, else the minimum.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L29900
fn lower_ssubsat(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let vt = dag.value_type(node);
    let (x, y) = (dag.operand(node, 0), dag.operand(node, 1));
    let min = 1u128 << (vt.size_in_bits() - 1);
    let zero = dag.get_constant(0, vt);
    let result = dag.get_node_multi(ISD::SSubO, &[vt, EVT::I8], &[x, y]);
    let (difference, overflow) = (result, SDValue { node: result.node, res: 1 });
    let sat_min = dag.get_constant(min, vt);
    let sat_max = dag.get_constant(min - 1, vt);
    let negative = dag.get_setcc(EVT::I8, difference, zero, CondCode::SetLt);
    let saturated = dag.get_select(vt, negative, sat_max, sat_min);
    dag.get_select(vt, overflow, saturated, difference)
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
