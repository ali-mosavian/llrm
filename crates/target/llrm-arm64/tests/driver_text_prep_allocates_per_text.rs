//! Reading the IR text allocates per text, not per line.

use llrm_arm64::driver::without_triple;
use llrm_support::alloc_count::{Counting, counted};

#[global_allocator]
static COUNTING: Counting = Counting;

/// `without_triple` ran two `replace`s on every line (912 lines of queens, 0.75
/// M of 57 M instructions, two allocations a line) although almost none
/// mentions `dso_local` or `ptrtoaddr`.
#[test]
fn preparing_the_ir_text_allocates_per_text_not_per_line() {
    let text = "target triple = \"aarch64-unknown-linux-gnu\"\n; a comment\n".to_string()
        + &"  %a = add i64 %x, %y\n".repeat(500)
        + "@g = dso_local global i32 0\n  %b = ptrtoaddr ptr %p to i64\n";
    let ((triple, kept), made) = counted(|| without_triple(&text));
    assert_eq!(triple.as_deref(), Some("aarch64-unknown-linux-gnu"), "the premise: the triple is read");
    assert!(
        kept.contains("@g = global i32 0") && kept.contains("= ptrtoint ptr %p"),
        "the premise: both are rewritten\n{kept}"
    );
    assert!(
        !kept.contains("; a comment") && !kept.contains("target triple"),
        "the premise: comments and the triple go"
    );
    assert!(made <= 12, "{made} allocations for 503 lines");
}
