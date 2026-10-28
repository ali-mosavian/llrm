//! `ccmp` chains for a select on an `and`/`or` tree of compares: the sequence
//! the pin emits, and the values it gives (built natively and compared with C
//! over a grid, where the host is arm64 macOS).

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn body(name: &str) -> String {
    let asm = driver::assembly(include_str!("data/conditional_compare.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)].to_owned()
}

/// `x == 72 || x == 79` selected two `cset`s, an `orr` and a `tst`: clang's
/// `cmp`, `ccmp`, `csel` (`fin_number`, +67 instructions over clang).
#[test]
fn an_or_of_compares_in_a_select_is_a_ccmp_chain() {
    let text = body("eq_or");
    assert!(text.contains("ccmp w0, w9, #4, ne"), "{text}");
    assert!(!text.contains("csinc"), "no cset: {text}");
}

#[test]
fn an_and_of_compares_in_a_select_is_a_ccmp_chain() {
    let text = body("and_and");
    assert!(text.contains("ccmp w0, #3, #4, lt"), "{text}");
}

/// `x == 72` read by the `or` and by a second select: clang still makes the
/// `cmp`/`ccmp` pair for the first and a `cmp` for the second (`fin_number`),
/// because the second is a `select_cc` of the operands.
#[test]
fn a_compare_that_another_select_also_reads_still_joins_the_chain() {
    let text = body("shared_leaf");
    assert_eq!(text.matches("ccmp").count(), 1, "{text}");
    assert_eq!(text.matches("subs wzr").count(), 2, "{text}");
}

#[test]
fn a_tree_that_must_start_with_its_and_is_a_chain_of_two_ccmps() {
    let text = body("mixed");
    assert_eq!(text.matches("ccmp").count(), 2, "{text}");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn the_chains_give_the_values_c_does() {
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let llc = std::path::Path::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/conditional_compare.ll");
    let object = dir.join("f.o");
    let said = std::process::Command::new(llc).args(["-O2", "-o"]).arg(&object).arg(ir).output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let driver = dir.join("main.c");
    std::fs::write(&driver, include_str!("data/conditional_compare.c")).unwrap();
    let program = dir.join("prog");
    let built = std::process::Command::new("cc").arg(&driver).arg(&object).arg("-o").arg(&program).output().unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let ran = std::process::Command::new(&program).output().unwrap();
    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stdout));
}

/// `if (x >= left && y >= top && x <= right && y <= bottom)` was four
/// compare-and-branch pairs (8 instructions): the pin keeps the compares of a
/// branch together when the right sides are cheap
/// (`shouldKeepJumpConditionsTogether`, AArch64's budget of 8), giving `cmp`
/// and three `ccmp` (`gfx_plot`).
#[test]
fn a_branch_on_cheap_compares_is_a_ccmp_chain() {
    let asm =
        driver::assembly(include_str!("data/conditional_compare_branch.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find("_plot:").expect("the function is there");
    let text = &asm[start..];
    let text = &text[..text.find("\n_").unwrap_or(text.len())];
    assert_eq!(text.matches("ccmp").count(), 3, "{text}");
    assert_eq!(text.lines().filter(|line| line.trim_start().starts_with("b.")).count(), 1, "{text}");
}

/// A pointer compare was no leaf of a `ccmp` chain (`int_bits` of a pointer is
/// none), so `p < end` joined with a bit test stayed a `csinc`, a `csel` and a
/// `tst` (`str_take`, +4 over clang): the pin gives `cmp` and `ccmp`.
#[test]
fn a_pointer_compare_is_a_ccmp_leaf() {
    let asm =
        driver::assembly(include_str!("data/conditional_compare_pointer.ll"), Platform::Darwin, true).expect("lowers");
    assert!(asm.contains("ccmp x") && asm.contains(", #0, #4, hs"), "{asm}");
}
