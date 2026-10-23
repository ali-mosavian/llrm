use super::*;

fn conv(
    info: &TargetLoweringInfo,
    vt: EVT,
) -> (TypeAction, EVT) {
    info.type_conversion(vt)
}

/// An empty action table would call every type legal; `i1` and `i12` then
/// reach the matcher as registers no instruction names.
#[test]
fn small_and_odd_integers_promote_and_wide_ones_expand() {
    for info in [TargetLoweringInfo::x86_32(), TargetLoweringInfo::x86_16()] {
        for vt in [EVT::I8, EVT::I16] {
            assert_eq!(conv(&info, vt), (TypeAction::Legal, vt));
        }
        assert_eq!(conv(&info, EVT::I1), (TypeAction::PromoteInteger, EVT::I8));
        assert_eq!(conv(&info, EVT::Int(12)), (TypeAction::PromoteInteger, EVT::I16));
        assert_eq!(conv(&info, EVT::Int(24)), (TypeAction::PromoteInteger, EVT::I32));
        assert_eq!(conv(&info, EVT::I64), (TypeAction::ExpandInteger, EVT::I32));
        assert_eq!(conv(&info, EVT::integer(128)), (TypeAction::ExpandInteger, EVT::I64));
    }
}

/// m32 has `i32` registers.
#[test]
fn m32_has_32_bit_registers() {
    assert_eq!(conv(&TargetLoweringInfo::x86_32(), EVT::I32), (TypeAction::Legal, EVT::I32));
}

/// An integer wider than the widest register promoted to the next power
/// of two would loop; `i40` rounds to `i64`, which then expands.
#[test]
fn an_odd_wide_integer_rounds_up_then_expands() {
    let info = TargetLoweringInfo::x86_32();
    assert_eq!(conv(&info, EVT::Int(40)), (TypeAction::PromoteInteger, EVT::I64));
    assert_eq!(conv(&info, EVT::Int(256)), (TypeAction::ExpandInteger, EVT::integer(128)));
}

/// Floats are never softened (SD14).
#[test]
fn floats_are_legal() {
    let info = TargetLoweringInfo::x86_32();
    for vt in [MVT::F32, MVT::F64, MVT::F80] {
        assert!(info.is_type_legal(vt.into()));
    }
}

/// m16 expands the far pointer into two words; without the rule it was
/// an unknown type.
#[test]
fn far_expands_to_two_words_at_m16_only() {
    assert_eq!(conv(&TargetLoweringInfo::x86_16(), MVT::Far.into()), (TypeAction::ExpandInteger, EVT::I16));
    let m32 = std::panic::catch_unwind(|| TargetLoweringInfo::x86_32().type_action(MVT::Far.into()));
    assert!(m32.is_err());
}

/// m16 said `i32` was a legal register type while its builder splits every
/// `i32` into two `i16`s, so 94 pair nodes (`build_pair`, `extract_element`)
/// reached selection unlegalized.
#[test]
fn m16_expands_an_i32_into_i16_halves_and_m32_keeps_it() {
    use TypeAction::*;
    let (m16, m32) = (TargetLoweringInfo::x86_16(), TargetLoweringInfo::x86_32());
    assert_eq!(m16.type_conversion(EVT::I32), (ExpandInteger, EVT::I16));
    assert_eq!(m32.type_conversion(EVT::I32), (Legal, EVT::I32));
}
