//! A register past the budget costs a reload at each reference (DEVIATE LU3e).
use std::rc::Rc;

use llrm_transforms::lsr_pass::LoopStrengthReduce;

fn reduced(text: &str) -> String {
    let mut module = llrm_mir::parse::module(text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopStrengthReduce { size: false });
    let target: Rc<dyn llrm_mir::target::Machine> = Rc::new(llrm_x86_m16::Dos::pin_tuned());
    manager.run_module(&mut module, target).expect("runs");
    llrm_mir::print::module(&module)
}

/// grep's byte loop carries six values besides its counter. LSR ran it
/// to zero from the negated length with the end pointer live (`mul i16 n, -1`);
/// the allocator spilled both every trip: m16 -O1 611 B against 600 B.
#[test]
fn a_loop_with_six_carried_values_does_not_count_up_to_zero_from_a_live_end() {
    let out = reduced(include_str!("lsr_spill/grep_p16.ll"));
    assert!(!out.lines().any(|line| line.contains("mul i16") && line.ends_with(", -1")), "{out}");
}
