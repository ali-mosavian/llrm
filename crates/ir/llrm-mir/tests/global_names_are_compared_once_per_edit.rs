//! `GlobalNames` compared every global's name with its own after each function
//! body: 104 M self Ir (215 M with its callers) over a 2400-function compile,
//! 16 times the 600-function figure. While no name has been made, it stands
//! without a compare.

use std::rc::Rc;

use llrm_mir::module::GlobalName;
use llrm_mir::passes::{GlobalNames, ModuleAnalyses, PreservedAnalyses, name_compares};
use llrm_mir::{parse, target::Neutral};

fn module(functions: usize) -> llrm_mir::module::Module {
    let text: String = (0..functions).map(|at| format!("declare void @f{at}()\n")).collect();
    parse::module(&text).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn test_a_thousand_function_bodies_compare_the_global_names_once_at_most() {
    let mut module = module(600);
    let mut analyses = ModuleAnalyses::of(&module, Rc::new(Neutral));
    let first = analyses.get::<GlobalNames>(&module);
    let before = name_compares();
    for _ in 0..1000 {
        analyses.invalidate(&PreservedAnalyses::none());
        assert!(Rc::ptr_eq(&first, &analyses.get::<GlobalNames>(&module)), "made again");
    }
    assert_eq!(name_compares() - before, 0, "compared per body");

    // A name made, the same names: one compare, then none.
    module.globals[3].name = GlobalName::from("f3".to_owned());
    for _ in 0..1000 {
        analyses.invalidate(&PreservedAnalyses::none());
        analyses.get::<GlobalNames>(&module);
    }
    assert_eq!(name_compares() - before, 1);

    // A rename: the names are made again.
    module.globals[3].name = GlobalName::from("renamed".to_owned());
    analyses.invalidate(&PreservedAnalyses::none());
    let renamed = analyses.get::<GlobalNames>(&module);
    assert_eq!(renamed[3].as_deref(), Some("renamed"));
}
