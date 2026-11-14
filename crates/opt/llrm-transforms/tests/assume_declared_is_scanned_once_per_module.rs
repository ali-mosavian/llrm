//! Each function's assumption cache scanned every global for a declaration of
//! `llvm.assume`: functions times globals, 7.5% of a 2400-function compile.
//! The scan is the module's, however many functions it has.

use llrm_mir::passes::PassManager;
use llrm_mir::valuetracking::{assume_scans, guard_scans};
use llrm_transforms::combine_pass::Combine;
use llrm_transforms::constraintelim::ConstraintElimination;
use llrm_transforms::cvp::Cvp;
use llrm_transforms::inferalignment::InferAlignment;
use llrm_transforms::instsimplify::InstSimplify;
use llrm_transforms::jumpthreading::{JumpThreading, JumpThreadingOptions};
use llrm_transforms::loopinstsimplify::LoopInstSimplify;
use llrm_transforms::loopunrollpass::LoopFullUnroll;
use llrm_transforms::simpleloopunswitch::SimpleLoopUnswitch;

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
    manager.add(Combine { enabled: true, phi_constant_users: false });
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
    // The counters are the process's: the other tests here add a few scans
    // while this one runs; a scan per function adds hundreds.
    assert!(many.0 < few.0 + 50 && many.1 < few.1 + 50, "the scans grew with the functions");
    assert!(many.0 < 20 && many.1 < 20, "{many:?} scans");
}

/// `LoopFullUnroll` and `SimpleLoopUnswitch` built each loop's assumption cache
/// with `AssumptionCache::of`, which scanned every global: 1.64x and 2.23x per
/// call from 600 to 2400 functions (tiny, x86).
#[test]
fn test_the_loop_passes_do_not_scan_the_globals_for_each_loop() {
    let scans = |functions: usize| {
        let mut text = String::new();
        for at in 0..functions {
            text += &format!(
                "define i32 @f{at}(ptr %p, i32 %n) {{\nentry:\n  br label %l\nl:\n  %i = phi i32 [ 0, %entry ], [ %j, %l ]\n  %s = phi i32 [ 0, %entry ], [ %t, %l ]\n  %q = getelementptr i32, ptr %p, i32 %i\n  %v = load i32, ptr %q\n  %t = add i32 %s, %v\n  %j = add i32 %i, 1\n  %c = icmp slt i32 %j, %n\n  br i1 %c, label %l, label %x\nx:\n  ret i32 %t\n}}\n"
            );
        }
        let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
        let mut manager = PassManager::default();
        manager.add(LoopFullUnroll {
            opt_level: 2,
            opt_size: false,
            only_when_forced: false,
            target_unrolling: true,
            inliner_follows: true,
            speed_rate: false,
            created: Default::default(),
            decided: Default::default(),
        });
        manager.add(SimpleLoopUnswitch { non_trivial: true });
        let before = assume_scans();
        manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
        assume_scans() - before
    };
    let (few, many) = (scans(300), scans(600));
    // The counter is the process's: the other tests here add a few scans
    // while this one runs; a scan per function adds hundreds.
    assert!(many < few + 50 && many < 100, "the scans grew with the functions: {few} then {many}");
}

/// `ConstraintElimination` built the induction analyses' assumption cache with
/// `AssumptionCache::of`, a scan of every global per function (tiny x86,
/// 600 -> 2400 functions: 57M -> 260M instructions, exponent 1.12).
#[test]
fn test_constraint_elimination_does_not_scan_the_globals_for_each_function() {
    let scans = |functions: usize| {
        let mut text = String::new();
        for at in 0..functions {
            text += &format!(
                "define i32 @f{at}(i32 %a) {{\nentry:\n  %c = icmp ult i32 %a, 10\n  br i1 %c, label %t, label %e\nt:\n  ret i32 1\ne:\n  ret i32 0\n}}\n"
            );
        }
        let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
        let mut manager = PassManager::default();
        manager.add(ConstraintElimination);
        let before = assume_scans();
        manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
        assume_scans() - before
    };
    let (few, many) = (scans(300), scans(600));
    // The counter is the process's: the other tests here add a few scans
    // while this one runs; a scan per function adds hundreds.
    assert!(many < few + 50 && many < 100, "the scans grew with the functions: {few} then {many}");
}
