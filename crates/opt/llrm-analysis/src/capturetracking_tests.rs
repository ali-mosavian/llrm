//! `DetermineUseCaptureKind` and `PointerMayBeCaptured`, against
//! CaptureTracking.cpp's rules.

use super::*;
use crate::testing::{function, parsed, value};

/// What `PointerMayBeCaptured` finds of `%name` in `@f` of `text`.
fn found(
    text: &str,
    name: &str,
    max_uses: usize,
) -> CaptureResult {
    let module = parsed(text);
    let function = function(&module, "f");
    pointer_may_be_captured(
        function,
        &module.globals,
        &module.context,
        value(function, name),
        CaptureComponents::ALL,
        CaptureComponents::captures_anything,
        max_uses,
    )
}

fn captured(
    text: &str,
    name: &str,
) -> bool {
    found(text, name, 0).without_ret.captures_anything()
}

#[test]
fn a_stored_pointer_is_captured_and_a_loaded_through_one_is_not() {
    let text = "define void @f(ptr %p, ptr %q, ptr %slot) {
b0:
  store ptr %p, ptr %slot
  %v = load i32, ptr %q
  store i32 1, ptr %q
  ret void
}
";
    assert!(captured(text, "p"));
    assert!(!captured(text, "q"));
}

/// A `ret` keeps the pointer only for the result with returns
/// (`CCWithRet`): a caller that discards the result is not captured by it.
#[test]
fn a_returned_pointer_is_captured_only_with_the_return() {
    let text = "define ptr @f(ptr %p) {
b0:
  %g = getelementptr i8, ptr %p, i32 4
  ret ptr %g
}
";
    let result = found(text, "p", 0);
    assert!(result.without_ret.captures_nothing());
    assert_eq!(result.with_ret, CaptureComponents::ALL);
}

#[test]
fn a_nocapture_parameter_keeps_nothing_and_an_unknown_callee_keeps_all() {
    let text = "declare void @keep(ptr)
declare void @look(ptr nocapture)
define void @f(ptr %p, ptr %q) {
b0:
  call void @keep(ptr %p)
  call void @look(ptr %q)
  ret void
}
";
    assert!(captured(text, "p"));
    assert!(!captured(text, "q"));
}

/// `p == null` keeps whether the address is null, not the address.
#[test]
fn a_comparison_with_null_keeps_only_whether_the_address_is_null() {
    let text = "define i1 @f(ptr %p, ptr %q) {
b0:
  %a = icmp eq ptr %p, null
  %b = icmp eq ptr %q, %p
  ret i1 %a
}
";
    // The use list runs newest first and the worklist pops from its back, as in
    // LLVM: `%a`, the oldest use, is explored first and the stop function
    // halts at it, before `%b` keeps the address.
    assert_eq!(found(text, "p", 0).without_ret, CaptureComponents::ADDRESS_IS_NULL);
    let only_null = "define i1 @f(ptr %p) {
b0:
  %a = icmp eq ptr %p, null
  ret i1 %a
}
";
    assert_eq!(found(only_null, "p", 0).without_ret, CaptureComponents::ADDRESS_IS_NULL);
}

/// Past `max_uses` uses the pointer is taken as captured: the pinned opt
/// agrees at the 150 default (basicaa's `capture_tracking_gives_up_...`).
#[test]
fn past_the_use_cap_a_pointer_is_taken_as_captured() {
    let reads: String = (0..3).map(|n| format!("  %v{n} = load i32, ptr %p\n")).collect();
    let text = format!("define void @f(ptr %p) {{\nb0:\n{reads}  ret void\n}}\n");
    assert!(found(&text, "p", 3).without_ret.captures_nothing(), "three uses, a cap of three");
    assert_eq!(found(&text, "p", 2).without_ret, CaptureComponents::ALL);
}
