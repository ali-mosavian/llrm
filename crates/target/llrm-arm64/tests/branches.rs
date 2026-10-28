//! How the selector branches.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn asm() -> String {
    driver::assembly(include_str!("data/or_of_compares.ll"), Platform::Darwin, true).expect("the functions lower")
}

/// `n == 0 || m == 0` made a bit of each comparison (subs, cset, subs, cset,
/// orr, tst, b.ne: seven instructions); clang branches on each in turn
/// (tests/llc rectwo: 196 B against 132).
#[test]
fn a_chain_of_comparisons_is_a_chain_of_branches() {
    let asm = asm();
    for set in ["cset", "csinc", "orr", "and\t", "ands"] {
        assert!(!asm.contains(set), "{set} left in\n{asm}");
    }
}

/// An `or` over two comparisons with zero is two `cbz`.
#[test]
fn an_or_of_zero_tests_is_two_compare_branches() {
    let asm = asm();
    let either = asm.split("_both:").next().unwrap();
    assert_eq!(either.matches("cbz").count(), 2, "{either}");
}

/// `br (select i1 a, b, false)` and `(select i1 a, true, b)` are LLVM's logical
/// `and` and `or` (`m_LogicalAnd`/`m_LogicalOr`), what clang emits for
/// `&&`/`||`; they made a bit (`cset`, `csel`, `tst`) where clang branches on
/// each comparison: QB `cn_pos` 42 instructions against 33.
#[test]
fn a_logical_and_or_or_of_comparisons_is_a_chain_of_branches() {
    let asm = driver::assembly(include_str!("data/logical_and_or_of_compares.ll"), Platform::Darwin, true)
        .expect("the functions lower");
    for set in ["cset", "csel", "csinc", "tst\t"] {
        assert!(!asm.contains(set), "{set} left in\n{asm}");
    }
    assert_eq!(asm.matches("cbz").count() + asm.matches("cbnz").count(), 4, "{asm}");
}

/// A call result marked `zeroext`/`signext` is already extended, so returning
/// it needs no extension and the call is a jump: QB `cn_pos` ended `blr; and
/// w0, w0, #0xff; ret` where clang has `br`.
#[test]
fn a_returned_extended_call_result_is_a_tail_call() {
    let asm = driver::assembly(include_str!("data/zeroext_call_result.ll"), Platform::Darwin, true)
        .expect("the functions lower");
    for left in ["blr", "and\t", "sxth", "uxtb"] {
        assert!(!asm.contains(left), "{left} left in\n{asm}");
    }
    assert_eq!(asm.matches("\tbr x").count(), 2, "{asm}");
}

/// A loop of a call and a switch over a dozen globals made each global's page
/// again in every arm (41 `adrp` in `text_write`, clang 10): the hoist took
/// every register the loop's carried values were guessed to hold (19 guessed,
/// room for 10) and so hoisted none. Pages are hoisted by liveness and equal
/// ones are one.
#[test]
fn a_loop_over_many_globals_makes_each_page_once() {
    let asm =
        driver::assembly(include_str!("data/loop_with_many_global_pages.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find("_text_write:").expect("the function is there");
    let text = &asm[start..];
    let text = &text[..text[1..].find("\n_").map_or(text.len(), |end| end + 1)];
    let pages = text.lines().filter(|line| line.trim_start().starts_with("adrp")).count();
    assert!(pages <= 12, "{pages} adrp: {text}");
}
