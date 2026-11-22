//! `x == 0` and `x != 0` on a long test all four bytes as `lda; ora; ora; ora`
//! (llvm-mos `G_CMPZ`). The halves were or'd into two words, moved through `x`
//! and `y`, and the word tested: `ora; tax; ora; tay; txa; bne; tya; bne`, 26
//! clocks for 15 (licm_graph_callee_footprint, 333,779 clocks against llvm-mos'
//! 297,188).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn function(body: &str) -> String {
    let ir = format!(
        "target datalayout = \"{}\"\ndeclare void @report(i32)\ndefine void @f(i32 %n) {{\n{body}\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    let start = text.find("\n__r_f:").expect("f is emitted");
    let end = text[start..].find("\n.segment").map_or(text.len(), |at| start + at);
    text[start..end].to_owned()
}

fn mnemonics(text: &str) -> Vec<&str> {
    text.lines().filter_map(|line| line.split_whitespace().next()).filter(|word| !word.ends_with(':')).collect()
}

#[test]
fn a_long_counter_exit_tests_all_four_bytes_in_one_run() {
    let text = function(
        "br label %loop\nloop:\n  %i = phi i32 [ %n, %0 ], [ %next, %loop ]\n  call void @report(i32 %i)\n  %next = add i32 %i, -1\n  %done = icmp eq i32 %next, 0\n  br i1 %done, label %exit, label %loop\nexit:\n  ret void",
    );
    let words = mnemonics(&text);
    assert!(words.windows(4).any(|w| w == ["lda", "ora", "ora", "ora"]), "{text}");
    assert!(!words.contains(&"tax") || !words.contains(&"tay"), "{text}");
}

/// licm_graph_callee_footprint's `run` (a long sum and a 1500-trip long counter
/// around a call): the sum's carry out is read by nothing, yet `lda #0; rol a`
/// made it a byte before the counter's step (which writes `a` first), 4 clocks
/// and 3 bytes a trip.
#[test]
fn the_unread_carry_of_a_long_sum_is_not_saved_before_the_counter_step() {
    let text = assembly(include_str!("data/long_counter_sum.ll"), &Request::new(Level::O2)).unwrap();
    assert!(!text.contains("rol a"), "{text}");
}

/// An equality of two longs used as a value, not a branch: the custom lowering
/// returned a byte where the type legalizer expected the bit it replaced, and
/// `zext i1` of it was `CannotSelect("ZeroExtend")` (8 tests of `call_glue`,
/// `divrem_pair` and `outline`).
#[test]
fn an_equality_of_longs_is_a_value_too() {
    let ir = format!(
        "target datalayout = \"{}\"\ndefine i8 @f(i32 %a, i32 %b) {{\n  %c = icmp ne i32 %a, %b\n  %d = icmp ne i32 %a, 7\n  %e = or i1 %c, %d\n  %r = zext i1 %e to i8\n  ret i8 %r\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(text.contains("rts"), "{text}");
}
