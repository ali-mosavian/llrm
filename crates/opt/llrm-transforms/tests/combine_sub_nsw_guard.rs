//! `if (n >= 32)` guard of `from_mag` (sfm16_dbg -O1 +14 B at wasm): after
//! `%31 = icmp slt %30, 1` became `icmp sgt %26, 4`, `%33 = icmp sgt %30, 31`
//! (`%30 = sub nsw 5, %26`) must become `icmp slt %26, -26` as the pin's does,
//! and not `samesign ugt %30, 31`, which no later fold takes apart.
use llrm_mir::passes::PassManager;
use llrm_transforms::combine_pass::Combine;

#[test]
fn test_a_sub_nsw_guard_folds_to_a_signed_compare_of_its_operand() {
    let text = include_str!("fixtures/shift_guard_chain.ll");
    let mut module = llrm_mir::parse::module(text).expect("parses");
    let mut manager = PassManager::default();
    manager.add(Combine { enabled: true });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
    let after = llrm_mir::print::module(&module);
    assert!(!after.contains("samesign ugt i32 %30, 31"), "{after}");
    assert!(after.contains("icmp slt i32 %26, -26") && !after.contains("%30, 31"), "{after}");
}
