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
//! DEVIATE: not translated yet are the `trunc` forms of `LowerToBTST`, the
//! `SETCC_CARRY` folds of `LowerSELECT`, the and/or-of-setcc and xor-1 folds of
//! `LowerBRCOND`, `EmitTest`'s narrowing of a truncated arithmetic result, and
//! floating-point conditions. Reason: each needs DAG queries or nodes the slice
//! does not reach. Exit: M3b.
//!
//! # See also
//! `llvm/lib/Target/M68k/M68kISelLowering.cpp`, pin d1106deb71cc: the
//! constructor (L48), `TranslateM68kCC` (L1774), `EmitTest` (L1888), `EmitCmp`
//! (L2103), `LowerSETCC` (L2126), `LowerSELECT` (L2244), `LowerBRCOND` (L2433).

use llrm_codegen::backend::selectiondag::isd::{CondCode, ISD};
use llrm_codegen::backend::selectiondag::legalize_dag::Hooks;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{Action, LoadExt, SDNodeFlags, SDValue, SelectionDAG, TargetLowering};
use llrm_codegen::backend::selectiondag::{MemBase, MemInfo, NodeData, TargetLoweringInfo};

use super::generated::ids::isd as ids;
use super::hooks::Cpu;

/// The pin's `M68kISD` numbers, as the matcher table names them.
pub mod m68kisd {
    use super::{ISD, ids};

    pub const CMP: ISD = ISD::Target(ids::CMP);
    pub const BTST: ISD = ISD::Target(ids::BTST);
    pub const SETCC: ISD = ISD::Target(ids::SETCC);
    pub const BRCOND: ISD = ISD::Target(ids::BRCOND);
    pub const CMOV: ISD = ISD::Target(ids::CMOV);
    pub const ADD: ISD = ISD::Target(ids::ADD);
    pub const SUB: ISD = ISD::Target(ids::SUB);
    pub const ADDX: ISD = ISD::Target(ids::ADDX);
    pub const SUBX: ISD = ISD::Target(ids::SUBX);
    pub const UMUL: ISD = ISD::Target(ids::UMUL);
    pub const SMUL: ISD = ISD::Target(ids::SMUL);
    pub const AND: ISD = ISD::Target(ids::AND);
    pub const OR: ISD = ISD::Target(ids::OR);
    pub const XOR: ISD = ISD::Target(ids::XOR);
    pub const CALL: ISD = ISD::Target(ids::CALL);
    pub const RET: ISD = ISD::Target(ids::RET);
    pub const TC_RETURN: ISD = ISD::Target(ids::TC_RETURN);
    pub const WRAPPER: ISD = ISD::Target(ids::Wrapper);
    pub const WRAPPER_PC: ISD = ISD::Target(ids::WrapperPC);
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
    target_lowering_with(cpu, true)
}

/// [`target_lowering`], with `carry_chain` false leaving `ADDC`, `ADDE`,
/// `SUBC` and `SUBE` to the legalizer's compare form (see
/// `Options::carry_chain`).
pub fn target_lowering_with(
    cpu: Cpu,
    carry_chain: bool,
) -> TargetLowering {
    use ISD::*;
    let int = [MVT::I8, MVT::I16, MVT::I32];
    let m68020 = cpu.at_least(Cpu::M68020);
    let mut tl =
        TargetLowering::empty(TargetLoweringInfo::new(&int, false).with_shift_by_value_type().with_soft_float());
    tl.init_actions();
    // no FPU at any CPU: every float operation is a compiler-rt call
    tl.set_soft_float_libcall_names();
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
    // DEVIATE M68K-REM32 (owner m68k, exit 2026-11-27): from the 68020 there
    // is a 32-bit divide, so `x % y` is `x - (x / y) * y` (the legalizer's own
    // `ExpandNode` for SREM/UREM) and links no `__modsi3`; gcc divides
    // inline. The pin keeps `LibCall` on every CPU (M68kISelLowering.cpp
    // L100).
    set(&[SRem, URem], &[MVT::I32], if m68020 { Action::Expand } else { Action::LibCall });
    for op in [UDivRem, SDivRem, MulHS, MulHU, UMulLoHi, SMulLoHi] {
        set(&[op], &[MVT::I8], Action::Promote);
        set(&[op], &[MVT::I16], Action::Legal);
        set(&[op], &[MVT::I32], Action::LibCall);
    }
    set(&[UMulLoHi, SMulLoHi], &[MVT::I8, MVT::I16], Action::Expand);
    // DEVIATE M68K-WIDEMUL (see `widemul`): the 64-bit `mulu.l` and `muls.l`.
    // The pin keeps `LibCall`. Owner m68k. Exit: 2026-11-27.
    if cpu.has_wide_multiply() {
        set(&[UMulLoHi, SMulLoHi], &[MVT::I32], Action::Custom);
    }
    // DEVIATE: `mulu.l` and `muls.l`, which `lower_xalu` selects for an `i32`
    // multiply with overflow, exist from the 68020; the 68000 expands it
    // (`expandMULO`) through the 64-bit product. The pin keeps `Custom` on
    // every CPU. Exit: never, while the pin does.
    set(&[SMulO, UMulO], &[MVT::I8, MVT::I16], Action::Custom);
    set(&[SMulO, UMulO], &[MVT::I32], if m68020 { Action::Custom } else { Action::Expand });
    set(&[ShlParts, SraParts, SrlParts], &[MVT::I32], Action::Custom);
    set(&[SAddO, UAddO, SSubO, USubO], &int, Action::Custom);
    set(&[AddC, AddE, SubC, SubE], &int, if carry_chain { Action::Custom } else { Action::Expand });
    set(&[BrCond], &[MVT::Other], Action::Custom);
    // `setOperationAction(ISD::BR_JT, MVT::Other, Expand)`: a load of the
    // table entry, then an indirect jump
    set(&[BrJT], &[MVT::Other], Action::Expand);
    set(&[Select, SetCC, SetCCCarry], &int, Action::Custom);
    // DEVIATE M68K-ABS-SELECT (owner m68k, exit 2026-11-24): `abs` is a
    // compare and a conditional negate (`tst`, `bpl`, `neg`), as gcc emits it.
    // The pin keeps `expandABS`, `(x ^ s) - s` for `s = x >>s 31`: a shift by
    // 31 is a register count on the 68000 (`asr.l` takes 1..8 as an
    // immediate, 6 + 2n clocks) and ties up a second register.
    set(&[Abs], &int, Action::Custom);
    set(&[BSwap], &[MVT::I8, MVT::I16], Action::Expand);
    set(&[BSwap], &[MVT::I32], Action::Legal);
    set(&[CtTz, CtLz, CtPop], &int, Action::Expand);
    // DEVIATE BC1 (llrm-sdag `actions`): the shared compiler-rt routines
    // (`__clzsi2`, `__ctzdi2`, `__popcountsi2`, ...) against the inline
    // expansion; the pin keeps `Expand` here. gcc calls libgcc for all six on
    // the 68000 and 68010, and so does this. From the 68020 gcc counts a
    // 32-bit `clz`, a 64-bit `clz` and a 32-bit `ctz` with `bfffo`
    // (M68K-BFFFO, see `bfffo`). Whole linked programs, 2026-11-09, every
    // count called against inline: u64 O2 -12 B and +1000 clk (+3% to +18% in
    // all six cells), u32 +24 B, so the 68020 and later stay inline at speed
    // levels; size-built functions choose per module (M68K-SIZE-LIBCALL, see
    // `libcalls`). Owner m68k. Exit: 2026-11-30.
    if !m68020 {
        set(&[CtLzZeroPoison, CtTzZeroPoison, CtPop], &[MVT::I32, MVT::I64], Action::LibCall);
    }
    // DEVIATE M68K-BFFFO (see `bfffo`): `bfffo` counts the leading zeros of an
    // `i32`, 32 for zero, so both forms are one instruction. The pin keeps
    // `Expand`. Owner m68k. Exit: 2026-11-30.
    if cpu.has_bit_field_find() {
        set(&[CtLz, CtLzZeroPoison], &[MVT::I32], Action::Legal);
    }
    set(&[ConstantPool, JumpTable, GlobalAddress, ExternalSymbol], &[MVT::I32], Action::Custom);
    set(&[VAStart], &[MVT::Other], Action::Custom);
    set(&[VAEnd, VACopy, StackSave, StackRestore], &[MVT::Other], Action::Expand);
    set(&[DynamicStackAlloc], &[MVT::I32], Action::Custom);
    // `RTLIB::MUL_I32` and the others: libgcc's names, the pin's defaults
    // (`RuntimeLibcalls.def`)
    for (op, name) in
        [(Mul, "__mulsi3"), (SDiv, "__divsi3"), (UDiv, "__udivsi3"), (SRem, "__modsi3"), (URem, "__umodsi3")]
    {
        tl.set_libcall_name(op, MVT::I32, name);
    }
    // `RTLIB::MUL_I64`: what `forceExpandWideMUL` calls for the product of two
    // `i32`s (a `smul.fix` with no `MULHS`, as the 68000's and the pin's)
    tl.set_libcall_name(Mul, MVT::I64, "__muldi3");
    // `setMinStackArgumentAlignment`: the pin's M68k leaves it at 1, so its
    // `va_arg` rounds the list to the type's alignment (2 for an `i32`) with
    // `addq #1; and #-2`. Every stack argument takes a 4-byte slot (gcc's
    // `PARM_BOUNDARY`), the 68000's stack stays even; no round is owed. Owner
    // m68k. DEVIATE M68K-VA-MIN-ALIGN, review 2026-12-20.
    tl.set_min_stack_argument_alignment(VA_SLOT);
    // `RTLIB::SDIV_I64` and the others: no CPU divides 64 bits
    for (op, name) in [(SDiv, "__divdi3"), (UDiv, "__udivdi3"), (SRem, "__moddi3"), (URem, "__umoddi3")] {
        tl.set_libcall_name(op, MVT::I64, name);
    }
    tl
}

/// The custom lowerings.
#[derive(Default)]
pub struct M68kLowering {
    /// Form `(An)+` and `-(An)` accesses; the pin sets no indexed action.
    pub indexed: bool,
    /// The beyond-pin size rewrites (`addq`, `extb.l`, `sub` of a sign-extended
    /// compare).
    pub short: bool,
    /// A global's address is an absolute one, not PC-relative (the medium
    /// model on the 68000 and 68010, the large model).
    pub absolute_data: bool,
    /// A multiply of two values that fit 16 bits is `mulu.w` / `muls.w`
    /// ([`prefers_word_multiply`]).
    pub half_mul: bool,
    /// The CPU and the level's price, which [`super::mulprice`] reads.
    pub cpu: Cpu,
    pub price: crate::timing::Price,
    pub mul: super::mulprice::Chain,
}

/// Whether `mulu.w` is no slower than `mulu.l` on `cpu`, by the timing
/// tables; the 68000 and 68010 have no `mulu.l`. `mulu.w` is also two bytes
/// shorter, so a tie goes to it.
pub fn prefers_word_multiply(cpu: Cpu) -> bool {
    use crate::isa::Cpu as Model;
    use crate::timing::cost_on;
    let model = match cpu {
        Cpu::M68000 | Cpu::M68010 => return true,
        Cpu::M68020 => Model::M68020,
        Cpu::M68030 => Model::M68030,
        Cpu::M68040 => Model::M68040,
        Cpu::M68060 => Model::M68060,
    };
    let clocks = |text| cost_on(model, text, 2, false).map(|cost| cost.clocks);
    matches!(
        (clocks("mulu.w %d1, %d0"), clocks("mulu.l %d1, %d0")),
        (Ok(word), Ok(long)) if word <= long
    )
}

impl llrm_codegen::backend::selectiondag::dag_combiner::CombineHooks for M68kLowering {
    fn little_endian(&self) -> bool {
        false
    }

    /// DEVIATE M68K-EXT-USES-TRUNC-FREE (owner llrm-target-m68k, exit 2026-11-24):
    /// `ExtendUsesToFormExtLoad` asks `isTruncateFree`, which the pin's m68k leaves
    /// false. A data register's low byte or word is the narrow value, so a load
    /// another reader takes narrow is still widened for a `zext`; with false
    /// `mul_words_to_long` is +4 B and `pattern_fill` -Os +2 B. `is_truncate_free`
    /// itself stays false: answering it true changed `shift_until_idioms`.
    fn extend_uses_truncate_free(
        &self,
        from: EVT,
        to: EVT,
    ) -> bool {
        !from.is_vector() && !to.is_vector() && from.is_integer() && to.is_integer() && from.size_in_bits() > to.size_in_bits()
    }

    /// `getScalarShiftAmountTy`: the pin's m68k keeps the default, the pointer
    /// type (`i32`). A shift a combine made after legalization with the
    /// shared `i8` amount had no pattern: `CannotSelect(Shl)`.
    fn shift_amount_type(
        &self,
        _shifted: EVT,
    ) -> EVT {
        EVT::I32
    }

    /// `M68kTargetLowering::decomposeMulByConstant`, priced: DEVIATE
    /// M68K-MUL-PRICE, see [`super::mulprice`].
    fn decompose_mul_by_constant(
        &self,
        vt: EVT,
        c: u128,
    ) -> bool {
        self.mul.allows(super::mulprice::decomposes(self.cpu, self.price, vt.size_in_bits(), c))
    }

    /// DEVIATE G2: the cheapest shift/add chain for any constant, priced
    /// ([`super::synth`]).
    fn synth_mul_chain(
        &self,
        vt: EVT,
        c: u128,
    ) -> Option<llrm_codegen::backend::selectiondag::dag_combiner::MulPlan> {
        if !self.mul.allows_beyond_pin() {
            return None;
        }
        let plan = super::synth::plan(self.cpu, self.price, vt.size_in_bits(), c)?;
        let taken = self.mul.allows(true).then_some(plan);
        if taken.is_some() {
            self.mul.note_synthesized();
        }
        taken
    }

    fn sub_of_sign_extended_bool(&self) -> bool {
        self.short
    }

    fn mul_of_halves(
        &self,
        vt: EVT,
    ) -> Option<llrm_codegen::backend::selectiondag::dag_combiner::HalfMul> {
        use super::generated::ids::opcode;
        (self.half_mul && vt == EVT::I32).then_some(llrm_codegen::backend::selectiondag::dag_combiner::HalfMul {
            half: EVT::I16,
            unsigned: u32::from(opcode::UMULd32d16),
            signed: u32::from(opcode::SMULd32d16),
            has_full_multiply: !matches!(self.cpu, Cpu::M68000 | Cpu::M68010),
        })
    }

    fn is_indexed_legal(
        &self,
        _is_load: bool,
        vt: EVT,
        _post: bool,
    ) -> bool {
        self.indexed && super::indexed::is_indexed_legal(vt)
    }

    /// `isLegalAddressingMode`: `(d16,An)` and `(An,Xn)`, as the target's cost
    /// model answers.
    fn is_legal_addressing_mode(
        &self,
        _vt: EVT,
        base_offs: i64,
        scale: i64,
    ) -> bool {
        use llrm_mir::tti::Tti;
        crate::machine::M68k::default().is_legal_addressing_mode(false, base_offs, true, scale)
    }

    fn indexed_address_parts(
        &self,
        dag: &SelectionDAG,
        access: SDValue,
        address: SDValue,
        post: bool,
    ) -> Option<(SDValue, i64)> {
        super::indexed::indexed_address_parts(dag, access, address, post)
    }

    /// The pin canonicalizes `x >= c` to `x > c - 1`; `slt 0` then folds to
    /// `bmi`.
    fn canonicalize_ge_le(&self) -> bool {
        true
    }
}

impl Hooks for M68kLowering {
    /// `getStackPointerRegisterToSaveRestore` and the frame's stack alignment,
    /// which the shared expansion of `DYNAMIC_STACKALLOC`, `STACKSAVE` and
    /// `STACKRESTORE` reads: the node sequence the pin's
    /// `LowerDYNAMIC_STACKALLOC` builds (`M68kISelLowering.cpp:3362`), with the
    /// alignment the frames keep (`frame::SHIPPED_STACK_ALIGN`, the pin's
    /// is 8: see there).
    fn stack_pointer(&self) -> Option<(llrm_lir::registers::RegId, u64)> {
        Some((crate::reg::SP, crate::frame::SHIPPED_STACK_ALIGN as u64))
    }

    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::SetCC => lower_setcc(dag, node, self.cpu),
            ISD::SetCCCarry => Some(lower_setcc_carry(dag, node)),
            ISD::ShlParts => Some(lower_shift_left_parts(dag, node)),
            ISD::SrlParts => Some(lower_shift_right_parts(dag, node, false)),
            ISD::SraParts => Some(lower_shift_right_parts(dag, node, true)),
            ISD::Select => Some(lower_select(dag, node, self.cpu)),
            ISD::Abs => Some(lower_abs(dag, node, self.cpu)),
            ISD::BrCond => Some(lower_brcond(dag, node, self.cpu)),
            ISD::GlobalAddress => Some(super::call::lower_global_address(dag, node, self.absolute_data)),
            ISD::JumpTable => Some(super::call::lower_jump_table(dag, node, self.absolute_data)),
            ISD::VAStart => lower_vastart(dag, node),
            ISD::UMulLoHi | ISD::SMulLoHi => Some(lower_wide_multiply(dag, node)),
            ISD::SAddO | ISD::UAddO | ISD::SSubO | ISD::USubO | ISD::SMulO | ISD::UMulO => lower_xalu(dag, node),
            ISD::AddC | ISD::AddE | ISD::SubC | ISD::SubE => lower_carry_chain(dag, node),
            _ => None,
        }
    }

    /// `makeLibCall` through `LowerCall`: the routine's address is a target
    /// external symbol, the chain is the entry node (the result is the value,
    /// nothing orders the call after a store).
    fn make_lib_call(
        &mut self,
        dag: &mut SelectionDAG,
        name: &'static str,
        args: &[SDValue],
        ret: EVT,
        signed: bool,
    ) -> Option<SDValue> {
        self.make_lib_call_parts(dag, name, args, &[ret], signed)?.first().copied()
    }

    fn make_lib_call_parts(
        &mut self,
        dag: &mut SelectionDAG,
        name: &'static str,
        args: &[SDValue],
        rets: &[EVT],
        signed: bool,
    ) -> Option<Vec<SDValue>> {
        use super::call::{Ext, In, Out};
        let ext = if signed { Ext::Sign } else { Ext::Zero };
        let out = |value: SDValue, dag: &SelectionDAG| Out {
            value,
            ext,
            pointer: false,
            arg_vt: dag.value_type(value),
            split: false,
            variadic: false,
            sret: false,
            byval: None,
        };
        // `getCopyToParts`: a 64-bit argument is two words, the high one first
        // on a big-endian target.
        let mut outs: Vec<Out> = Vec::new();
        for &value in args {
            if dag.value_type(value) == EVT::I64 {
                let [low, high] = split_words(dag, value);
                outs.push(out(high, dag));
                outs.push(out(low, dag));
            } else {
                outs.push(out(value, dag));
            }
        }
        let word = In {
            vt: EVT::I32,
            pointer: false,
            arg_vt: EVT::I32,
            split: false,
            sret: false,
            ext: Ext::Any,
            byval: None,
        };
        let mut ins: Vec<In> = Vec::new();
        for &ret in rets {
            if ret == EVT::I64 {
                ins.extend([word, word]);
            } else {
                ins.push(In {
                    vt: ret,
                    pointer: false,
                    arg_vt: ret,
                    split: false,
                    sret: false,
                    ext: Ext::Any,
                    byval: None,
                });
            }
        }
        let callee = dag.get_target_external_symbol(EVT::I32, name);
        let root = dag.entry_node();
        let (_chain, values) = super::call::M68kCalling::default().lower_call(dag, root, callee, &outs, &ins);
        if values.len() != ins.len() {
            return None;
        }
        // a 64-bit result: the high word in `d0`, the low in `d1`
        let mut joined = Vec::new();
        let mut next = values.into_iter();
        for &ret in rets {
            if ret == EVT::I64 {
                let (high, low) = (next.next()?, next.next()?);
                joined.push(dag.get_node(ISD::BuildPair, EVT::I64, &[low, high]));
            } else {
                joined.push(next.next()?);
            }
        }
        Some(joined)
    }

    fn little_endian(&self) -> bool {
        false
    }
}

/// The low and the high word of a 64-bit value.
fn split_words(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> [SDValue; 2] {
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[value]);
    let by = dag.get_constant(32, EVT::I32);
    let upper = dag.get_node(ISD::Srl, EVT::I64, &[value, by]);
    [low, dag.get_node(ISD::Truncate, EVT::I32, &[upper])]
}

/// `LowerXALUO` (with `lowerOverflowArithmetic`): the arithmetic as one node
/// that also sets the codes, and the overflow as a `SETCC` of them.
///
/// DEVIATE: a multiply of 8 or 16 bits is the wide multiply of
/// `expandMULO`, not the pin's `LowerXALUO`: the pin promotes it and reports
/// the constant zero as its overflow, so `255 * 2` in a byte overflows
/// nothing. Exit: never, while the pin does.
fn lower_xalu(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    if matches!(dag.opcode(node), ISD::SMulO | ISD::UMulO)
        && dag.value_type(SDValue { node: node.node, res: 0 }) != EVT::I32
    {
        return Some(expand_narrow_mulo(dag, node));
    }
    let result_vt = dag.value_type(SDValue { node: node.node, res: 0 });
    let overflow_vt = dag.value_type(SDValue { node: node.node, res: 1 });
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let (base, code) = match dag.opcode(node) {
        ISD::SAddO => (m68kisd::ADD, cond::VS),
        ISD::UAddO => (m68kisd::ADD, cond::CS),
        ISD::SSubO => (m68kisd::SUB, cond::VS),
        ISD::USubO => (m68kisd::SUB, cond::CS),
        ISD::UMulO => (m68kisd::UMUL, cond::VS),
        ISD::SMulO => (m68kisd::SMUL, cond::VS),
        other => unreachable!("not an overflow operation: {other:?}"),
    };
    let arith = dag.get_node_multi(base, &[result_vt, EVT::I8], &[lhs, rhs]);
    let code = cc_operand(dag, code);
    let overflow = dag.get_node(m68kisd::SETCC, overflow_vt, &[code, SDValue { node: arith.node, res: 1 }]);
    Some(dag.get_node_multi(
        ISD::MergeValues,
        &[result_vt, overflow_vt],
        &[SDValue { node: arith.node, res: 0 }, overflow],
    ))
}

/// `LowerADDC_ADDE_SUBC_SUBE`: the arithmetic as `add`/`sub`, and with the
/// carry in as `addx`/`subx`; the carry out is the codes, not glue. A type the
/// legalizer has not made legal is left to it.
fn lower_carry_chain(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let result_vt = dag.value_type(SDValue { node: node.node, res: 0 });
    if !matches!(result_vt, EVT::I8 | EVT::I16 | EVT::I32) {
        return None;
    }
    let (base, extra) = match dag.opcode(node) {
        ISD::AddC => (m68kisd::ADD, false),
        ISD::AddE => (m68kisd::ADDX, true),
        ISD::SubC => (m68kisd::SUB, false),
        ISD::SubE => (m68kisd::SUBX, true),
        other => unreachable!("not a carry chain operation: {other:?}"),
    };
    let mut operands = vec![dag.operand(node, 0), dag.operand(node, 1)];
    if extra {
        operands.push(dag.operand(node, 2));
    }
    Some(dag.get_node_multi(base, &[result_vt, EVT::I8], &operands))
}

/// `UMUL_LOHI` / `SMUL_LOHI` of two `i32`s as `mulu.l` / `muls.l` with a
/// register pair: the low and the high half, in the order of the node's
/// results.
fn lower_wide_multiply(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let name = if dag.opcode(node) == ISD::SMulLoHi { "SMUL64" } else { "UMUL64" };
    let code = super::widemul::code_of(name).expect("a widening multiply");
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    dag.get_node_multi(ISD::Machine(u32::from(code)), &[EVT::I32, EVT::I32], &[lhs, rhs])
}

/// `TargetLowering::expandMULO` through the type twice as wide: the product
/// of the extended operands, whose low half is the result and whose high half
/// is empty (unsigned) or the sign of the result (signed) when nothing
/// overflowed.
fn expand_narrow_mulo(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let signed = dag.opcode(node) == ISD::SMulO;
    let result_vt = dag.value_type(SDValue { node: node.node, res: 0 });
    let overflow_vt = dag.value_type(SDValue { node: node.node, res: 1 });
    let bits = result_vt.size_in_bits();
    let wide = EVT::integer(bits * 2);
    let extend = if signed { ISD::SignExtend } else { ISD::ZeroExtend };
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let (lhs, rhs) = (dag.get_node(extend, wide, &[lhs]), dag.get_node(extend, wide, &[rhs]));
    let product = dag.get_node(ISD::Mul, wide, &[lhs, rhs]);
    let result = dag.get_node(ISD::Truncate, result_vt, &[product]);
    let by = dag.get_constant(u128::from(bits), EVT::I8);
    let high = dag.get_node(ISD::Srl, wide, &[product, by]);
    let high = dag.get_node(ISD::Truncate, result_vt, &[high]);
    let expected = if signed {
        let sign = dag.get_constant(u128::from(bits - 1), EVT::I8);
        dag.get_node(ISD::Sra, result_vt, &[result, sign])
    } else {
        dag.get_constant(0, result_vt)
    };
    let overflow = dag.get_setcc(overflow_vt, high, expected, CondCode::SetNe);
    dag.get_node_multi(ISD::MergeValues, &[result_vt, overflow_vt], &[result, overflow])
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

/// `LowerBRCOND` and `LowerSELECT` look past the extension of a boolean to the
/// value it extends (`(and (setcc_carry ...), 1)`, a truncate with zero high
/// bits). A promoted `i1` condition is `sign_extend_inreg x, i1`, which is
/// non-zero exactly when bit 0 of `x` is: the condition is `btst #0,x` of a
/// long, else the test of `x & 1`, so no `neg` is made.
fn look_past_boolean(
    dag: &mut SelectionDAG,
    cond_value: SDValue,
) -> SDValue {
    let one_bit = dag.opcode(cond_value) == ISD::SignExtendInReg
        && matches!(dag.node(dag.operand(cond_value, 1)).data, NodeData::ValueType(vt) if vt.size_in_bits() == 1);
    if !one_bit {
        return cond_value;
    }
    let mut value = dag.operand(cond_value, 0);
    // Bit 0 of `x ^ c` is bit 0 of `x` flipped where `c` has it set: a
    // branch on the opposite of a boolean (`not`, `xor 1`) reads `x` itself
    // with the opposite condition.
    let mut flipped = false;
    while dag.opcode(value) == ISD::Xor
        && let Some(mask) = dag.constant(dag.operand(value, 1))
    {
        flipped ^= mask & 1 == 1;
        value = dag.operand(value, 0);
    }
    let vt = dag.value_type(value);
    if vt == EVT::I32 {
        let bit_no = dag.get_constant(0, EVT::I32);
        return bit_test(dag, value, bit_no, if flipped { CondCode::SetEq } else { CondCode::SetNe });
    }
    // `btst` takes a long: a byte would first be zero extended
    let one = dag.get_constant(1, vt);
    let bit = dag.get_node(ISD::And, vt, &[value, one]);
    let code = cc_operand(dag, if flipped { cond::EQ } else { cond::NE });
    let flags = emit_test(dag, bit, if flipped { cond::EQ } else { cond::NE });
    dag.get_node(m68kisd::SETCC, EVT::I8, &[code, flags])
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

/// The bytes every stack argument takes, which is also the most a `va_arg`
/// aligns its list pointer to (gcc packs each in a 4-byte slot; the pin's M68k
/// sets neither `getMinStackArgumentAlignment` nor a cap).
pub const VA_SLOT: u32 = 4;

/// `getBitTestCondition`: the `BTST` of bit `bit_no` of `src`, read as `cc`
/// (`EQ` or `NE`); `BTST` sets `Z` when the bit is clear, as an `and` does.
fn bit_test(
    dag: &mut SelectionDAG,
    src: SDValue,
    bit_no: SDValue,
    cc: CondCode,
) -> SDValue {
    let mut src = src;
    if matches!(dag.value_type(src), EVT::I8 | EVT::I16) {
        src = dag.get_node(ISD::AnyExtend, EVT::I32, &[src]);
    }
    let vt = dag.value_type(src);
    // a constant bit number is an immediate (`btst #k`, see `btst.rs`)
    let bit_no = match dag.constant(bit_no) {
        Some(bits) => dag.get_constant_with(bits, vt, true, false),
        None if dag.value_type(bit_no) == vt => bit_no,
        None => dag.get_node(ISD::AnyExtend, vt, &[bit_no]),
    };
    let test = dag.get_node(m68kisd::BTST, EVT::I8, &[src, bit_no]);
    let code = cc_operand(dag, if cc == CondCode::SetEq { cond::EQ } else { cond::NE });
    dag.get_node(m68kisd::SETCC, EVT::I8, &[code, test])
}

/// Whether `btst #k,Dn` takes no more bytes and no more clocks than
/// `and.l #imm,Dn` on `cpu`, read from the timing tables (the 68000 and 68010
/// 10 against 14 clk, the 68020 and 68030 4 against 6, the 68040 and 68060 1
/// against 1; 4 B against 6).
pub fn btst_beats_and_immediate(cpu: Cpu) -> bool {
    use crate::timing::cost;
    static BEATS: std::sync::LazyLock<[bool; 6]> = std::sync::LazyLock::new(|| {
        [Cpu::M68000, Cpu::M68010, Cpu::M68020, Cpu::M68030, Cpu::M68040, Cpu::M68060].map(|cpu| {
            let clocks = |text: &str, bytes: u32| cost(cpu, text, bytes, false).expect("a priced instruction").clocks;
            clocks("btst #23, %d0", 4) <= clocks("and.l #8388608, %d0", 6)
        })
    });
    BEATS[cpu as usize]
}

/// Whether `tst.w`/`tst.b Dn` takes no more bytes and no more clocks than
/// `btst #k,Dn` on `cpu`, read from the timing tables (2 B against 4).
fn tst_beats_btst(cpu: Cpu) -> bool {
    use crate::timing::cost;
    static BEATS: std::sync::LazyLock<[bool; 6]> = std::sync::LazyLock::new(|| {
        [Cpu::M68000, Cpu::M68010, Cpu::M68020, Cpu::M68030, Cpu::M68040, Cpu::M68060].map(|cpu| {
            let clocks = |text: &str, bytes: u32| cost(cpu, text, bytes, false).expect("a priced instruction").clocks;
            let btst = clocks("btst #15, %d0", 4);
            clocks("tst.w %d0", 2) <= btst && clocks("tst.b %d0", 2) <= btst
        })
    });
    BEATS[cpu as usize]
}

/// DEVIATE M68K-BTST-SIGN (owner m68k-lsr-addressing, exit 2026-11-30): the
/// pin has no counterpart (its `LowerAndToBTST` leaves a constant mask that
/// fits a `TEST` alone). A mask that is the sign bit of the low byte or word,
/// `(x & 0x80) != 0` or `(x & 0x8000) != 0` of an `i32`, is the sign of that
/// part: `tst.b`/`tst.w Dn` (2 B, no copy) and a sign condition, where
/// `btst #k,Dn` is 4 B and `and.l #imm` 6 B and a copy. Priced from the
/// tables. Any other single bit stays [`lower_and_to_btst`]'s.
fn lower_and_to_narrow_sign_test(
    dag: &mut SelectionDAG,
    and: SDValue,
    cc: CondCode,
    cpu: Cpu,
) -> Option<SDValue> {
    if dag.value_type(and) != EVT::I32 || !tst_beats_btst(cpu) {
        return None;
    }
    let mask = dag.constant(dag.operand(and, 1))?;
    let vt = match mask {
        0x80 => EVT::I8,
        0x8000 => EVT::I16,
        _ => return None,
    };
    // `x & sign == 0` is `part >= 0`; `!= 0` is `part < 0`
    let part = dag.get_node(ISD::Truncate, vt, &[dag.operand(and, 0)]);
    let mut zero = dag.get_constant(0, vt);
    let code = translate_cc(dag, if cc == CondCode::SetEq { CondCode::SetGe } else { CondCode::SetLt }, &mut zero);
    let flags = emit_cmp(dag, part, zero, code);
    let code = cc_operand(dag, code);
    Some(dag.get_node(m68kisd::SETCC, EVT::I8, &[code, flags]))
}

/// `LowerAndToBTST`: `(1 << n) & x` and `(x >> n) & 1` against zero are a bit
/// test.
fn lower_and_to_btst(
    dag: &mut SelectionDAG,
    and: SDValue,
    cc: CondCode,
    cpu: Cpu,
) -> Option<SDValue> {
    if dag.value_type(and) != EVT::I32 {
        return None;
    }
    let (mut op0, mut op1) = (dag.operand(and, 0), dag.operand(and, 1));
    if dag.opcode(op1) == ISD::Shl {
        std::mem::swap(&mut op0, &mut op1);
    }
    if dag.opcode(op0) == ISD::Shl {
        if dag.constant(dag.operand(op0, 0)) == Some(1) {
            return Some(bit_test(dag, op1, dag.operand(op0, 1), cc));
        }
        return None;
    }
    let mask = dag.constant(op1)?;
    if mask == 1 && dag.opcode(op0) == ISD::Srl {
        return Some(bit_test(dag, dag.operand(op0, 0), dag.operand(op0, 1), cc));
    }
    // DEVIATE M68K-BTST (owner m68k-lsr-addressing, exit 2026-11-30): the pin
    // takes a constant mask to BTST only when the immediate does not fit a
    // `TEST` (`!isUInt<32>`, M68kISelLowering.cpp#L1731), never at i32. A
    // single bit below the sign bit is `btst #k,Dn` (4 B, no copy, flags Z
    // only: this node's users are `EQ`/`NE`) where the pin's `and.l #imm`
    // takes 6 B and a copy of a live value. Priced per CPU from the tables;
    // bit 31 stays the sign test (`tst.l`, 2 B).
    if mask.is_power_of_two() && mask < 1 << 31 && btst_beats_and_immediate(cpu) {
        let bit_no = dag.get_constant(u128::from(mask.trailing_zeros()), EVT::I32);
        return Some(bit_test(dag, op0, bit_no, cc));
    }
    None
}

/// `LowerSETCC`, integer part.
///
/// `LowerSETCC`.
fn lower_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
    cpu: Cpu,
) -> Option<SDValue> {
    let (op0, mut op1) = (dag.operand(node, 0), dag.operand(node, 1));
    if !dag.value_type(op0).is_integer() {
        return None;
    }
    let cc = cond_of(dag, dag.operand(node, 2));
    // `(1 << n) & x == 0`, `((x >> n) & 1) != 0`: a bit test
    if matches!(cc, CondCode::SetEq | CondCode::SetNe)
        && dag.is_null_constant(op1)
        && dag.opcode(op0) == ISD::And
        && dag.has_one_use(op0.node)
        && let Some(test) =
            lower_and_to_narrow_sign_test(dag, op0, cc, cpu).or_else(|| lower_and_to_btst(dag, op0, cc, cpu))
    {
        return Some(test);
    }
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

/// `LowerSETCCCARRY`: the comparison of `lhs` and `rhs` with a borrow in, as
/// `subx` over the carry turned into the extend flag (`carry + -1`).
fn lower_setcc_carry(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (lhs, rhs, carry) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let cc = cond_of(dag, dag.operand(node, 3));
    let mut unchanged = rhs;
    let code = translate_cc(dag, cc, &mut unchanged);
    let carry_vt = dag.value_type(carry);
    let minus_one = dag.get_constant(u128::MAX >> (128 - carry_vt.size_in_bits()), carry_vt);
    let carry = dag.get_node_multi(m68kisd::ADD, &[carry_vt, EVT::I8], &[carry, minus_one]);
    let compared = dag.get_node_multi(
        m68kisd::SUBX,
        &[dag.value_type(lhs), EVT::I8],
        &[lhs, rhs, SDValue { node: carry.node, res: 1 }],
    );
    let code = cc_operand(dag, code);
    dag.get_node(m68kisd::SETCC, EVT::I8, &[code, SDValue { node: compared.node, res: 1 }])
}

/// `LowerShiftLeftParts`: a 64-bit shift left of `(lo, hi)` by `shamt` in
/// 32-bit registers, as a select on `shamt < 32`.
fn lower_shift_left_parts(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (lo, hi, shamt) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let vt = dag.value_type(lo);
    let zero = dag.get_constant(0, vt);
    let one = dag.get_constant(1, vt);
    let minus_size = dag.get_constant(0xFFFF_FFE0, vt);
    let size_minus_1 = dag.get_constant(31, vt);
    let shamt_minus_size = dag.get_node(ISD::Add, vt, &[shamt, minus_size]);
    let size_minus_1_shamt = dag.get_node(ISD::Xor, vt, &[size_minus_1, shamt]);
    let lo_true = dag.get_node(ISD::Shl, vt, &[lo, shamt]);
    let right_1 = dag.get_node(ISD::Srl, vt, &[lo, one]);
    let right = dag.get_node(ISD::Srl, vt, &[right_1, size_minus_1_shamt]);
    let left_hi = dag.get_node(ISD::Shl, vt, &[hi, shamt]);
    let hi_true = dag.get_node(ISD::Or, vt, &[left_hi, right]);
    let hi_false = dag.get_node(ISD::Shl, vt, &[lo, shamt_minus_size]);
    let below = dag.get_setcc(EVT::I8, shamt_minus_size, zero, CondCode::SetLt);
    let lo = dag.get_node(ISD::Select, vt, &[below, lo_true, zero]);
    let hi = dag.get_node(ISD::Select, vt, &[below, hi_true, hi_false]);
    dag.get_node_multi(ISD::MergeValues, &[vt, vt], &[lo, hi])
}

/// `LowerShiftRightParts`: the same for a shift right, arithmetic or logical.
fn lower_shift_right_parts(
    dag: &mut SelectionDAG,
    node: SDValue,
    arithmetic: bool,
) -> SDValue {
    let (lo, hi, shamt) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let vt = dag.value_type(lo);
    let right = if arithmetic { ISD::Sra } else { ISD::Srl };
    let zero = dag.get_constant(0, vt);
    let one = dag.get_constant(1, vt);
    let minus_size = dag.get_constant(0xFFFF_FFE0, vt);
    let size_minus_1 = dag.get_constant(31, vt);
    let shamt_minus_size = dag.get_node(ISD::Add, vt, &[shamt, minus_size]);
    let size_minus_1_shamt = dag.get_node(ISD::Xor, vt, &[size_minus_1, shamt]);
    let right_lo = dag.get_node(ISD::Srl, vt, &[lo, shamt]);
    let left_hi_1 = dag.get_node(ISD::Shl, vt, &[hi, one]);
    let left_hi = dag.get_node(ISD::Shl, vt, &[left_hi_1, size_minus_1_shamt]);
    let lo_true = dag.get_node(ISD::Or, vt, &[right_lo, left_hi]);
    let hi_true = dag.get_node(right, vt, &[hi, shamt]);
    let lo_false = dag.get_node(right, vt, &[hi, shamt_minus_size]);
    let hi_false = if arithmetic { dag.get_node(ISD::Sra, vt, &[hi, size_minus_1]) } else { zero };
    let below = dag.get_setcc(EVT::I8, shamt_minus_size, zero, CondCode::SetLt);
    let lo = dag.get_node(ISD::Select, vt, &[below, lo_true, lo_false]);
    let hi = dag.get_node(ISD::Select, vt, &[below, hi_true, hi_false]);
    dag.get_node_multi(ISD::MergeValues, &[vt, vt], &[lo, hi])
}

/// `M68k::GetOppositeBranchCondition`: the codes pair up as `(2n, 2n + 1)`:
/// `HI`/`LS`, `CC`/`CS`, `NE`/`EQ`, ...
fn opposite(code: u128) -> u128 {
    code ^ 1
}

/// `abs x` as `x < 0 ? 0 - x : x`, selected like any other `select`.
fn lower_abs(
    dag: &mut SelectionDAG,
    node: SDValue,
    cpu: Cpu,
) -> SDValue {
    let vt = dag.value_type(node);
    let x = dag.operand(node, 0);
    let zero = dag.get_constant(0, vt);
    let negated = dag.get_node(ISD::Sub, vt, &[zero, x]);
    let negative = dag.get_setcc(EVT::I8, x, zero, CondCode::SetLt);
    let select = dag.get_node(ISD::Select, vt, &[negative, negated, x]);
    lower_select(dag, select, cpu)
}

/// `LowerSELECT`, the general case: `CMOV false, true, cc, flags`.
fn lower_select(
    dag: &mut SelectionDAG,
    node: SDValue,
    cpu: Cpu,
) -> SDValue {
    let (mut cond_value, op1, op2) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    if dag.opcode(cond_value) == ISD::SetCC {
        if let Some(lowered) = lower_setcc(dag, cond_value, cpu) {
            cond_value = lowered;
        }
    }
    cond_value = look_past_boolean(dag, cond_value);
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
    cpu: Cpu,
) -> SDValue {
    let (chain, mut cond_value, dest) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    if dag.opcode(cond_value) == ISD::SetCC {
        if let Some(lowered) = lower_setcc(dag, cond_value, cpu) {
            cond_value = lowered;
        }
    }
    cond_value = look_past_boolean(dag, cond_value);
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

/// `M68kTargetLowering::LowerVASTART`: `va_start` stores the address of the
/// first variadic argument's slot (`VarArgsFrameIndex`) through the list.
fn lower_vastart(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    llrm_codegen::backend::selectiondag::target_calling::lower_vastart_frame_index(dag, node, EVT::I32, 2)
}
