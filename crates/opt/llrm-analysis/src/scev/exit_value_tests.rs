//! `computeExitCountExhaustively`: loops of constants run until the exit test
//! is decided. Counts as `opt -passes='print<scalar-evolution>'` at the pin.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::valuetracking::AssumptionCache;

use super::backedge::ExitCountKind;
use super::{Cx, ScalarEvolution};

/// The exact exit count of the block named `exiting` in the loop headed by
/// `loop` of `text`'s @f, as a constant if it is one.
fn exit_count(
    text: &str,
    exiting: &str,
) -> Option<u128> {
    let module = llrm_mir::parse::module(text).expect("a module");
    let (_, _, function) = module
        .functions()
        .find(|(_, global, function)| global.name.as_deref() == Some("f") && function.entry().is_some())
        .expect("@f");
    let layout = DataLayout::default();
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(&module, function);
    let cx = Cx {
        context: &module.context,
        globals: &module.globals,
        metadata: &module.metadata,
        function,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let named = |name: &str| {
        *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some(name)).expect(name)
    };
    let (header, exiting) = (named("loop"), named(exiting));
    let mut se = ScalarEvolution::new();
    let count = se.get_exit_count(&cx, header, exiting, ExitCountKind::Exact);
    se.as_constant(count).map(|c| c.zext_value())
}

/// LLVM's `ult_const_max` (eliminate-exit.ll): `udiv iv, 10 <u 2` leaves when
/// `iv` reaches 20. The exit count was CouldNotCompute, because the exhaustive
/// count was a stub; `opt` prints `exit count for loop: i32 20`.
#[test]
fn test_an_exit_on_a_quotient_of_the_counter_is_counted_by_running_the_loop() {
    let text = "declare void @side_effect()
define void @f(i64 %n) {
entry:
  br label %loop
loop:
  %iv = phi i64 [ 0, %entry ], [ %iv.next, %latch ]
  %iv.next = add i64 %iv, 1
  %udiv = udiv i64 %iv, 10
  %cmp1 = icmp ult i64 %udiv, 2
  br i1 %cmp1, label %latch, label %exit
latch:
  call void @side_effect()
  %cmp2 = icmp ult i64 %iv, %n
  br i1 %cmp2, label %loop, label %exit
exit:
  ret void
}
";
    assert_eq!(exit_count(text, "loop"), Some(20));
    assert_eq!(exit_count(text, "latch"), None);
}

/// A counter that never reaches the exit within 100 iterations is not counted.
#[test]
fn test_a_loop_longer_than_the_brute_force_limit_is_not_counted_by_running_it() {
    let text = "define void @f() {
entry:
  br label %loop
loop:
  %iv = phi i64 [ 0, %entry ], [ %iv.next, %loop ]
  %iv.next = add i64 %iv, 1
  %q = udiv i64 %iv, 10
  %c = icmp ult i64 %q, 20
  br i1 %c, label %loop, label %exit
exit:
  ret void
}
";
    assert_eq!(exit_count(text, "loop"), None);
}

/// A float-controlled loop is counted by running it: `x` steps by 0.25 and the
/// loop leaves when `x + 0.25 >= 3.0`, so the test fails on the 12th trip. The
/// `fadd` of two constants was never folded by `constantfolding.rs` (FP binary
/// operations were owed), so the exhaustive run stopped at once and the exit
/// count was CouldNotCompute; `opt` prints 11 for the backedge-taken count.
#[test]
fn test_an_exit_on_a_float_recurrence_is_counted_by_running_the_loop() {
    let text = "define void @f(ptr %p) {
b0:
  br label %loop
loop:
  %x = phi float [ 0.0, %b0 ], [ %x1, %loop ]
  %x1 = fadd float %x, 0.25
  store float %x, ptr %p
  %c = fcmp olt float %x1, 3.0
  br i1 %c, label %loop, label %exit
exit:
  ret void
}
";
    assert_eq!(exit_count(text, "loop"), Some(11));
}
