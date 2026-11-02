//! The wasm lowering on hand-built DAGs, against the pin's
//! `WebAssemblyISelLowering.cpp` (constructor, `LowerGlobalAddress`,
//! `LowerFrameIndex`).

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::legalize_dag::DagLegalizer;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{Action, DagConfig, NodeData, SDValue, SelectionDAG, TypeAction};
use llrm_wasm::dagisel::hooks::Cpu;
use llrm_wasm::dagisel::lowering::{WasmLowering, target_lowering, wasmisd};

fn legalize(
    dag: &mut SelectionDAG,
    cpu: Cpu,
) -> SDValue {
    let tli = target_lowering(cpu);
    DagLegalizer::new(dag, &tli, &mut WasmLowering { tables: &[] }).run().expect("legalizes");
    dag.get_root()
}

/// The pin wraps a global as `Wrapper (TargetGlobalAddress sym + offset)`, the
/// shape `CONST_I32` and the address patterns match. With no wasm action table
/// the address stayed a generic `GlobalAddress` that no pattern selects.
#[test]
fn a_global_address_is_a_wrapper_of_its_target_form() {
    for (cpu, vt) in [(Cpu::Wasm32, EVT::I32), (Cpu::Wasm64, EVT::I64)] {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let global = dag.get_global_address(vt, 7, 12);
        dag.set_root(global);

        let root = legalize(&mut dag, cpu);

        assert_eq!(dag.opcode(root), wasmisd::WRAPPER);
        let target = dag.operand(root, 0);
        assert_eq!(dag.opcode(target), ISD::TargetGlobalAddress);
        assert_eq!(dag.node(target).data, NodeData::Global { symbol: 7, offset: 12 });
    }
}

#[test]
fn a_frame_index_is_its_target_form() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let slot = dag.get_frame_index(EVT::I32, 3);
    dag.set_root(slot);

    let root = legalize(&mut dag, Cpu::Wasm32);

    assert_eq!(dag.opcode(root), ISD::TargetFrameIndex);
    assert_eq!(dag.node(root).data, NodeData::FrameIndex(3));
}

/// `i32` and `i64` are the register classes: `i8` and `i16` promote to `i32`,
/// and the wide-multiply family expands for both.
#[test]
fn only_i32_and_i64_are_legal_integers() {
    let tl = target_lowering(Cpu::Wasm32);
    assert_eq!(tl.types.type_conversion(EVT::I8), (TypeAction::PromoteInteger, EVT::I32));
    assert_eq!(tl.types.type_conversion(EVT::I64).0, TypeAction::Legal);
    for vt in [EVT::I32, EVT::I64] {
        assert_eq!(tl.operation_action(ISD::MulHU, vt), Action::Expand);
        assert_eq!(tl.operation_action(ISD::Add, vt), Action::Legal);
    }
}
