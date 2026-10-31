//! What the target does with each operation on each type: the pin's
//! `TargetLoweringBase` action tables and `X86TargetLowering`'s constructor,
//! for i486 and P5.
//!
//! # How it works
//! [`TargetLowering`] holds the type half ([`TargetLoweringInfo`]) and four
//! tables: (operation, type) to [`Action`], (extending load, type, memory
//! type), (type, memory type) for truncating stores, and (condition code,
//! type). Everything starts `Legal`; `init_actions` makes the pin's
//! `initActions` defaults and `x86_32` the constructor's calls, in the pin's
//! order, because a later call overwrites an earlier one. `operation_action`
//! is `getOperationAction`: an integer with no simple type is `Expand`.
//! `type_to_promote_to` is `getTypeToPromoteTo`: the recorded type, else the
//! next wider legal type that handles the operation.
//!
//! # Runs
//! Built once per target; read by LegalizeDAG (B1) and the type legalizer.
//!
//! The constructor's arms are translated where a node of [`ISD`] exists and
//! the arm applies to i486/P5 (no SSE, no CMOV before P6, no BMI, POPCNT or
//! LZCNT, x87 on, 32-bit). Not yet translated because [`ISD`] has no such
//! node: `ABDS`/`ABDU`, the `STRICT_*` family, `FSIN`/`FCOS`/`FPOW`-style
//! math, `SHL_PARTS`/`SRA_PARTS`/`SRL_PARTS`, `SETCCCARRY`/`SADDO_CARRY`,
//! atomics, `VASTART`/`VAARG`, `JumpTable`/`BlockAddress`/`GlobalTLSAddress`,
//! `LRINT`, `ADDRSPACECAST`, `INTRINSIC_*`. Each is added with the piece
//! that needs it (B1), in the position the constructor sets it. Arms for
//! SSE, AVX, MMX, vectors, `f16`/`f128` and `is64Bit` are N/A.
//!
//! DEVIATE SD32: the tables are keyed by [`MVT`]; `Far` has no entry and is
//! always expanded by the type legalizer. Reason: SD2. Exit: none. Owner
//! sdag-b. Review 2026-11-13.
//!
//! # See also
//! LLVM's `initActions`
//! ([L764](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetLoweringBase.cpp#L764))
//! and `X86TargetLowering::X86TargetLowering`
//! ([L131](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L131)).

use llrm_support::hash::HashMap;

use super::isd::{CondCode, ISD};
use super::lowering::TargetLoweringInfo;
use super::mvt::{EVT, MVT};

/// `TargetLowering::LegalizeAction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Legal,
    Promote,
    Expand,
    LibCall,
    Custom,
}

/// The CPU features the translated arms test.
#[derive(Clone, Copy, Debug, Default)]
pub struct Features {
    pub cmov: bool,
    pub bmi: bool,
    pub lzcnt: bool,
    pub popcnt: bool,
    pub movbe: bool,
    pub shld_slow: bool,
    /// `hasSSE2()`: f32 and f64 live in xmm registers (m64; D64-4).
    pub sse2: bool,
}

/// `ISD::LoadExtType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoadExt {
    ExtLoad,
    SExtLoad,
    ZExtLoad,
}

const INTS: [MVT; 6] = [MVT::I1, MVT::I8, MVT::I16, MVT::I32, MVT::I64, MVT::I128];
const FLOATS: [MVT; 3] = [MVT::F32, MVT::F64, MVT::F80];
const ALL: [MVT; 10] =
    [MVT::I1, MVT::I8, MVT::I16, MVT::I32, MVT::I64, MVT::I128, MVT::F32, MVT::F64, MVT::F80, MVT::Other];

pub struct TargetLowering {
    pub types: TargetLoweringInfo,
    operations: HashMap<(ISD, MVT), Action>,
    promoted_to: HashMap<(ISD, MVT), MVT>,
    load_ext: HashMap<(LoadExt, MVT, MVT), Action>,
    trunc_store: HashMap<(MVT, MVT), Action>,
    cond_code: HashMap<(CondCode, MVT), Action>,
    /// `addLegalFPImmediate`: the float constants an instruction makes.
    legal_fp_immediates: Vec<(MVT, u128)>,
    /// `ShouldShrinkFPConstant` is false for `f64`: with SSE2 `cvtss2sd` costs
    /// more than a straight `movsd`.
    keeps_f64_constants: bool,
    /// `canUseCMOV`: the target has the conditional move.
    cmov: bool,
    /// `getLibcallName` of the integer operations (`RTLIB::MUL_I32` and the
    /// like): the runtime routine a `LibCall` action calls. Empty unless the
    /// target fills it.
    libcall_names: HashMap<(ISD, MVT), &'static str>,
}

impl TargetLowering {
    /// Every action `Legal`: a table nothing has been set in.
    pub fn empty(types: TargetLoweringInfo) -> TargetLowering {
        TargetLowering {
            types,
            operations: HashMap::default(),
            promoted_to: HashMap::default(),
            load_ext: HashMap::default(),
            trunc_store: HashMap::default(),
            cond_code: HashMap::default(),
            legal_fp_immediates: Vec::new(),
            keeps_f64_constants: false,
            cmov: false,
            libcall_names: HashMap::default(),
        }
    }

    /// `setLibcallName` for the integer operation `op` of `vt`.
    pub fn set_libcall_name(
        &mut self,
        op: ISD,
        vt: MVT,
        name: &'static str,
    ) {
        self.libcall_names.insert((op, vt), name);
    }

    /// `getLibcallName` of the routine `op` of `vt` calls, if the target has
    /// one.
    pub fn libcall_name(
        &self,
        op: ISD,
        vt: EVT,
    ) -> Option<&'static str> {
        let EVT::Simple(vt) = vt else { return None };
        self.libcall_names.get(&(op, vt)).copied()
    }

    pub fn set_operation_action(
        &mut self,
        op: ISD,
        vt: MVT,
        action: Action,
    ) {
        self.operations.insert((op, vt), action);
    }

    fn set_ops(
        &mut self,
        ops: &[ISD],
        vts: &[MVT],
        action: Action,
    ) {
        for &op in ops {
            for &vt in vts {
                self.set_operation_action(op, vt, action);
            }
        }
    }

    /// `setOperationPromotedToType`.
    pub fn set_operation_promoted_to_type(
        &mut self,
        op: ISD,
        vt: MVT,
        to: MVT,
    ) {
        self.set_operation_action(op, vt, Action::Promote);
        self.promoted_to.insert((op, vt), to);
    }

    pub fn set_load_ext_action(
        &mut self,
        ext: LoadExt,
        vt: MVT,
        mem: MVT,
        action: Action,
    ) {
        self.load_ext.insert((ext, vt, mem), action);
    }

    pub fn set_trunc_store_action(
        &mut self,
        vt: MVT,
        mem: MVT,
        action: Action,
    ) {
        self.trunc_store.insert((vt, mem), action);
    }

    pub fn set_cond_code_action(
        &mut self,
        cc: CondCode,
        vt: MVT,
        action: Action,
    ) {
        self.cond_code.insert((cc, vt), action);
    }

    pub fn add_legal_fp_immediate(
        &mut self,
        vt: MVT,
        bits: u128,
    ) {
        self.legal_fp_immediates.push((vt, bits));
    }

    /// `ShouldShrinkFPConstant`.
    pub fn should_shrink_fp_constant(
        &self,
        vt: MVT,
    ) -> bool {
        vt == MVT::F80 || !self.keeps_f64_constants
    }

    /// `isFPImmLegal`.
    pub fn is_fp_imm_legal(
        &self,
        vt: MVT,
        bits: u128,
    ) -> bool {
        self.legal_fp_immediates.contains(&(vt, bits))
    }

    /// `isMultiStoresCheaperThanBitsMerge`: the pin says so for a float and
    /// an integer half only.
    ///
    /// DEVIATE SD240: a target with the far pointer (m16) says so for two
    /// integer halves too. A far pointer or a `long` stored from its two
    /// words is two word stores, where merging them took `shl; movzx; add`
    /// (QCport -Os: 131 stack arguments built so). Owner w10. Exit: none
    /// while the DAG keeps `BUILD_PAIR`. Review 2026-12-10.
    pub fn is_multi_stores_cheaper_than_bits_merge(
        &self,
        lo: EVT,
        hi: EVT,
    ) -> bool {
        (lo.is_floating_point() && hi.is_integer())
            || (lo.is_integer() && hi.is_floating_point())
            || (self.types.has_far_pointer() && lo.is_integer() && hi.is_integer())
    }

    /// `getOperationAction`: a type with no simple form is expanded.
    pub fn operation_action(
        &self,
        op: ISD,
        vt: EVT,
    ) -> Action {
        match vt {
            EVT::Int(_) => Action::Expand,
            EVT::Simple(vt) => self.operations.get(&(op, vt)).copied().unwrap_or(Action::Legal),
        }
    }

    pub fn load_ext_action(
        &self,
        ext: LoadExt,
        vt: MVT,
        mem: MVT,
    ) -> Action {
        self.load_ext.get(&(ext, vt, mem)).copied().unwrap_or(Action::Legal)
    }

    pub fn trunc_store_action(
        &self,
        vt: MVT,
        mem: MVT,
    ) -> Action {
        self.trunc_store.get(&(vt, mem)).copied().unwrap_or(Action::Legal)
    }

    pub fn cond_code_action(
        &self,
        cc: CondCode,
        vt: MVT,
    ) -> Action {
        self.cond_code.get(&(cc, vt)).copied().unwrap_or(Action::Legal)
    }

    /// `Subtarget.canUseCMOV()`.
    pub fn can_use_cmov(&self) -> bool {
        self.cmov
    }

    /// `isOperationLegal`.
    pub fn is_operation_legal(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        self.types.is_type_legal(vt) && self.operation_action(op, vt) == Action::Legal
    }

    pub fn is_operation_legal_or_custom(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        self.types.is_type_legal(vt) && matches!(self.operation_action(op, vt), Action::Legal | Action::Custom)
    }

    /// `getTypeToPromoteTo`: the recorded type, else the next wider integer
    /// (or float) type that is legal and handles `op`.
    pub fn type_to_promote_to(
        &self,
        op: ISD,
        vt: MVT,
    ) -> MVT {
        if let Some(&to) = self.promoted_to.get(&(op, vt)) {
            return to;
        }
        let ladder: &[MVT] = if INTS.contains(&vt) { &INTS } else { &FLOATS };
        let at = ladder.iter().position(|&candidate| candidate == vt).expect("promoting a non-numeric type");
        ladder[at + 1..]
            .iter()
            .copied()
            .find(|&wider| self.is_operation_legal_or_custom(op, wider.into()))
            .expect("no wider type handles the operation")
    }

    /// `TargetLoweringBase::initActions`, for the opcodes [`ISD`] has.
    pub fn init_actions(&mut self) {
        use ISD::*;
        for vt in ALL {
            self.set_ops(
                &[
                    SMin,
                    SMax,
                    UMin,
                    UMax,
                    Abs,
                    AbsMinPoison,
                    FShl,
                    FShr,
                    SAddSat,
                    UAddSat,
                    SSubSat,
                    USubSat,
                    SMulFix,
                    UMulFix,
                    SMulFixSat,
                    UMulFixSat,
                    SShlSat,
                    UShlSat,
                ],
                &[vt],
                Action::Expand,
            );
            self.set_ops(&[SAddO, SSubO, UAddO, USubO, SMulO, UMulO], &[vt], Action::Expand);
            self.set_ops(&[UAddOCarry, USubOCarry, SetCCCarry, AddC, AddE, SubC, SubE], &[vt], Action::Expand);
            self.set_ops(&[CtLzZeroPoison, CtTzZeroPoison, CtLs, BitReverse, Parity], &[vt], Action::Expand);
            // A pair that is a legal register type (an i32 at m16) is built by
            // shifts and an or.
            self.set_ops(&[BuildPair], &[vt], Action::Expand);
        }
        self.set_operation_action(Trap, MVT::Other, Action::Expand);
        self.set_ops(&[ConstantFP], &FLOATS, Action::Expand);
    }

    /// m32 for i486/P5 with the x87 and no SSE: `initActions`, then the
    /// constructor's calls in the pin's order (L179 to L2830, scalar arms).
    /// `ptr` is the pointer type (`PtrVT`).
    pub fn x86_32(
        features: Features,
        ptr: MVT,
    ) -> TargetLowering {
        Self::x86(TargetLoweringInfo::x86_32(), features, ptr, false)
    }

    /// m64: the constructor's `is64Bit()` arms (L183..L533, the scalar integer
    /// ones); the x87 and SSE conversions stay as m32 until D64-4.
    pub fn x86_64(
        features: Features,
        ptr: MVT,
    ) -> TargetLowering {
        Self::x86(TargetLoweringInfo::x86_64(), features, ptr, true)
    }

    /// m16: the same constructor calls over the 386 register file, with `i16`
    /// pointers and the far pointer's type action.
    ///
    /// DEVIATE SD307: the pin has no 16-bit model, so this table is the 32-bit
    /// one, kept to what the old m16 selector covers (`tests/m16_coverage.rs`
    /// lists the functions that must stay selectable), not derived from the
    /// pin. Owner sdag-c. Exit: the m16 shadow against the old selector
    /// matches (11-20); the table is then checked against the old selector's
    /// output until the old selector is deleted at the switch. Review
    /// 2026-12-10.
    pub fn x86_16(
        features: Features,
        ptr: MVT,
    ) -> TargetLowering {
        Self::x86(TargetLoweringInfo::x86_16(), features, ptr, false)
    }

    fn x86(
        types: TargetLoweringInfo,
        features: Features,
        ptr: MVT,
        is64: bool,
    ) -> TargetLowering {
        use Action::*;
        use ISD::*;
        let mut t = TargetLowering::empty(types);
        t.cmov = features.cmov;
        t.init_actions();
        let ints = [MVT::I8, MVT::I16, MVT::I32];
        // `if (VT == MVT::i64 && !Subtarget.is64Bit()) continue;`
        let native: &[MVT] = if is64 { &[MVT::I8, MVT::I16, MVT::I32, MVT::I64] } else { &ints };
        let words: &[MVT] = if is64 { &[MVT::I32, MVT::I64] } else { &[MVT::I32] };
        let ints64 = [MVT::I8, MVT::I16, MVT::I32, MVT::I64];

        // L186..L203
        for vt in INTS {
            t.set_load_ext_action(LoadExt::SExtLoad, vt, MVT::I1, Promote);
        }
        for (vt, mem) in [
            (MVT::I64, MVT::I32),
            (MVT::I64, MVT::I16),
            (MVT::I64, MVT::I8),
            (MVT::I32, MVT::I16),
            (MVT::I32, MVT::I8),
            (MVT::I16, MVT::I8),
            (MVT::F64, MVT::F32),
        ] {
            t.set_trunc_store_action(vt, mem, Expand);
        }
        for vt in FLOATS {
            t.set_cond_code_action(CondCode::SetOEq, vt, Expand);
            t.set_cond_code_action(CondCode::SetUNe, vt, Expand);
        }
        // L205..L240
        if features.cmov {
            t.set_ops(&[Abs], &native[1..], Custom);
        }
        t.set_ops(&[SSubSat], native, Custom);
        let shld = if features.shld_slow { Custom } else { Legal };
        t.set_ops(&[FShl, FShr], &[MVT::I8, MVT::I16], Custom);
        t.set_ops(&[FShl, FShr], words, shld);
        // L508: the double-width shifts (i64 at 32 bits)
        t.set_ops(&[ShlParts, SraParts, SrlParts], words, Custom);
        // L241..L300: the x87 conversions
        t.set_ops(&[UIntToFp], &[MVT::I8, MVT::I16], Promote);
        t.set_ops(&[UIntToFp], &[MVT::I32, MVT::I64], Custom);
        t.set_ops(&[SIntToFp], &[MVT::I8], Promote);
        t.set_ops(&[SIntToFp], &[MVT::I16, MVT::I32, MVT::I64], Custom);
        t.set_ops(&[FpToSInt], &[MVT::I8], Promote);
        t.set_ops(&[FpToSInt], &[MVT::I16, MVT::I32, MVT::I64], Custom);
        t.set_ops(&[FpToUInt], &[MVT::I8, MVT::I16], Promote);
        t.set_ops(&[FpToUInt], &[MVT::I32, MVT::I64], Custom);
        // L300-L303, L850: `lrint` of an x87 value is `fist`
        t.set_ops(&[LRint], &[MVT::F32, MVT::F64, MVT::F80], Custom);
        // an i64 result at 32 bits is expanded after the same `fist`, as
        // FpToSInt's is
        t.set_ops(&[LRint], &[MVT::I64], Custom);
        // L316
        if !features.sse2 {
            t.set_ops(&[BitCast], &[MVT::F32, MVT::I32], Expand);
            if is64 {
                t.set_ops(&[BitCast], &[MVT::F64, MVT::I64], Expand);
            }
        } else if !is64 {
            t.set_operation_action(BitCast, MVT::I64, Custom);
        }
        // L337..L370
        t.set_ops(&[MulHS, MulHU, SDiv, UDiv, SRem, URem], &ints64, Expand);
        // The pin divides an i8 or i16 by a constant with a widened multiply
        // (BuildSDIV finds neither MULHS nor SMUL_LOHI there); only i32
        // has the two-result multiply (`MUL32r`).
        t.set_ops(&[SMulLoHi, UMulLoHi], &[MVT::I8, MVT::I16], Expand);
        // fixed-point multiply is expanded by `expandFixedPointMul` (the base
        // default)
        t.set_ops(&[SMulFix], &[MVT::I8, MVT::I16, MVT::I32], Expand);
        t.set_operation_action(BrJT, MVT::Other, Expand);
        t.set_operation_action(BrCond, MVT::Other, Custom);
        t.set_ops(&[BrCC, SelectCC], &[MVT::F32, MVT::F64, MVT::F80, MVT::I8, MVT::I16, MVT::I32, MVT::I64], Expand);
        if is64 {
            t.set_operation_action(SignExtendInReg, MVT::I32, Legal);
        }
        t.set_ops(&[SignExtendInReg], &[MVT::I16, MVT::I8], Legal);
        t.set_operation_action(SignExtendInReg, MVT::I1, Expand);
        t.set_ops(&[FRem], &FLOATS, LibCall);
        // L372..L468: bit counts
        for op in [CtTz, CtTzZeroPoison] {
            t.set_operation_promoted_to_type(op, MVT::I8, MVT::I32);
            t.set_operation_promoted_to_type(op, MVT::I16, MVT::I32);
        }
        if !features.bmi {
            t.set_ops(&[CtTz], words, Custom);
            t.set_ops(&[CtTzZeroPoison], words, Legal);
        }
        if features.lzcnt {
            t.set_operation_promoted_to_type(CtLz, MVT::I8, MVT::I32);
            t.set_operation_promoted_to_type(CtLzZeroPoison, MVT::I8, MVT::I32);
        } else {
            t.set_ops(&[CtLz, CtLzZeroPoison], native, Custom);
        }
        t.set_ops(&[Parity], native, Custom);
        if features.popcnt {
            t.set_operation_promoted_to_type(CtPop, MVT::I8, MVT::I32);
            t.set_operation_promoted_to_type(CtPop, MVT::I16, MVT::I32);
        } else {
            t.set_ops(&[CtPop], &ints64, Custom);
        }
        // L469..L520
        if !features.movbe {
            t.set_operation_action(BSwap, MVT::I16, Expand);
        }
        t.set_ops(&[Select, SetCC], &FLOATS, Custom);
        t.set_ops(&[Select, SetCC], native, Custom);
        t.set_ops(&[ConstantPool, GlobalAddress, ExternalSymbol, JumpTable], &[ptr], Custom);
        if is64 {
            t.set_ops(&[ConstantPool, GlobalAddress, ExternalSymbol, JumpTable], words, Custom);
        }
        t.set_operation_action(Trap, MVT::Other, Legal);
        t.set_operation_action(DynamicStackAlloc, ptr, Custom);
        // The pin expands these to a stack pointer copy; the same, as Custom.
        t.set_operation_action(StackSave, ptr, Custom);
        t.set_operation_action(StackRestore, MVT::Other, Custom);
        // L559..L566: 32-bit va_list is a pointer
        t.set_operation_action(VAStart, MVT::Other, Custom);
        t.set_ops(&[VAEnd, VACopy], &[MVT::Other], Expand);
        if features.sse2 {
            // L618..L674: f32 and f64 use SSE
            t.set_load_ext_action(LoadExt::ExtLoad, MVT::F64, MVT::F32, Expand);
            for vt in [MVT::F32, MVT::F64] {
                // ANDPD simulates FABS, XORP FNEG, ANDPD and ORPD FCOPYSIGN
                t.set_ops(&[FAbs, FNeg, FCopySign], &[vt], Custom);
                t.set_ops(&[FAdd, FSub], &[vt], Custom);
                // (no FSINCOS node here: nothing builds one)
                t.set_ops(&[FSin, FCos], &[vt], Expand);
            }
            t.set_ops(&[FpExtend], &[MVT::F32, MVT::F64], Custom);
        } else {
            // L720..L760: x87 float classes (no SSE)
            for vt in [MVT::F32, MVT::F64] {
                t.set_ops(&[Undef, Poison], &[vt], Expand);
                t.set_operation_action(FCopySign, vt, Expand);
            }
        }
        // L806..L870: f80
        t.set_ops(&[Undef, Poison], &[MVT::F80], Expand);
        t.set_operation_action(FCopySign, MVT::F80, Expand);
        t.keeps_f64_constants = features.sse2;
        // L764..L860: the x87 loads of 0.0 and 1.0 (`fldz`, `fld1`, and `fchs`)
        // L690..L706: SSE has only +0.0 (`xorps`, `xorpd`)
        for (vt, one, sign) in [
            (MVT::F32, 0x3F80_0000u128, 1u128 << 31),
            (MVT::F64, 0x3FF0_0000_0000_0000, 1u128 << 63),
            (MVT::F80, (0x3FFFu128 << 64) | (1u128 << 63), 1u128 << 79),
        ] {
            if features.sse2 && vt != MVT::F80 {
                t.add_legal_fp_immediate(vt, 0);
                continue;
            }
            for bits in [0, sign, one, one | sign] {
                t.add_legal_fp_immediate(vt, bits);
            }
        }
        // L2757..L2776
        t.set_ops(&[SAddO, UAddO, SSubO, USubO, SMulO, UMulO, UAddOCarry, USubOCarry, SetCCCarry], native, Custom);
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The action tables were std maps: SipHash on every `(op, type)` asked.
    /// Does not compile for any other hasher.
    #[test]
    fn test_action_tables_are_keyed_with_the_fixed_key_hasher() {
        let table = TargetLowering::empty(TargetLoweringInfo::new(&[MVT::I32], false));
        let _: &llrm_support::hash::HashMap<(ISD, MVT), Action> = &table.operations;
    }
}
