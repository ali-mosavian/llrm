//! A pass that asks for the LazyValueInfo cache and edits nothing leaves it
//! standing. `LLRM_CHECK_PRESERVED` called that stale (`fixednarrow left lvi
//! held but stale` on nbody_fixed and stft_fixed): it compared the kept cache
//! with a fresh, empty one by identity.
use llrm_mir::lazyvalueinfo::LviCache;
use llrm_mir::passes::{Analyses, FunctionPass, PassManager, PreservedAnalyses, Unit};

struct AsksLvi;

impl FunctionPass for AsksLvi {
    fn name(&self) -> &'static str {
        "asks-lvi"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        analyses.get::<LviCache>(unit.context, unit.layout, unit.function);
        PreservedAnalyses::all()
    }
}

#[test]
fn test_a_cache_the_pass_leaves_standing_passes_the_preserved_oracle() {
    // SAFETY: the one test of this binary, before any thread reads the flag.
    unsafe {
        std::env::set_var("LLRM_CHECK_PRESERVED", "1");
    }
    let text = "define i16 @f(i16 %x) {
b0:
  %y = add i16 %x, 1
  ret i16 %y
}
";
    let mut module = llrm_mir::parse::module(text).expect("parses");
    let mut manager = PassManager::default();
    manager.add(AsksLvi);
    manager.add(AsksLvi);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
}
