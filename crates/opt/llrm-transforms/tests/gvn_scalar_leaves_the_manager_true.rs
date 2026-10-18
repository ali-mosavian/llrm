//! The scalar part of gvn edits the body before the load part reads what the
//! manager holds of it. The registers it was handed still named the constant
//! sums the scalar pass had erased: `LLRM_CHECK_FACTS` found them stale on
//! `particle` -O2.
use llrm_analysis::manager::Registers;
use llrm_mir::passes::{Analyses, FunctionPass, PassManager, PreservedAnalyses, Unit};
use llrm_transforms::gvn::Gvn;

/// Asks for the registers and changes nothing, so the manager holds them.
struct Asks;

impl FunctionPass for Asks {
    fn name(&self) -> &'static str {
        "asks"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        analyses.get::<Registers>(unit.context, unit.layout, unit.function);
        PreservedAnalyses::all()
    }
}

#[test]
fn test_registers_are_of_the_body_the_scalar_pass_left() {
    // SAFETY: the one test of this binary, before any thread reads the flag.
    unsafe { std::env::set_var("LLRM_CHECK_FACTS", "1") };
    let text = "define i16 @f(i16 %x, ptr %p) {
b0:
  %k = add i16 50, 50
  %y = add i16 %x, %k
  store i16 %y, ptr %p
  %v = load i16, ptr %p
  ret i16 %v
}
";
    let mut module = llrm_mir::parse::module(text).expect("parses");
    let mut manager = PassManager::default();
    manager.add(Asks);
    manager.add(Gvn::default());
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
    let after = llrm_mir::print::module(&module);
    assert!(after.contains("add i16 %x, 100") && !after.contains("load"), "{after}");
}
