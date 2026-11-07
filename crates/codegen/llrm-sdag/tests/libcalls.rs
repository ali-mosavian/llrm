//! The soft-float table names what the `match` arms it replaced named: every
//! `f32` and `f64` routine, once, and none for a pair the pin has no row for.

use llrm_sdag::libcalls::{Cmp3, Libcall, name, names};
use llrm_sdag::mvt::MVT;

#[test]
fn the_table_names_each_f32_and_f64_routine_the_old_arms_named() {
    let (f, d) = (MVT::F32, MVT::F64);
    let want = [
        (Libcall::Add(f), "__addsf3"),
        (Libcall::Div(d), "__divdf3"),
        (Libcall::FpExt(f, d), "__extendsfdf2"),
        (Libcall::FpRound(d, f), "__truncdfsf2"),
        (Libcall::FpToSInt(f, MVT::I32), "__fixsfsi"),
        (Libcall::FpToUInt(d, MVT::I64), "__fixunsdfdi"),
        (Libcall::SIntToFp(MVT::I64, f), "__floatdisf"),
        (Libcall::UIntToFp(MVT::I32, d), "__floatunsidf"),
        (Libcall::Fcmp3(Cmp3::Uo, d), "__unorddf2"),
        (Libcall::Fcmp3(Cmp3::Oge, f), "__gesf2"),
        // binary128: compiler-rt's tf routines
        (Libcall::Add(MVT::F128), "__addtf3"),
        (Libcall::FpExt(d, MVT::F128), "__extenddftf2"),
        (Libcall::FpRound(MVT::F128, f), "__trunctfsf2"),
        (Libcall::FpToSInt(MVT::F128, MVT::I64), "__fixtfdi"),
        (Libcall::FpToUInt(MVT::F128, MVT::I64), "__fixunstfdi"),
        (Libcall::SIntToFp(MVT::I32, MVT::F128), "__floatsitf"),
        (Libcall::Fcmp3(Cmp3::Uo, MVT::F128), "__unordtf2"),
    ];
    for (call, expected) in want {
        assert_eq!(name(call), Some(expected), "{call:?}");
    }
    assert_eq!(names().count(), 67, "12 arithmetic, 6 ext/round, 14 to-int, 14 from-int, 21 compares (f32, f64, f128)");
}

#[test]
fn a_pair_the_pin_has_no_row_for_names_nothing() {
    assert_eq!(name(Libcall::FpExt(MVT::F64, MVT::F32)), None);
    assert_eq!(name(Libcall::FpRound(MVT::F80, MVT::F32)), None);
}
