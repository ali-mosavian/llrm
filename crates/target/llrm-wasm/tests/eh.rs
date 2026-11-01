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
