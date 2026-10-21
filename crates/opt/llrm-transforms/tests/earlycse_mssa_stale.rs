//! `earlycse` left MemorySSA held but stale (`LLRM_CHECK_PRESERVED`) on the
//! body of `bench/stft_fixed` at -O3 (`pipeline.body`, pass 649).
use llrm_mir::passes::PassManager;
use llrm_transforms::earlycse::EarlyCse;

#[test]
fn test_earlycse_keeps_only_a_memoryssa_that_is_still_true() {
    // SAFETY: the one test of this binary, before any thread reads the flag.
    unsafe {
        std::env::set_var("LLRM_CHECK_PRESERVED", "1");
    }
    let text = include_str!("fixtures/earlycse_leaves_mssa_stale.ll");
    let mut module = llrm_mir::parse::module(text).expect("parses");
    let mut manager = PassManager::default();
    manager.add(EarlyCse);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
}
