//! What the backend cannot do yet stops at compile time with a name: a program
//! that needs it must not compile to something else.

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::assembly;

fn refusal(text: &str) -> String {
    assembly(text, Width::W32, Options::default()).expect_err("refused")
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

/// An object aligned past the stack's 16 needs stack realignment (a base
/// pointer): refused by name, not laid out at 16.
#[test]
fn an_over_aligned_object_is_refused_by_name() {
    let why = refusal(
        "declare void @take(ptr)\ndefine void @f() {\n  %a = alloca i32, align 32\n  call void @take(ptr %a)\n  ret void\n}\n",
    );
    assert!(why.contains("stack realignment"), "{why}");
}

/// `-exception-model=wasm` is the EH port's switch (stage 0): a function with
/// an `invoke` or a `landingpad` is refused by name until the stages that
/// lower them land, not compiled as if there were no unwinding; and the model
/// changes nothing for a function without either, which stays the pin's
/// `-exception-model=none` output.
#[test]
fn the_wasm_exception_model_refuses_unwinding_and_leaves_the_rest_alone() {
    use llrm_wasm::dag::ExceptionModel;

    let model = Options { exception_model: ExceptionModel::Wasm, ..Options::default() };
    let unwinding = "declare void @g(i32)\ndeclare i32 @p(...)\ndefine i32 @f(i32 %a) personality ptr @p {\nentry:\n  invoke void @g(i32 %a) to label %ok unwind label %pad\nok:\n  ret i32 1\npad:\n  %l = landingpad { ptr, i32 } cleanup\n  ret i32 2\n}\n";
    let why = assembly(unwinding, Width::W32, model).expect_err("refused");
    assert!(why.contains("exception handling is not translated yet"), "{why}");
    let plain = "define i32 @f(i32 %a) {\n  %b = add i32 %a, 1\n  ret i32 %b\n}\n";
    assert_eq!(assembly(plain, Width::W32, model), assembly(plain, Width::W32, Options::default()));
}
