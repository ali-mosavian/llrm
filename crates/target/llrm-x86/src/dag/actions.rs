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

use std::collections::HashMap;

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
}

impl TargetLowering {
    /// Every action `Legal`: a table nothing has been set in.
    pub fn empty(types: TargetLoweringInfo) -> TargetLowering {
        TargetLowering {
            types,
            operations: HashMap::new(),
            promoted_to: HashMap::new(),
            load_ext: HashMap::new(),
            trunc_store: HashMap::new(),
            cond_code: HashMap::new(),
            legal_fp_immediates: Vec::new(),
        }
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

    /// `isFPImmLegal`.
    pub fn is_fp_imm_legal(
        &self,
        vt: MVT,
        bits: u128,
    ) -> bool {
        self.legal_fp_immediates.contains(&(vt, bits))
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
    fn init_actions(&mut self) {
        use ISD::*;
        for vt in ALL {
            self.set_ops(
                &[SMin, SMax, UMin, UMax, Abs, FShl, FShr, SAddSat, UAddSat, SSubSat, USubSat],
                &[vt],
                Action::Expand,
            );
            self.set_ops(&[SAddO, SSubO, UAddO, USubO, SMulO, UMulO], &[vt], Action::Expand);
            self.set_ops(&[UAddOCarry, USubOCarry, AddC, AddE, SubC, SubE], &[vt], Action::Expand);
            self.set_ops(&[CtLzZeroPoison, CtTzZeroPoison, CtLs, BitReverse, Parity], &[vt], Action::Expand);
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
        Self::x86(TargetLoweringInfo::x86_32(), features, ptr)
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
        Self::x86(TargetLoweringInfo::x86_16(), features, ptr)
    }

    fn x86(
        types: TargetLoweringInfo,
        features: Features,
        ptr: MVT,
    ) -> TargetLowering {
        use Action::*;
        use ISD::*;
        let mut t = TargetLowering::empty(types);
        t.init_actions();
        let ints = [MVT::I8, MVT::I16, MVT::I32];
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
            t.set_ops(&[Abs], &[MVT::I16, MVT::I32], Custom);
        }
        t.set_ops(&[SSubSat], &ints, Custom);
        let shld = if features.shld_slow { Custom } else { Legal };
        t.set_ops(&[FShl, FShr], &[MVT::I8, MVT::I16], Custom);
        t.set_ops(&[FShl, FShr], &[MVT::I32], shld);
        // L508: the double-width shifts (i64 at 32 bits)
        t.set_ops(&[ShlParts, SraParts, SrlParts], &[MVT::I32], Custom);
        // L241..L300: the x87 conversions
        t.set_ops(&[UIntToFp], &[MVT::I8, MVT::I16], Promote);
        t.set_ops(&[UIntToFp], &[MVT::I32, MVT::I64], Custom);
        t.set_ops(&[SIntToFp], &[MVT::I8], Promote);
        t.set_ops(&[SIntToFp], &[MVT::I16, MVT::I32, MVT::I64], Custom);
        t.set_ops(&[FpToSInt], &[MVT::I8], Promote);
        t.set_ops(&[FpToSInt], &[MVT::I16, MVT::I32, MVT::I64], Custom);
        t.set_ops(&[FpToUInt], &[MVT::I8, MVT::I16], Promote);
        t.set_ops(&[FpToUInt], &[MVT::I32, MVT::I64], Custom);
        // L316: no SSE2
        t.set_ops(&[BitCast], &[MVT::F32, MVT::I32], Expand);
        // L337..L370
        t.set_ops(&[MulHS, MulHU, SDiv, UDiv, SRem, URem], &ints64, Expand);
        // The pin divides an i8 or i16 by a constant with a widened multiply
        // (BuildSDIV finds neither MULHS nor SMUL_LOHI there); only i32
        // has the two-result multiply (`MUL32r`).
        t.set_ops(&[SMulLoHi, UMulLoHi], &[MVT::I8, MVT::I16], Expand);
        t.set_operation_action(BrJT, MVT::Other, Expand);
        t.set_operation_action(BrCond, MVT::Other, Custom);
        t.set_ops(&[BrCC, SelectCC], &[MVT::F32, MVT::F64, MVT::F80, MVT::I8, MVT::I16, MVT::I32, MVT::I64], Expand);
        t.set_ops(&[SignExtendInReg], &[MVT::I16, MVT::I8], Legal);
        t.set_operation_action(SignExtendInReg, MVT::I1, Expand);
        t.set_ops(&[FRem], &FLOATS, LibCall);
        // L372..L468: bit counts
        for op in [CtTz, CtTzZeroPoison] {
            t.set_operation_promoted_to_type(op, MVT::I8, MVT::I32);
            t.set_operation_promoted_to_type(op, MVT::I16, MVT::I32);
        }
        if !features.bmi {
            t.set_operation_action(CtTz, MVT::I32, Custom);
            t.set_operation_action(CtTzZeroPoison, MVT::I32, Legal);
        }
        if features.lzcnt {
            t.set_operation_promoted_to_type(CtLz, MVT::I8, MVT::I32);
            t.set_operation_promoted_to_type(CtLzZeroPoison, MVT::I8, MVT::I32);
        } else {
            t.set_ops(&[CtLz, CtLzZeroPoison], &ints, Custom);
        }
        t.set_ops(&[Parity], &ints, Custom);
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
        t.set_ops(&[Select, SetCC], &ints, Custom);
        t.set_ops(&[ConstantPool, GlobalAddress, ExternalSymbol, JumpTable], &[ptr], Custom);
        t.set_operation_action(Trap, MVT::Other, Legal);
        t.set_operation_action(DynamicStackAlloc, ptr, Custom);
        // L559..L566: 32-bit va_list is a pointer
        t.set_operation_action(VAStart, MVT::Other, Custom);
        t.set_ops(&[VAEnd, VACopy], &[MVT::Other], Expand);
        // L720..L760: x87 float classes (no SSE)
        for vt in [MVT::F32, MVT::F64] {
            t.set_ops(&[Undef, Poison], &[vt], Expand);
            t.set_operation_action(FCopySign, vt, Expand);
        }
        // L806..L870: f80
        t.set_ops(&[Undef, Poison], &[MVT::F80], Expand);
        t.set_operation_action(FCopySign, MVT::F80, Expand);
        // L764..L860: the x87 loads of 0.0 and 1.0 (`fldz`, `fld1`, and `fchs`)
        for (vt, one, sign) in [
            (MVT::F32, 0x3F80_0000u128, 1u128 << 31),
            (MVT::F64, 0x3FF0_0000_0000_0000, 1u128 << 63),
            (MVT::F80, (0x3FFFu128 << 64) | (1u128 << 63), 1u128 << 79),
        ] {
            for bits in [0, sign, one, one | sign] {
                t.add_legal_fp_immediate(vt, bits);
            }
        }
        // L2757..L2776
        t.set_ops(&[SAddO, UAddO, SSubO, USubO, SMulO, UMulO, UAddOCarry, USubOCarry], &ints, Custom);
        t
    }
}
