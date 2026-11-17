//! `optimizeExtendOrTruncateConversion`'s zext arm on the arm64 cost model.

use crate::cgpaddr::CgpAddr;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\n";

fn run(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager
        .run_module(
            &mut module,
            std::rc::Rc::new(llrm_arm64::machine::Arm64Machine { platform: llrm_arm64::machine::Platform::Linux }),
        )
        .unwrap();
    printed(&module)
}

const LOOP: &str = "define void @f(ptr noalias %a, ptr noalias %b, i64 %n) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %s = getelementptr inbounds <8 x i8>, ptr %a, i64 %i
  %v = load <8 x i8>, ptr %s, align 8
  %w = zext <8 x i8> %v to <8 x i32>
  %d = getelementptr inbounds <8 x i32>, ptr %b, i64 %i
  store <8 x i32> %w, ptr %d, align 32
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, %n
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
";

/// A `zext <8 x i8> to <8 x i32>` in a loop's header stayed a `zext`, and the
/// loop kept `ushll` + `ushll2` + masks where the pin's `tbl` shuffle loads
/// them once.
#[test]
fn a_zext_of_bytes_in_a_loop_header_is_the_shuffle_that_tbl_makes() {
    let text = run(LOOP);
    assert!(!text.contains("zext"), "{text}");
    assert!(text.contains("shufflevector <8 x i8>"), "{text}");
    assert!(text.contains("bitcast <32 x i8>"), "{text} ");
}

/// The pin keeps the extension outside a loop header: the masks would be code
/// for one use.
#[test]
fn a_zext_outside_a_loop_stays() {
    let text = run("define void @f(ptr noalias %a, ptr noalias %b) {
  %v = load <8 x i8>, ptr %a, align 8
  %w = zext <8 x i8> %v to <8 x i32>
  store <8 x i32> %w, ptr %b, align 32
  ret void
}
");
    assert!(text.contains("zext <8 x i8>"), "{text}");
    assert!(!text.contains("shufflevector"), "{text}");
}

/// The masks are loads: a function optimized for size keeps the extension.
#[test]
fn a_zext_in_an_optsize_function_stays() {
    let text = run(&LOOP.replace("{\nentry:", "optsize {\nentry:"));
    assert!(text.contains("zext <8 x i8>"), "{text}");
}
