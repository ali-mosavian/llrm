//! A chain of equalities of adjacent bytes with constants is one wide load
//! and one compare, as the pin's SLP vectorizer leaves it.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn body(name: &str) -> String {
    let asm = driver::assembly(include_str!("data/slp_compares.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)].to_owned()
}

fn count(
    text: &str,
    mnemonic: &str,
) -> usize {
    text.lines().filter(|line| line.split_whitespace().next() == Some(mnemonic)).count()
}

/// The eight compares of `COMSPEC=` in `B$SSHL` were eight `ldrb`, a `cmp` and
/// seven `ccmp`s with their constants held in callee-saved registers (+7
/// instructions against clang): clang loads once and compares once.
#[test]
fn eight_bytes_are_one_load_and_one_compare() {
    let text = body("comspec");
    assert_eq!(count(&text, "ldrb"), 0, "{text}");
    assert_eq!(count(&text, "ldr"), 1, "{text}");
    assert!(count(&text, "ccmp") <= 1, "{text}");
}

#[test]
fn four_bytes_are_one_word() {
    let text = body("four");
    assert_eq!((count(&text, "ldrb"), count(&text, "ldr")), (0, 1), "{text}");
}

/// The pin keeps two bytes scalar.
#[test]
fn two_bytes_stay_bytes() {
    assert_eq!(count(&body("two"), "ldrb"), 2);
}

/// A call between the loads may change the bytes, so the pairs stay.
#[test]
fn bytes_either_side_of_a_call_are_not_merged() {
    assert_eq!(count(&body("stored_between"), "ldrb"), 4);
}

/// Under an `or` a leaf is not a conjunct of the chain.
#[test]
fn bytes_under_an_or_are_not_merged_into_the_chain() {
    let text = body("under_or");
    assert_eq!(count(&text, "ldrb"), 4, "{text}");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn the_answers_are_the_ones_the_byte_compares_give() {
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let llc = std::path::Path::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/slp_compares.ll");
    let object = dir.join("f.o");
    let said = std::process::Command::new(llc).args(["-O2", "-o"]).arg(&object).arg(ir).output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let driver = dir.join("main.c");
    std::fs::write(&driver, include_str!("data/slp_compares_main.c")).unwrap();
    let program = dir.join("prog");
    let built = std::process::Command::new("cc").arg(&driver).arg(&object).arg("-o").arg(&program).output().unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let ran = std::process::Command::new(&program).output().unwrap();
    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stdout));
}
