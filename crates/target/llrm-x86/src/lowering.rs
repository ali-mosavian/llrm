//! X86's `TargetLowering` facts, the same for every x86 target whatever its
//! word size.
//!
//! # Example
//! ```
//! use llrm_mir::lowering::Lowering;
//! use llrm_x86::lowering::X86;
//! assert!(X86 { is_64bit: false }.is_mask_and_cmp0_folding_beneficial());
//! assert!(X86 { is_64bit: false }.is_truncate_free(32, 16));
//! assert!(!X86 { is_64bit: false }.is_zext_free(32, 64));
//! assert!(X86 { is_64bit: true }.is_zext_free(32, 64));
//! ```
//!
//! # Runs
//! `cargo test -p llrm-x86 lowering`.

use std::sync::OnceLock;

use llrm_mir::lowering::{Isd, LoadExt, Lowering};
use llrm_sdag::actions::{Features, TargetLowering};
use llrm_sdag::isd::ISD;
use llrm_sdag::mvt::MVT;

/// The DAG's tables for i486, built once.
fn table() -> &'static TargetLowering {
    static TABLE: OnceLock<TargetLowering> = OnceLock::new();
    TABLE.get_or_init(|| TargetLowering::x86_32(Features::default(), MVT::I32))
}

/// The x86 family's answers. `is_64bit` is `Subtarget.is64Bit()`.
pub struct X86 {
    pub is_64bit: bool,
}

impl Lowering for X86 {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86ISelLowering.cpp#L3807-L3810
    fn is_mask_and_cmp0_folding_beneficial(&self) -> bool {
        true
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86ISelLowering.h#L243
    // A float and an integer: two stores save two bitwise instructions and the
    // float-to-int domain switch. Two integers are left out, as the pin does.
    fn is_multi_stores_cheaper_than_bits_merge(
        &self,
        low: llrm_mir::lowering::Domain,
        high: llrm_mir::lowering::Domain,
    ) -> bool {
        use llrm_mir::lowering::Domain::{Float, Integer};
        matches!((low, high), (Float, Integer) | (Integer, Float))
    }

    // A switch on an i8 or i16 is widened to i32 (checked on the pin's llc at
    // m16, m32 and m64: `switch i32` after CodeGenPrepare).
    fn preferred_switch_condition_bits(
        &self,
        bits: u32,
    ) -> u32 {
        bits.max(32)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86ISelLowering.cpp#L3805
    fn prefer_zero_compare_branch(&self) -> bool {
        true
    }

    // The register classes (X86ISelLowering L179-183), from the DAG's table; a
    // 64-bit target has i64 as well (DEVIATE TL2, as below).
    fn is_type_legal(
        &self,
        bits: u32,
    ) -> bool {
        if self.is_64bit {
            return matches!(bits, 8 | 16 | 32 | 64);
        }
        MVT::integer(bits).is_some_and(|vt| table().types.is_type_legal(vt.into()))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86ISelLowering.cpp#L2955
    fn enable_ext_ld_promotion(&self) -> bool {
        true
    }

    // `isOperationLegalOrCustom` from the DAG's action table (`dag::actions`):
    // one copy of what the legalizer does with each operation on each type.
    // DEVIATE TL2: the table translates the 32-bit arms only, so a 64-bit
    // target answers as before the table: the divides `Expand` (L327), the
    // rest legal on the register types. Owner sdag-b, 2027-02-04.
    fn is_operation_legal_or_custom(
        &self,
        op: Isd,
        bits: u32,
    ) -> bool {
        if self.is_64bit {
            let legal_type = matches!(bits, 8 | 16 | 32 | 64);
            return legal_type && !matches!(op, Isd::UDiv | Isd::SDiv | Isd::URem | Isd::SRem);
        }
        let Some(vt) = MVT::integer(bits) else { return false };
        let node = match op {
            Isd::Add => ISD::Add,
            Isd::Sub => ISD::Sub,
            Isd::Mul => ISD::Mul,
            Isd::UDiv => ISD::UDiv,
            Isd::SDiv => ISD::SDiv,
            Isd::URem => ISD::URem,
            Isd::SRem => ISD::SRem,
            Isd::Shl => ISD::Shl,
            Isd::Srl => ISD::Srl,
            Isd::Sra => ISD::Sra,
            Isd::And => ISD::And,
            Isd::Or => ISD::Or,
            Isd::Xor => ISD::Xor,
            Isd::Select => ISD::Select,
            Isd::SetCc => ISD::SetCC,
            Isd::Truncate => ISD::Truncate,
            Isd::ZeroExtend => ISD::ZeroExtend,
            Isd::SignExtend => ISD::SignExtend,
        };
        table().is_operation_legal_or_custom(node, vt.into())
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86ISelLowering.cpp#L186-L187
    // `setLoadExtAction(SEXTLOAD, VT, i1, Promote)` for every integer; the
    // rest is the base class's `Legal`.
    fn is_load_ext_legal(
        &self,
        ext: LoadExt,
        value_bits: u32,
        memory_bits: u32,
    ) -> bool {
        !(ext == LoadExt::Sign && memory_bits == 1)
            && matches!(value_bits, 1 | 2 | 4 | 8 | 16 | 32 | 64 | 128)
            && matches!(memory_bits, 1 | 2 | 4 | 8 | 16 | 32 | 64 | 128)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86ISelLowering.cpp#L36708-L36714
    // Any narrowing of one integer to another is a use of the low part.
    fn is_truncate_free(
        &self,
        from_bits: u32,
        to_bits: u32,
    ) -> bool {
        from_bits > to_bits
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86ISelLowering.cpp#L36751-L36754
    // x86-64 implicitly zero-extends 32-bit results in 64-bit registers.
    fn is_zext_free(
        &self,
        from_bits: u32,
        to_bits: u32,
    ) -> bool {
        from_bits == 32 && to_bits == 64 && self.is_64bit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `test` makes the pair one instruction on every x86 target; the base
    /// class says no.
    #[test]
    fn test_x86_folds_a_mask_and_compare_with_zero() {
        assert!(X86 { is_64bit: false }.is_mask_and_cmp0_folding_beneficial());
    }

    /// A promoted add is legal at 8, 16 and 32 bits, at 64 on a 64-bit target
    /// only, a divide at none, and the base class has nothing legal.
    #[test]
    fn test_x86_operations_are_legal_on_its_register_types() {
        let (narrow, wide) = (X86 { is_64bit: false }, X86 { is_64bit: true });
        assert!(
            narrow.is_operation_legal_or_custom(Isd::Add, 16) && narrow.is_operation_legal_or_custom(Isd::SetCc, 8)
        );
        assert!(!narrow.is_operation_legal_or_custom(Isd::Add, 64) && wide.is_operation_legal_or_custom(Isd::Add, 64));
        assert!(
            !wide.is_operation_legal_or_custom(Isd::UDiv, 32) && !narrow.is_operation_legal_or_custom(Isd::Add, 24)
        );
    }

    /// An extending load of 8, 16 or 32 bits is one operation, a sign
    /// extension of a bit is not (`Promote`), and a width that is no machine
    /// type is not.
    #[test]
    fn test_x86_extending_loads_are_legal_but_a_sign_extended_bit() {
        let x86 = X86 { is_64bit: false };
        assert!(x86.is_load_ext_legal(LoadExt::Zero, 32, 8) && x86.is_load_ext_legal(LoadExt::Sign, 32, 16));
        assert!(x86.is_load_ext_legal(LoadExt::Zero, 32, 1) && !x86.is_load_ext_legal(LoadExt::Sign, 32, 1));
        assert!(!x86.is_load_ext_legal(LoadExt::Zero, 32, 24) && !x86.is_load_ext_legal(LoadExt::Any, 40, 8));
    }

    /// A truncation is free on every x86 target; a zero extension from 32 to
    /// 64 bits only where the word is 64 bits (a 32-bit write clears the upper
    /// half), and a sign extension nowhere. The base class says no to all.
    #[test]
    fn test_x86_extensions_are_free_where_the_pin_says() {
        use llrm_mir::lowering::Ext;
        let (narrow, wide) = (X86 { is_64bit: false }, X86 { is_64bit: true });
        assert!(narrow.is_truncate_free(32, 16) && wide.is_truncate_free(64, 32));
        assert!(!narrow.is_truncate_free(16, 32) && !narrow.is_truncate_free(16, 16));
        assert!(!narrow.is_ext_free(Ext::ZExt, 32, 64) && wide.is_ext_free(Ext::ZExt, 32, 64));
        assert!(!wide.is_ext_free(Ext::ZExt, 16, 32) && !wide.is_ext_free(Ext::SExt, 32, 64));
        assert!(!wide.is_ext_free(Ext::FpExt, 32, 64));
    }

    /// Two stores beat the merge for a float and an integer, in either order;
    /// two integers (or a vector) are left merged, as the pin does.
    #[test]
    fn test_x86_splits_a_merged_store_of_a_float_and_an_integer_only() {
        use llrm_mir::lowering::Domain::{Float, Integer, Other};
        let x86 = X86 { is_64bit: false };
        assert!(x86.is_multi_stores_cheaper_than_bits_merge(Float, Integer));
        assert!(x86.is_multi_stores_cheaper_than_bits_merge(Integer, Float));
        assert!(!x86.is_multi_stores_cheaper_than_bits_merge(Integer, Integer));
        assert!(!x86.is_multi_stores_cheaper_than_bits_merge(Float, Float));
        assert!(!x86.is_multi_stores_cheaper_than_bits_merge(Float, Other));
    }
}
