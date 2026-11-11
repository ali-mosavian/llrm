//! The generated scheduling models against the pin's own `llc`.

use crate::generated::aarch64::ids::opcode;
use crate::generated::aarch64::{instrinfo, sched};
use crate::sched::{Modeled, SchedModel, SchedPred};

fn latency(
    model: &SchedModel,
    opcode: u16,
    shifted: bool,
) -> i32 {
    match model.resolve(opcode, &mut |p| p == SchedPred::HasShiftedReg && shifted) {
        Some(Modeled::Class(class)) => class.write_latency[0].cycles,
        other => panic!("opcode {opcode}: {other:?}"),
    }
}

#[test]
fn the_cpus_of_the_target_have_the_pins_models() {
    let generic = sched::model("generic").unwrap();
    let apple = sched::model("apple-m1").unwrap();
    assert_eq!(
        (
            generic.name,
            generic.issue_width,
            generic.micro_op_buffer_size,
            generic.load_latency,
            generic.post_ra_scheduler
        ),
        ("CortexA510Model", 3, 0, 3, true)
    );
    assert_eq!(
        (apple.name, apple.issue_width, apple.micro_op_buffer_size, apple.load_latency, apple.post_ra_scheduler),
        ("CycloneModel", 6, 192, 4, false)
    );
    assert!(sched::model("cortex-a53").is_none(), "a CPU the target does not name has no model");
}

/// The numbers `llc -O2 -mcpu=<cpu> -debug-only=machine-scheduler` prints as
/// `Latency` for `load; sdiv; madd; add` at the pin: the table reads the same
/// classes the pin's scheduler does.
#[test]
fn latencies_are_the_ones_the_pins_scheduler_prints() {
    let generic = sched::model("generic").unwrap();
    let apple = sched::model("apple-m1").unwrap();
    for (op, a510, cyclone) in
        [(opcode::LDRWui, 2, 4), (opcode::SDIVWr, 8, 10), (opcode::MADDWrrr, 3, 4), (opcode::ADDWri, 1, 1)]
    {
        assert_eq!((latency(generic, op, false), latency(apple, op, false)), (a510, cyclone), "opcode {op}");
    }
}

/// `ADDWrs` is a variant class on Cortex-A510: a shifted operand is
/// `WriteISReg`, else `WriteI`. Resolving without the predicate would price
/// every shifted add as a plain one.
#[test]
fn a_variant_class_follows_its_predicate() {
    let generic = sched::model("generic").unwrap();
    assert_eq!(latency(generic, opcode::ADDWrs, true), 2);
    assert_eq!(latency(generic, opcode::ADDWrs, false), 1);
}

#[test]
fn an_atomic_is_unsupported_and_an_opcode_off_the_table_is_none() {
    for cpu in ["generic", "apple-m1"] {
        let model = sched::model(cpu).unwrap();
        assert!(matches!(model.resolve(opcode::CASAB, &mut |_| false), Some(Modeled::Unsupported)), "{cpu}");
        assert!(model.resolve(u16::MAX, &mut |_| false).is_none(), "{cpu}");
    }
}

/// Every opcode the DAG tables emit has a class in each model: a gap would be
/// priced as a default.
#[test]
fn every_emitted_opcode_has_a_class() {
    for cpu in ["generic", "apple-m1"] {
        let model = sched::model(cpu).unwrap();
        for &(op, _) in instrinfo::INSTRS.iter() {
            assert!(model.resolve(op, &mut |_| false).is_some(), "{cpu}: opcode {op} has no class");
        }
    }
}

/// With the vector types live, the table gained 638 NEON rows and lost the
/// scalar `FMOVWSr`/`FMOVXDr` (patterns of types it dropped): a scalar float
/// bitcast and the zero-fill `movi` the hand code emits were scheduled
/// differently with the vectors off (nbody, lru, sfm16_float: 12 objects of
/// 188 differed). Vector-only opcodes are marked; scalar ones keep a class.
#[test]
fn vector_only_opcodes_are_marked_and_scalar_ones_keep_their_class() {
    let apple = sched::model("apple-m1").unwrap();
    for op in [opcode::MOVIv2d_ns, opcode::MOVIv16b_ns, opcode::ADDv4i32] {
        assert!(sched::dynamic_only(op), "opcode {op}");
        assert!(apple.class_of(op).is_some(), "opcode {op} has a class when the vectors are on");
    }
    for op in [opcode::FMOVWSr, opcode::FMOVXDr, opcode::FMOVDr, opcode::ADDWri, opcode::LDRQui] {
        assert!(!sched::dynamic_only(op), "opcode {op}");
        assert!(apple.class_of(op).is_some(), "opcode {op} has a class");
    }
}
