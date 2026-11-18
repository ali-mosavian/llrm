use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::X86Lowering;
use super::*;

/// `extract_element x:i32, 0/1` of a register type (an `i32` at m16 is legal)
/// reached selection as `CannotSelect(ExtractElement)` (ia16_abi, wc16_mixed,
/// register_upper_half of the m16 run): the node lies about being legal, and
/// is a truncate, of a shift for the high half.
#[test]
fn extracting_a_half_of_a_legal_wide_value_is_a_truncate_of_a_shift() {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    for (which, shifted) in [(0u128, false), (1, true)] {
        let mut dag = dag();
        let x = dag.get_register(EVT::I32, 5);
        let index = dag.get_constant(which, EVT::I16);
        let half = dag.get_node(ISD::ExtractElement, EVT::I16, &[x, index]);
        dag.set_root(half);
        DagLegalizer::new(
            &mut dag,
            &tli,
            &mut X86Lowering {
                multiply_early_out: None,
                imul_if_smaller: false, hot_chains: false,
                share_products: false,
                cmov: false,
                sse2: false,
                min_size: false,
                three_address_adds: false,
                i16_mul_chains: false,
                sext_load: false,
                zext_free: false,
                branch_constant_selects: false,
                status_word_tests: false,
                profile: None,
            },
        )
        .run()
        .unwrap();
        let root = dag.get_root();
        assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Truncate, EVT::I16), "half {which}");
        let inner = dag.operand(root, 0);
        if shifted {
            assert_eq!((dag.opcode(inner), dag.operand(inner, 0)), (ISD::Srl, x));
            assert_eq!(dag.constant(dag.operand(inner, 1)), Some(16));
        } else {
            assert_eq!(inner, x);
        }
    }
}

/// `extract_element v4f32 %x, i` from a vector in a register reached selection
/// as `CannotSelect(ExtractElement)` (fpbench and nbody_single at -O3 once the
/// vectorizer emitted a `<4 x float>` phi): the float vectors' extract is
/// `Expand` but no expansion existed. It is a store of the vector and a load of
/// the lane (`ExpandExtractFromVectorThroughStack`).
#[test]
fn extracting_a_lane_of_a_float_vector_in_a_register_is_a_load_from_the_stack() {
    let mut tli =
        TargetLowering::x86_64_vectors(Features { sse2: true, vector128: true, ..Features::default() }, MVT::I64);
    tli.set_operation_action(ISD::ExtractElement, MVT::V4F32, Action::Expand);
    for index in [Some(1u128), None] {
        let mut dag = dag();
        let x = dag.get_register(EVT::Simple(MVT::V4F32), 5);
        let at = match index {
            Some(lane) => dag.get_constant(lane, EVT::I64),
            None => dag.get_register(EVT::I64, 6),
        };
        let lane = dag.get_node(ISD::ExtractElement, EVT::Simple(MVT::F32), &[x, at]);
        dag.set_root(lane);
        DagLegalizer::new(
            &mut dag,
            &tli,
            &mut X86Lowering {
                multiply_early_out: None,
                imul_if_smaller: false, hot_chains: false,
                share_products: false,
                cmov: false,
                sse2: true,
                min_size: false,
                three_address_adds: false,
                i16_mul_chains: false,
                sext_load: false,
                zext_free: false,
                branch_constant_selects: false,
                status_word_tests: false,
                profile: None,
            },
        )
        .run()
        .unwrap();
        let root = dag.get_root();
        assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Load, EVT::Simple(MVT::F32)), "index {index:?}");
        let spill = dag.operand(root, 0);
        assert_eq!(dag.opcode(spill), ISD::Store, "index {index:?}: the vector is stored first");
    }
}
