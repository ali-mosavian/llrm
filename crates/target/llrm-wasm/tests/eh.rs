//! Functions with an `invoke` and a `landingpad` through the whole backend in
//! the wasm exception model, against the pin's `llc -mtriple=wasm32 -O0
//! -fast-isel=false -exception-model=wasm -mattr=+exception-handling`. Each
//! `tests/eh/NAME.ll` has its text in `NAME.s` and its object in `NAME.wasm`.

use std::path::Path;

use llrm_wasm::Width;
use llrm_wasm::dag::{ExceptionModel, Options};
use llrm_wasm::driver::{assembly, object};

const MODEL: Options = Options { stackify: true, exception_model: ExceptionModel::Wasm };

/// `compare.sh`'s `norm`: comments, trailing blanks, block labels and blank
/// lines are not compared.
fn norm(text: &str) -> String {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim_end())
        .filter(|line| !line.is_empty() && !(line.starts_with(".LBB") && line.ends_with(':')))
        .map(|line| format!("{line}\n"))
        .collect()
}

fn check(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/eh");
    let source = std::fs::read_to_string(dir.join(format!("{name}.ll"))).expect("the .ll");
    let golden = std::fs::read_to_string(dir.join(format!("{name}.s"))).expect("the .s");
    let got = assembly(&source, Width::W32, MODEL).unwrap_or_else(|why| panic!("{name}: {why}"));
    assert_eq!(norm(&got), norm(&golden), "{name}");
    let bytes = std::fs::read(dir.join(format!("{name}.wasm"))).expect("the .wasm");
    assert!(object(&source, Width::W32, MODEL).expect("an object") == bytes, "{name}: the object differs");
}

/// An `invoke` was a call and its pad was dropped: no `try`, no `catch_all`, no
/// stack pointer restore (a pad the program enters through the runtime only).
#[test]
fn one_invoke_is_a_try_with_a_catch_all() {
    check("one_invoke");
}

/// Two invokes of one pad share one `try`, which ends before the code after
/// the second.
#[test]
fn two_invokes_share_a_try() {
    check("two_invokes");
}

/// The `try` encloses a loop whose body invokes; the pad sorts after the loop.
#[test]
fn an_invoke_in_a_loop_puts_the_try_around_the_loop() {
    check("loop_invoke");
}

/// A plain call between two invokes of one pad is outside what unwinds to the
/// pad: the pin wraps it in a `try` / `delegate 1` to the caller. It was
/// refused by name (and with the check off its throw would land in the pad).
#[test]
fn a_call_that_may_throw_inside_a_try_is_delegated_to_the_caller() {
    check("call_in_try");
}

/// A second pad's invoke inside the first pad's `try`: a `try` / `delegate 1`
/// to the pad that is outside, not the caller.
#[test]
fn an_invoke_of_another_pad_inside_a_try_is_delegated_to_its_pad() {
    check("two_pads");
}

/// Several calls in a row, in one block and the next, share one `try` /
/// `delegate` (a range, not a `try` each).
#[test]
fn a_run_of_calls_that_may_throw_shares_one_delegate() {
    check("call_run");
}

/// A call after the `try` of an earlier invoke and before the next invoke.
#[test]
fn a_call_between_two_invokes_of_a_pad_is_delegated() {
    check("call_after_try");
}

/// The delegate inside a loop the `try` also encloses.
#[test]
fn a_call_in_a_loop_with_an_invoke_is_delegated_inside_the_loop() {
    check("call_in_loop");
}

/// A pad whose body invokes into a second pad.
#[test]
fn a_pad_that_invokes_into_a_second_pad_is_the_pins() {
    check("nested_pads");
}
