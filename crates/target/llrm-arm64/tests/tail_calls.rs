//! Calls in tail position.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// The text of `function`, up to the next one.
fn body(name: &str) -> String {
    body_at(name, Level::O2)
}

fn body_at(
    name: &str,
    level: Level,
) -> String {
    let asm =
        driver::assembly(include_str!("data/tail_calls.ll"), Platform::Darwin, level).expect("the functions lower");
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)].to_owned()
}

/// A `tail call` followed by `ret` was `bl` and `ret` in a frame of its own;
/// clang branches (`b _callee`).
#[test]
fn a_tail_call_is_a_branch() {
    let asm = body("forwards");
    assert!(asm.contains("\tb _callee"), "{asm}");
    for left in ["bl ", "ret", "stp"] {
        assert!(!asm.contains(left), "{left} in\n{asm}");
    }
}

/// An indirect callee is a branch through a register too (`sibcall.rs`).
#[test]
fn a_tail_call_through_a_pointer_is_a_register_branch() {
    let asm = body("through_a_pointer");
    assert!(asm.contains("\tbr x") && !asm.contains("blr"), "{asm}");
}

#[test]
fn a_call_not_marked_tail_stays_a_call() {
    // The optimizer marks what it can; unoptimized, the marker is the source's.
    let asm = body_at("unmarked", Level::O0);
    assert!(asm.contains("bl _callee"), "{asm}");
}

/// Arguments on the stack are the caller's frame: no branch.
#[test]
fn a_call_with_stack_arguments_stays_a_call() {
    let asm = body("on_the_stack");
    assert!(asm.contains("bl _many"), "{asm}");
}

/// The frame is taken down before the branch, and the saved registers come back
/// first.
#[test]
fn the_frame_is_restored_before_the_branch() {
    let asm = body("after_a_call");
    let restore = asm.find("ldp").expect("the frame record is restored");
    assert!(asm.find("\tb _callee").expect("a branch") > restore, "{asm}");
    assert!(!asm.contains("ret"), "{asm}");
}

/// A `byval` argument is one of the gate's refusals
/// (`isEligibleForTailCallOptimization`): it stays `bl`. A convention the pin has
/// no table for is a compile error (`dag_refusal_is_an_error.rs`). The
/// caller's own hidden result pointer, passed on in `x8`, is not one: the pin's
/// `llc` makes `b _fill` of `sret_callee` (`x8` is no stack argument).
#[test]
fn byval_stays_a_call_and_sret_is_forwarded() {
    let asm = body("byval_argument");
    assert!(asm.contains("bl _shape"), "{asm}");
    let asm = body("sret_callee");
    assert!(asm.contains("b _fill") && !asm.contains("bl _fill"), "{asm}");
}
