//! The manager keeps no map of constants, so the seeds `Decide` hands the
//! constant-cycle propagation are empty. `LLRM_CHECK_CYCLES` asserted that a
//! body with no open phi came to no more than its seeds and panicked on
//! `bench/parity/qmove` (bq32, -Os): the propagation found the constants
//! itself.
use llrm_mir::passes::PassManager;
use llrm_transforms::decide::Decide;

#[test]
fn test_a_body_with_no_phi_and_no_seeds_is_decided_under_the_cycles_oracle() {
    // SAFETY: the one test of this binary, before any thread reads the flag.
    unsafe {
        std::env::set_var("LLRM_CHECK_CYCLES", "1");
    }
    // The body `bench/parity/qmove` reaches decide@809 with: sums of constants
    // no pass has folded yet.
    let text = "define i32 @f() {
b0:
  %0 = mul i32 10, 10000
  %1 = add i32 %0, 405
  ret i32 %1
}
";
    let mut module = llrm_mir::parse::module(text).expect("parses");
    let mut manager = PassManager::default();
    manager.add(Decide { constants: true });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
    assert!(llrm_mir::print::module(&module).contains("ret i32"));
}
