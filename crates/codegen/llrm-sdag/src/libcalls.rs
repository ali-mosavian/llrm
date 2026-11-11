//! The soft-float routines' names: the pin's `RuntimeLibcalls.td` rows for the
//! `f32` and `f64` arithmetic, conversion and compare libcalls, keyed the way
//! `RTLIB::getFPEXT` and its siblings key them. One table, so a new float type
//! adds rows, not match arms.
//!
//! # Example
//! ```
//! use llrm_sdag::libcalls::{Libcall, name};
//! use llrm_sdag::mvt::MVT;
//! assert_eq!(name(Libcall::FpExt(MVT::F32, MVT::F64)), Some("__extendsfdf2"));
//! assert_eq!(name(Libcall::FpExt(MVT::F64, MVT::F32)), None);
//! ```
//!
//! # Runs
//! A lookup scans the table: constant time, the table has a fixed size.

use super::mvt::MVT;

/// The three-way compare routines: `FCMP3_PRED_*` and `UO_*`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmp3 {
    Oeq,
    Une,
    Oge,
    Olt,
    Ole,
    Ogt,
    Uo,
}

/// The `RTLIB::Libcall` entry a routine stands for. A conversion names its
/// float and its integer type in the order of the pin's `getFPTOSINT(OpVT,
/// RetVT)` and `getSINTTOFP(OpVT, RetVT)`: source first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Libcall {
    Add(MVT),
    Sub(MVT),
    Mul(MVT),
    Div(MVT),
    /// `FPEXT_<from>_<to>`
    FpExt(MVT, MVT),
    /// `FPROUND_<from>_<to>`
    FpRound(MVT, MVT),
    /// `FPTOSINT_<float>_<int>`
    FpToSInt(MVT, MVT),
    /// `FPTOUINT_<float>_<int>`
    FpToUInt(MVT, MVT),
    /// `SINTTOFP_<int>_<float>`
    SIntToFp(MVT, MVT),
    /// `UINTTOFP_<int>_<float>`
    UIntToFp(MVT, MVT),
    /// `FCMP3_PRED_<pred>_<float>`, `UO_<float>`
    Fcmp3(Cmp3, MVT),
}

use Cmp3::*;
use Libcall::*;

/// `RuntimeLibcalls.td`'s `def __name : RuntimeLibcallImpl<ENTRY>` rows, the
/// compiler-rt spelling.
/// [pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/IR/RuntimeLibcalls.td#L1179)
static TABLE: &[(Libcall, &str)] = &[
    (Add(MVT::F32), "__addsf3"),
    (Add(MVT::F64), "__adddf3"),
    (Sub(MVT::F32), "__subsf3"),
    (Sub(MVT::F64), "__subdf3"),
    (Mul(MVT::F32), "__mulsf3"),
    (Mul(MVT::F64), "__muldf3"),
    (Div(MVT::F32), "__divsf3"),
    (Div(MVT::F64), "__divdf3"),
    (Add(MVT::F128), "__addtf3"),
    (Sub(MVT::F128), "__subtf3"),
    (Mul(MVT::F128), "__multf3"),
    (Div(MVT::F128), "__divtf3"),
    (FpExt(MVT::F32, MVT::F64), "__extendsfdf2"),
    (FpRound(MVT::F64, MVT::F32), "__truncdfsf2"),
    (FpExt(MVT::F32, MVT::F128), "__extendsftf2"),
    (FpExt(MVT::F64, MVT::F128), "__extenddftf2"),
    (FpRound(MVT::F128, MVT::F32), "__trunctfsf2"),
    (FpRound(MVT::F128, MVT::F64), "__trunctfdf2"),
    (FpToSInt(MVT::F128, MVT::I32), "__fixtfsi"),
    (FpToSInt(MVT::F128, MVT::I64), "__fixtfdi"),
    (FpToSInt(MVT::F128, MVT::I128), "__fixtfti"),
    (FpToUInt(MVT::F128, MVT::I32), "__fixunstfsi"),
    (FpToUInt(MVT::F128, MVT::I64), "__fixunstfdi"),
    (FpToUInt(MVT::F128, MVT::I128), "__fixunstfti"),
    (SIntToFp(MVT::I32, MVT::F128), "__floatsitf"),
    (SIntToFp(MVT::I64, MVT::F128), "__floatditf"),
    (SIntToFp(MVT::I128, MVT::F128), "__floattitf"),
    (UIntToFp(MVT::I32, MVT::F128), "__floatunsitf"),
    (UIntToFp(MVT::I64, MVT::F128), "__floatunditf"),
    (UIntToFp(MVT::I128, MVT::F128), "__floatuntitf"),
    (Fcmp3(Oeq, MVT::F128), "__eqtf2"),
    (Fcmp3(Une, MVT::F128), "__netf2"),
    (Fcmp3(Oge, MVT::F128), "__getf2"),
    (Fcmp3(Olt, MVT::F128), "__lttf2"),
    (Fcmp3(Ole, MVT::F128), "__letf2"),
    (Fcmp3(Ogt, MVT::F128), "__gttf2"),
    (Fcmp3(Uo, MVT::F128), "__unordtf2"),
    (FpToSInt(MVT::F32, MVT::I32), "__fixsfsi"),
    (FpToSInt(MVT::F64, MVT::I32), "__fixdfsi"),
    (FpToSInt(MVT::F32, MVT::I64), "__fixsfdi"),
    (FpToSInt(MVT::F64, MVT::I64), "__fixdfdi"),
    (FpToUInt(MVT::F32, MVT::I32), "__fixunssfsi"),
    (FpToUInt(MVT::F64, MVT::I32), "__fixunsdfsi"),
    (FpToUInt(MVT::F32, MVT::I64), "__fixunssfdi"),
    (FpToUInt(MVT::F64, MVT::I64), "__fixunsdfdi"),
    (SIntToFp(MVT::I32, MVT::F32), "__floatsisf"),
    (SIntToFp(MVT::I32, MVT::F64), "__floatsidf"),
    (SIntToFp(MVT::I64, MVT::F32), "__floatdisf"),
    (SIntToFp(MVT::I64, MVT::F64), "__floatdidf"),
    (UIntToFp(MVT::I32, MVT::F32), "__floatunsisf"),
    (UIntToFp(MVT::I32, MVT::F64), "__floatunsidf"),
    (UIntToFp(MVT::I64, MVT::F32), "__floatundisf"),
    (UIntToFp(MVT::I64, MVT::F64), "__floatundidf"),
    (Fcmp3(Oeq, MVT::F32), "__eqsf2"),
    (Fcmp3(Oeq, MVT::F64), "__eqdf2"),
    (Fcmp3(Une, MVT::F32), "__nesf2"),
    (Fcmp3(Une, MVT::F64), "__nedf2"),
    (Fcmp3(Oge, MVT::F32), "__gesf2"),
    (Fcmp3(Oge, MVT::F64), "__gedf2"),
    (Fcmp3(Olt, MVT::F32), "__ltsf2"),
    (Fcmp3(Olt, MVT::F64), "__ltdf2"),
    (Fcmp3(Ole, MVT::F32), "__lesf2"),
    (Fcmp3(Ole, MVT::F64), "__ledf2"),
    (Fcmp3(Ogt, MVT::F32), "__gtsf2"),
    (Fcmp3(Ogt, MVT::F64), "__gtdf2"),
    (Fcmp3(Uo, MVT::F32), "__unordsf2"),
    (Fcmp3(Uo, MVT::F64), "__unorddf2"),
];

/// `getLibcallName` of `call`: the routine the pin's table names for it, if
/// any.
pub fn name(call: Libcall) -> Option<&'static str> {
    TABLE.iter().find(|(entry, _)| *entry == call).map(|(_, name)| *name)
}

/// Every routine the table names.
pub fn names() -> impl Iterator<Item = &'static str> {
    TABLE.iter().map(|(_, name)| *name)
}
