//! The tables instruction selection reads are built once, not per call.

use llrm_arm64::dag::pipeline::Aarch64Sched;
use llrm_arm64::reg;
use llrm_codegen::backend::selectiondag::schedule::SchedInfo;
use llrm_support::alloc_count::{Counting, counted};

#[global_allocator]
static COUNTING: Counting = Counting;

/// `pin_register` built the table of every pin register (a name search over
/// the 256 ids for each, 100 K instructions) on each call; the scheduler asks
/// once per register of every unit: 1.8 M of rectwo's 11 M instructions in
/// isel, 3.5 M with the per-block copy.
#[test]
fn asking_for_a_pin_register_builds_no_table() {
    let sched = Aarch64Sched;
    assert!(sched.pin_register(reg::W1).is_some(), "the premise: it answers");
    let (_, made) = counted(|| {
        for _ in 0..50 {
            std::hint::black_box(sched.pin_register(reg::W1));
        }
    });
    assert_eq!(made, 0, "{made} allocations for 50 questions");
}
