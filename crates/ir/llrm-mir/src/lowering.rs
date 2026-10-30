//! The target's lowering facts that passes translated from LLVM ask, as LLVM
//! splits them: `TargetLowering` answers these, `TargetTransformInfo`
//! (`tti`) prices instructions.
//!
//! # Example
//! ```
//! use llrm_mir::lowering::Lowering;
//! struct Plain;
//! impl Lowering for Plain {}
//! assert!(!Plain.is_mask_and_cmp0_folding_beneficial());
//! assert!(!Plain.is_mask_and_cmp0_folding_beneficial_for(Some(0x100)));
//! assert!(!Plain.is_truncate_free(32, 16));
//! assert!(!Plain.is_multi_stores_cheaper_than_bits_merge(llrm_mir::lowering::Domain::Float, llrm_mir::lowering::Domain::Integer));
//! assert!(!Plain.is_operation_legal_or_custom(llrm_mir::lowering::Isd::Add, 32));
//! assert!(Plain.is_load_ext_legal(llrm_mir::lowering::LoadExt::Zero, 32, 8));
//! assert!(!Plain.is_load_ext_legal(llrm_mir::lowering::LoadExt::Zero, 32, 24));
//! assert!(!Plain.is_ext_free(llrm_mir::lowering::Ext::ZExt, 32, 64));
//! ```
//!
//! # Runs
//! Unit tests of the targets that answer (`llrm-x86`'s `lowering`).

/// The extension an instruction makes, for `is_ext_free`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ext {
    ZExt,
    SExt,
    FpExt,
}

/// How an extension's user reads it, for `is_ext_free_impl`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtUser {
    /// A `shl` by a constant.
    ConstShl,
    /// An index of a `getelementptr` whose indexed type is `store_bits` wide.
    Gep { store_bits: u64 },
    /// A `trunc` back to the extended value's own type.
    SameTypeTrunc,
    /// Any other user.
    Other,
}

/// The operations of `ISD` that an IR instruction becomes
/// (`TargetLowering::InstructionOpcodeToISD`), for the legality hook.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Isd {
    Add,
    Sub,
    Mul,
    UDiv,
    SDiv,
    URem,
    SRem,
    Shl,
    Srl,
    Sra,
    And,
    Or,
    Xor,
    Select,
    SetCc,
    Truncate,
    ZeroExtend,
    SignExtend,
}

/// The kind of extending load (`ISD::LoadExtType`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadExt {
    Any,
    Zero,
    Sign,
}

/// What a value type is to `isMultiStoresCheaperThanBitsMerge`
/// (`EVT::isInteger`, `EVT::isFloatingPoint`): a vector or pointer is neither.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Domain {
    Integer,
    Float,
    Other,
}

/// Whether an integer of `bits` is a simple value type (`MVT`), the only ones
/// `getLoadExtAction` has an entry for; any other is `Expand`.
fn simple_int(bits: u32) -> bool {
    matches!(bits, 1 | 2 | 4 | 8 | 16 | 32 | 64 | 128)
}

/// `TargetLowering`'s hooks. Each default is the base class's.
pub trait Lowering {
    /// `isTypeLegal(EVT(iN))`: a register type of the target. The base class
    /// has none.
    fn is_type_legal(
        &self,
        bits: u32,
    ) -> bool {
        let _ = bits;
        false
    }

    /// `enableExtLdPromotion` (TargetLowering.h#L977): whether CodeGenPrepare
    /// may promote the operands of an extension towards the load that feeds
    /// them. The base class says no.
    fn enable_ext_ld_promotion(&self) -> bool {
        false
    }

    /// `isOperationLegalOrCustom(op, EVT(iN))`
    /// ([TargetLowering.h](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/TargetLowering.h)):
    /// whether the type is legal and the target does `op` on it itself or by a
    /// node of its own. The base class has no registers: nothing is.
    fn is_operation_legal_or_custom(
        &self,
        op: Isd,
        bits: u32,
    ) -> bool {
        let _ = (op, bits);
        false
    }

    /// `isLoadExtLegal` (`getLoadExtAction(..) == Legal`,
    /// [TargetLowering.h](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/TargetLowering.h)):
    /// whether a load of `memory_bits` extended to `value_bits` is one
    /// operation. The base class's table is all `Legal` for simple integers.
    fn is_load_ext_legal(
        &self,
        ext: LoadExt,
        value_bits: u32,
        memory_bits: u32,
    ) -> bool {
        let _ = ext;
        simple_int(value_bits) && simple_int(memory_bits)
    }

    /// `isTruncateFree(Type*, Type*)`
    /// ([TargetLowering.h#L3209](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/TargetLowering.h#L3209)):
    /// whether narrowing an integer of `from_bits` to `to_bits` costs nothing.
    fn is_truncate_free(
        &self,
        from_bits: u32,
        to_bits: u32,
    ) -> bool {
        let _ = (from_bits, to_bits);
        false
    }

    /// `isZExtFree(Type*, Type*)` (TargetLowering.h#L3304): whether widening an
    /// integer without a sign costs nothing, the value having been made by an
    /// instruction that clears the upper bits.
    fn is_zext_free(
        &self,
        from_bits: u32,
        to_bits: u32,
    ) -> bool {
        let _ = (from_bits, to_bits);
        false
    }

    /// `isFPExtFree` (TargetLowering.h#L3450): the base class says no.
    fn is_fp_ext_free(
        &self,
        dest_bits: u32,
        src_bits: u32,
    ) -> bool {
        let _ = (dest_bits, src_bits);
        false
    }

    /// `isExtFree(const Instruction*)` (TargetLowering.h#L3245): a floating
    /// extension asks `is_fp_ext_free`, a zero extension `is_zext_free`; a sign
    /// extension is free where `isExtFreeImpl` says, and the base class says
    /// no.
    fn is_ext_free(
        &self,
        kind: Ext,
        from_bits: u32,
        to_bits: u32,
    ) -> bool {
        match kind {
            Ext::FpExt => self.is_fp_ext_free(to_bits, from_bits),
            Ext::ZExt => self.is_zext_free(from_bits, to_bits),
            Ext::SExt => false,
        }
    }

    /// `isExtFreeImpl(const Instruction*)` (TargetLowering.h#L3245), asked
    /// after `is_ext_free` says no, with how each user reads the extension.
    /// The base class says no.
    fn is_ext_free_impl(
        &self,
        users: &[ExtUser],
    ) -> bool {
        let _ = users;
        false
    }

    /// `isMaskAndCmp0FoldingBeneficial` (TargetLowering.h): whether an `and`
    /// compared only with zero is better made again at each compare, for the
    /// one instruction the pair becomes.
    fn is_mask_and_cmp0_folding_beneficial(&self) -> bool {
        false
    }

    /// `preferZeroCompareBranch` (TargetLowering.h L770): whether a branch on a
    /// compare with a constant is better made a branch on a compare with zero
    /// of a value the block computes anyway. The base class says no; x86 says
    /// yes (X86ISelLowering.cpp L3805).
    fn prefer_zero_compare_branch(&self) -> bool {
        false
    }

    /// `getPreferredSwitchConditionType`, in bits: the width a switch on an
    /// integer of `bits` is widened to (`optimizeSwitchType`). The base class
    /// keeps the type.
    fn preferred_switch_condition_bits(
        &self,
        bits: u32,
    ) -> u32 {
        bits
    }

    /// `isMultiStoresCheaperThanBitsMerge(EVT, EVT)` (TargetLowering.h):
    /// whether a store of two values merged into one wider integer is
    /// better made two stores. The base class says no; x86 says yes for a
    /// float and an integer (X86ISelLowering.h#L243).
    fn is_multi_stores_cheaper_than_bits_merge(
        &self,
        low: Domain,
        high: Domain,
    ) -> bool {
        let _ = (low, high);
        false
    }

    /// `isEqualityCmpFoldedWithSignedCmp` (TargetLowering.h#L766): whether an
    /// `icmp eq` can be lowered as the signed compare beside it. The base class
    /// says yes; PowerPC says no. CodeGenPrepare's
    /// `foldICmpWithDominatingICmp` runs only where it is false.
    fn is_equality_cmp_folded_with_signed_cmp(&self) -> bool {
        true
    }

    /// `isCtpopFast(EVT)` (TargetLowering.h): whether `ctpop` of an integer of
    /// `bits` is one fast operation. The base class asks whether it is a legal
    /// operation, which none of the tables here lists; x86 asks for POPCNT
    /// (X86ISelLowering.cpp), and no CPU llrm builds for (i386, i486, pentium,
    /// the baseline of x86-64) has it.
    fn is_ctpop_fast(
        &self,
        bits: u32,
    ) -> bool {
        let _ = bits;
        false
    }

    /// The same question about the mask the `and` applies (`None`: unknown),
    /// which AArch64 answers by: a single-bit mask is a `tbz`, so the pair is
    /// folded; any other is better made again at each compare. By default the
    /// mask is not asked.
    fn is_mask_and_cmp0_folding_beneficial_for(
        &self,
        mask: Option<u128>,
    ) -> bool {
        let _ = mask;
        self.is_mask_and_cmp0_folding_beneficial()
    }
}
