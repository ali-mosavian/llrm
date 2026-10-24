use super::*;

fn p5() -> TargetLowering {
    TargetLowering::x86_32(Features::default(), MVT::I32)
}

fn act(
    t: &TargetLowering,
    op: ISD,
    vt: MVT,
) -> Action {
    t.operation_action(op, vt.into())
}

/// The pin's X86 makes `select` and `setcc` Custom at every integer width
/// on a CPU with no CMOV, and `LowerSELECT` branches; with the table empty
/// they are Legal and a select reaches the matcher with no instruction.
#[test]
fn select_is_custom_without_cmov() {
    let t = p5();
    for vt in [MVT::I8, MVT::I16, MVT::I32, MVT::F32, MVT::F64, MVT::F80] {
        assert_eq!(act(&t, ISD::Select, vt), Action::Custom, "{vt:?}");
        assert_eq!(act(&t, ISD::SetCC, vt), Action::Custom, "{vt:?}");
    }
    let empty = TargetLowering::empty(TargetLoweringInfo::x86_32());
    assert_eq!(act(&empty, ISD::Select, MVT::I32), Action::Legal);
}

/// Wide integers: the type is expanded (B0a) and the division family is
/// `Expand` for every width, so `SDIVREM` is the two-result form.
#[test]
fn i64_expands_and_division_is_expanded() {
    let t = p5();
    assert_eq!(t.types.type_action(EVT::I64), TypeAction::ExpandInteger);
    for vt in [MVT::I8, MVT::I16, MVT::I32, MVT::I64] {
        for op in [ISD::SDiv, ISD::UDiv, ISD::SRem, ISD::URem, ISD::MulHS, ISD::MulHU] {
            assert_eq!(act(&t, op, vt), Action::Expand, "{op:?} {vt:?}");
        }
    }
    assert_eq!(act(&t, ISD::Add, MVT::I32), Action::Legal);
}

/// An `i8` conversion to float promotes; a table without the x87 arm left
/// `i8 sitofp` Legal, which no instruction takes.
#[test]
fn byte_conversions_promote() {
    let t = p5();
    assert_eq!(act(&t, ISD::SIntToFp, MVT::I8), Action::Promote);
    assert_eq!(act(&t, ISD::UIntToFp, MVT::I16), Action::Promote);
    assert_eq!(act(&t, ISD::FpToSInt, MVT::I8), Action::Promote);
    assert_eq!(act(&t, ISD::SIntToFp, MVT::I32), Action::Custom);
    assert_eq!(act(&t, ISD::FpToUInt, MVT::I32), Action::Custom);
}

/// The bit counts of a CPU with no BMI, LZCNT or POPCNT: byte and word
/// `cttz` promote to `i32`, `ctlz` and `ctpop` are Custom (the expansion
/// `bitcounts.rs` makes today); with POPCNT the small `ctpop`s promote.
#[test]
fn bit_counts_follow_the_cpu() {
    let t = p5();
    assert_eq!(act(&t, ISD::CtTz, MVT::I8), Action::Promote);
    assert_eq!(t.type_to_promote_to(ISD::CtTz, MVT::I16), MVT::I32);
    assert_eq!(act(&t, ISD::CtTz, MVT::I32), Action::Custom);
    assert_eq!(act(&t, ISD::CtLz, MVT::I32), Action::Custom);
    assert_eq!(act(&t, ISD::CtPop, MVT::I32), Action::Custom);
    let pop = TargetLowering::x86_32(Features { popcnt: true, ..Features::default() }, MVT::I32);
    assert_eq!(act(&pop, ISD::CtPop, MVT::I8), Action::Promote);
    assert_eq!(pop.type_to_promote_to(ISD::CtPop, MVT::I8), MVT::I32);
}

/// No `br_cc` or `select_cc` node survives; `brcond` is lowered by hand;
/// `frem` is a library call and `bswap` of a word is expanded.
#[test]
fn branches_and_odd_operations() {
    let t = p5();
    for vt in [MVT::F32, MVT::F64, MVT::F80, MVT::I8, MVT::I16, MVT::I32, MVT::I64] {
        assert_eq!(act(&t, ISD::BrCC, vt), Action::Expand);
        assert_eq!(act(&t, ISD::SelectCC, vt), Action::Expand);
    }
    assert_eq!(act(&t, ISD::BrCond, MVT::Other), Action::Custom);
    assert_eq!(act(&t, ISD::BrJT, MVT::Other), Action::Expand);
    assert_eq!(act(&t, ISD::FRem, MVT::F64), Action::LibCall);
    assert_eq!(act(&t, ISD::BSwap, MVT::I16), Action::Expand);
    assert_eq!(act(&t, ISD::BSwap, MVT::I32), Action::Legal);
}

/// Loads, truncating stores and condition codes.
#[test]
fn memory_and_condition_code_actions() {
    let t = p5();
    for vt in [MVT::I8, MVT::I16, MVT::I32, MVT::I64] {
        assert_eq!(t.load_ext_action(LoadExt::SExtLoad, vt, MVT::I1), Action::Promote);
    }
    assert_eq!(t.trunc_store_action(MVT::I32, MVT::I16), Action::Expand);
    assert_eq!(t.trunc_store_action(MVT::F64, MVT::F32), Action::Expand);
    assert_eq!(t.cond_code_action(CondCode::SetOEq, MVT::F64), Action::Expand);
    assert_eq!(t.cond_code_action(CondCode::SetOLt, MVT::F64), Action::Legal);
}

/// The initial defaults: an integer type with no simple form is Expand,
/// the overflow and carry family is Expand until the constructor makes
/// some of it Custom, and a later call overwrites an earlier one.
#[test]
fn defaults_and_overwriting() {
    let t = p5();
    assert_eq!(t.operation_action(ISD::Add, EVT::Int(12)), Action::Expand);
    assert_eq!(act(&t, ISD::AddC, MVT::I32), Action::Expand);
    assert_eq!(act(&t, ISD::UAddO, MVT::I32), Action::Custom);
    assert_eq!(act(&t, ISD::UAddO, MVT::I64), Action::Expand, "i64 stays expanded");
    let mut t = TargetLowering::empty(TargetLoweringInfo::x86_32());
    t.set_operation_action(ISD::Select, MVT::I32, Action::Expand);
    t.set_operation_action(ISD::Select, MVT::I32, Action::Custom);
    assert_eq!(act(&t, ISD::Select, MVT::I32), Action::Custom);
}

/// m16 had no action table (`x86_32` was the only constructor), so no 16-bit
/// function could be legalized: the same operations act as at 32 bits, and the
/// far pointer expands.
#[test]
fn m16_has_the_32_bit_actions_and_a_far_pointer_type() {
    let features = Features::default();
    let (m32, m16) = (TargetLowering::x86_32(features, MVT::I32), TargetLowering::x86_16(features, MVT::I16));
    for op in [ISD::SDiv, ISD::MulHU, ISD::CtPop, ISD::Parity, ISD::BrCond] {
        for vt in [MVT::I16, MVT::I32] {
            assert_eq!(m16.operation_action(op, vt.into()), m32.operation_action(op, vt.into()), "{op:?} {vt:?}");
        }
    }
    assert_eq!(m16.types.type_action(MVT::Far.into()), TypeAction::ExpandInteger);
}

/// The address operations were Custom at `i32` whatever the pointer type, so an
/// m16 global address (an `i16`) reached selection as a bare `GlobalAddress`.
#[test]
fn address_operations_are_custom_at_the_pointer_type() {
    let features = Features::default();
    for (tli, ptr) in
        [(TargetLowering::x86_32(features, MVT::I32), MVT::I32), (TargetLowering::x86_16(features, MVT::I16), MVT::I16)]
    {
        for op in [ISD::GlobalAddress, ISD::ConstantPool, ISD::ExternalSymbol] {
            assert_eq!(act(&tli, op, ptr), Action::Custom, "{op:?} {ptr:?}");
        }
    }
}

/// Only `i32` has the two-result multiply: an `i16` divide by a constant used
/// `SMUL_LOHI` (legal by default), which the matcher cannot take, and 12
/// execution programs stopped at "unselected SMulLoHi". The pin's `BuildSDIV`
/// finds neither `MULHS` nor `SMUL_LOHI` at `i16` and widens.
#[test]
fn the_two_result_multiply_is_legal_at_i32_only() {
    let t = p5();
    for op in [ISD::SMulLoHi, ISD::UMulLoHi] {
        assert_eq!(act(&t, op, MVT::I32), Action::Legal, "{op:?} i32");
        assert_eq!(act(&t, op, MVT::I16), Action::Expand, "{op:?} i16");
        assert_eq!(act(&t, op, MVT::I8), Action::Expand, "{op:?} i8");
    }
}
