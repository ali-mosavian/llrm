use super::returns_duplicated;
use crate::testing::{parsed, printed};

fn after(text: &str) -> (bool, String) {
    let mut module = parsed(text);
    let llrm_mir::module::Module { context, globals, .. } = &mut module;
    let all: Vec<llrm_mir::module::GlobalValue> = globals.to_vec();
    let mut changed = false;
    for global in globals.iter_mut() {
        if let llrm_mir::module::GlobalKind::Function(function) = &mut global.kind {
            changed |= returns_duplicated(context, &all, function);
        }
    }
    (changed, printed(&module))
}

/// x_mergesort `msort` -Os: the late path ends in `tail call memcpy; br ret`
/// and the early exit branches to the same `ret void`. The pin's CGP gives
/// the late path its own `ret`, so its restore point is a block of its own
/// and ShrinkWrap finds none (the pin does not wrap `msort`; ours did, +9 B).
const MSORT: &str = "declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)

define void @f(i32 %n, ptr %a, ptr %b) {
b0:
  %c = icmp slt i32 %n, 2
  br i1 %c, label %done, label %late

late:
  tail call void @llvm.memcpy.p0.p0.i32(ptr %a, ptr %b, i32 %n, i1 false)
  br label %done

done:
  ret void
}
";

#[test]
fn test_a_return_after_a_tail_call_is_copied_into_the_block_that_calls() {
    let (changed, text) = after(MSORT);
    assert!(changed, "{text}");
    assert_eq!(text.matches("ret void").count(), 2, "{text}");
    assert!(!text.contains("br label %done"), "{text}");
}

#[test]
fn test_a_call_that_is_no_tail_call_keeps_the_shared_return() {
    let (changed, text) = after(&MSORT.replace("tail call", "call"));
    assert!(!changed, "{text}");
    assert_eq!(text.matches("ret void").count(), 1, "{text}");
}

/// The phi of a returned value: each arm that is a tail call's result gets
/// the call's value returned in place.
#[test]
fn test_a_phi_of_tail_call_results_returns_each_result_in_its_block() {
    let (changed, text) = after(
        "declare i32 @g(i32)
declare i32 @h(i32)

define i32 @f(i32 %x) {
b0:
  %c = icmp eq i32 %x, 0
  br i1 %c, label %a, label %b

a:
  %r1 = tail call i32 @g(i32 %x)
  br label %done

b:
  %r2 = tail call i32 @h(i32 %x)
  br label %done

done:
  %r = phi i32 [ %r1, %a ], [ %r2, %b ]
  ret i32 %r
}
",
    );
    assert!(changed, "{text}");
    assert!(text.contains("ret i32 %r1") && text.contains("ret i32 %r2"), "{text}");
    assert!(!text.contains("phi"), "{text}");
}
