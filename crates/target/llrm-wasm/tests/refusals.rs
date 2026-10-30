//! What the backend cannot do yet stops at compile time with a name: a program
//! that needs it must not compile to something else.

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::assembly;

fn refusal(text: &str) -> String {
    assembly(text, Width::W32, Options::default()).expect_err("refused")
}

/// A loop with two entries (`a` and `b` jump to each other, `entry` reaches
/// both) went through `CFGSort` and `CFGStackify`, which assume natural loops,
/// and came out as wasm for another program.
#[test]
fn irreducible_control_flow_is_refused_by_name() {
    let why = refusal(
        "define i32 @f(i1 %c, i1 %d, i1 %e) {\nentry:\n  br i1 %c, label %a, label %b\na:\n  br i1 %d, label %b, label %out\n\
         b:\n  br i1 %e, label %a, label %out\nout:\n  ret i32 0\n}\n",
    );
    assert!(why.contains("irreducible control flow"), "{why}");
}

/// Every `switch`, whatever its density, stopped on 'a switch lowering made
/// blocks': the reader could not tell it from an internal fault.
#[test]
fn switch_is_refused_by_name() {
    let why = refusal(
        "define i32 @f(i32 %x) {\nentry:\n  switch i32 %x, label %d [ i32 1, label %one i32 2, label %two ]\none:\n  ret i32 10\n\
         two:\n  ret i32 20\nd:\n  ret i32 0\n}\n",
    );
    assert!(why.contains("switch") && why.contains("not supported yet"), "{why}");
}
