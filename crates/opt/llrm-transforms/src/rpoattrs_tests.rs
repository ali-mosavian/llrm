use llrm_mir::passes::{ModuleAnalyses, ModulePass};

use crate::rpoattrs::ReversePostOrderFunctionAttrs;
use crate::testing::{parsed, printed};

fn run(text: &str) -> String {
    let mut module = parsed(&format!("{}{text}", llrm_analysis::testing::DOS));
    let mut analyses = ModuleAnalyses::of(&module, std::rc::Rc::new(llrm_x86_m16::Dos::default()));
    ReversePostOrderFunctionAttrs.run(&mut module, &mut analyses);
    printed(&module)
}

/// An internal function called only from `norecurse` ones is `norecurse`, down
/// a chain; one also called from a function that may recurse, or one whose
/// address is taken, is not. Post-order inference leaves each of them without
/// it, and the inliner, DSE and loop passes read it.
#[test]
fn an_internal_function_called_only_from_norecurse_ones_is_norecurse() {
    let text = "define internal void @leaf() {\nb0:\n  ret void\n}\n\n\
        define internal void @mid() {\nb0:\n  call void @leaf()\n  ret void\n}\n\n\
        define void @top() norecurse {\nb0:\n  call void @mid()\n  ret void\n}\n";
    let out = run(text);
    assert!(out.contains("define internal void @leaf() norecurse"), "{out}");
    assert!(out.contains("define internal void @mid() norecurse"), "{out}");
}

#[test]
fn a_function_with_a_caller_that_may_recurse_or_an_address_taken_is_left_alone() {
    let text = "@keep = global ptr @addr\n\n\
        define internal void @addr() {\nb0:\n  ret void\n}\n\n\
        define internal void @shared() {\nb0:\n  ret void\n}\n\n\
        define void @a() norecurse {\nb0:\n  call void @shared()\n  ret void\n}\n\n\
        define void @b() {\nb0:\n  call void @shared()\n  ret void\n}\n\n\
        define void @c() norecurse {\nb0:\n  call void @addr()\n  ret void\n}\n";
    let out = run(text);
    assert!(!out.contains("@shared() norecurse"), "{out}");
    assert!(!out.contains("@addr() norecurse"), "{out}");
}
