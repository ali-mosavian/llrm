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

/// An `invoke`'s unwind edge was a successor of its block, so a pad nothing
/// else branches to stayed in the function: `i32.const 2; return` after the
/// normal path's `return`. The pin drops it (`LowerInvoke`).
#[test]
fn an_unwind_edge_is_not_an_edge() {
    let text = "declare void @g(i32)\ndeclare i32 @p(...)\ndefine i32 @f(i32 %a) personality ptr @p {\nentry:\n  invoke void @g(i32 %a) to label %ok unwind label %pad\nok:\n  ret i32 1\npad:\n  %l = landingpad { ptr, i32 } cleanup\n  ret i32 2\n}\n";
    let out = assembly(text, Width::W32, Options::default()).expect("compiles");
    assert!(!out.contains("i32.const\t2"), "{out}");
}

/// A landing pad whose value is read (`extractvalue` of the exception) has no
/// lowering here: it must stop at compile time, not drop the value.
#[test]
fn a_read_landing_pad_value_is_refused_by_name() {
    let why = refusal(
        "declare void @g(i32)\ndeclare i32 @p(...)\ndefine i32 @f(i32 %a) personality ptr @p {\nentry:\n  invoke void @g(i32 %a) to label %ok unwind label %pad\n\
         ok:\n  ret i32 1\npad:\n  %l = landingpad { ptr, i32 } cleanup\n  %s = extractvalue { ptr, i32 } %l, 1\n  ret i32 %s\n}\n",
    );
    assert!(why.contains("landing pad"), "{why}");
}
