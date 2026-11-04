//! Each function's assumption cache scanned every global for a declaration of
//! `llvm.assume`: functions times globals, 7.5% of a 2400-function compile.
//! The scan is the module's, however many functions it has.

use llrm_mir::passes::PassManager;
use llrm_mir::valuetracking::{assume_scans, guard_scans};
use llrm_transforms::combine_pass::Combine;
use llrm_transforms::cvp::Cvp;
use llrm_transforms::inferalignment::InferAlignment;
use llrm_transforms::instsimplify::InstSimplify;
use llrm_transforms::jumpthreading::{JumpThreading, JumpThreadingOptions};
use llrm_transforms::loopinstsimplify::LoopInstSimplify;

/// Scans of the globals for `llvm.assume` and for `llvm.experimental.guard`
/// (each function's lazy value info looked) while `Combine`, `Cvp`,
/// `InstSimplify`, `LoopInstSimplify` and `InferAlignment` run over `functions`
/// bodies.
fn scans(functions: usize) -> (u64, u64) {
    let mut text = String::new();
    for at in 0..functions {
        text += &format!(
            "define i32 @f{at}(i32 %a) {{\nentry:\n  %c = icmp ult i32 %a, 10\n  br i1 %c, label %t, label %e\nt:\n  %x = add i32 %a, 1\n  br label %m\ne:\n  br label %m\nm:\n  %p = phi i32 [ %x, %t ], [ 0, %e ]\n  ret i32 %p\n}}\n"
        );
    }
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = PassManager::default();
    manager.add(Cvp);
    manager.add(JumpThreading { enabled: true, options: JumpThreadingOptions::default() });
    manager.add(Combine { enabled: true });
    manager.add(InstSimplify);
    manager.add(LoopInstSimplify);
    manager.add(InferAlignment);
    let (assume, guard) = (assume_scans(), guard_scans());
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
    (assume_scans() - assume, guard_scans() - guard)
}

#[test]
fn test_the_globals_are_scanned_for_an_assume_once_per_module() {
    let (few, many) = (scans(600), scans(1200));
    eprintln!("600 functions: {few:?} scans, 1200 functions: {many:?}");
    assert_eq!(few, many, "the scans grew with the functions");
    assert!(many.0 < 20 && many.1 < 20, "{many:?} scans");
}
