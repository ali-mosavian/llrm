//! The 6502 states its `TargetLowering` to the shared mid-end.

use llrm_6502::driver::machine;
use llrm_6502::ir_lowering::STEPS;
use llrm_mir::lowering::{CgpStep, Isd, LoadExt};

const ALL: [CgpStep; 8] = [
    CgpStep::Branches,
    CgpStep::SwitchType,
    CgpStep::OverflowMath,
    CgpStep::Compares,
    CgpStep::MaskSinking,
    CgpStep::CastSinking,
    CgpStep::ExtPromotion,
    CgpStep::SplitStores,
];

/// `Mos6502Machine::lowering()` was `None`, so every CodeGenPrepare step that
/// asks `TargetLowering` was dead on the 6502 (the same gap wasm had). Now it
/// answers, and runs exactly the steps `STEPS` lists, each added with its own
/// A/B.
#[test]
fn the_machine_states_its_lowering_and_runs_the_listed_steps() {
    let lowering = machine().lowering().expect("a lowering");
    for step in ALL {
        assert_eq!(lowering.runs_cgp_step(step), STEPS.contains(&step), "{step:?}");
    }
}

/// i8 and i16 are the register types, i32 is not; the answers come from the
/// DAG's action table.
#[test]
fn its_answers_are_the_dag_tables() {
    let lowering = machine().lowering().expect("a lowering");
    assert!(lowering.is_type_legal(8) && lowering.is_type_legal(16) && !lowering.is_type_legal(32));
    assert!(lowering.is_operation_legal_or_custom(Isd::Add, 8) && !lowering.is_operation_legal_or_custom(Isd::Add, 32));
    // a sign-extending load is a load and an extension; no instruction reads
    // one
    assert!(!lowering.is_load_ext_legal(LoadExt::Sign, 16, 8));
    assert!(!lowering.is_load_ext_legal(LoadExt::Zero, 16, 8));
}
