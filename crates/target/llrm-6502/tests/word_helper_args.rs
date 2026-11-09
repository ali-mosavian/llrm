//! The word multiply, divide and remainder routines take the first word in
//! A:X and the second in rc2:rc3, where a function's own two word arguments
//! already are. They took them in rc2:rc3 and rc4:rc5, and `a / b` moved both
//! first: `ldy rc2; sty rc4; ldy rc3; sty rc5; sta rc2; stx rc3; jmp` (21 B
//! against 3).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn body(op: &str) -> Vec<String> {
    let ir = format!(
        "target datalayout = \"{}\"\ndefine i16 @f(i16 %a, i16 %b) {{\n  %d = {op} i16 %a, %b\n  ret i16 %d\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("f:").unwrap_or_else(|| panic!("no f in\n{text}"));
    text[from..].split(".segment").next().unwrap().lines().map(|line| line.trim().to_owned()).collect()
}

/// A product is left out: two words of unknown width tie, and the DAG's operand
/// order decides which is the multiplier (`mul_operand_order.rs`).
#[test]
fn a_word_divide_of_the_arguments_moves_nothing() {
    for (op, routine) in
        [("sdiv", "__divhi3"), ("udiv", "__udivhi3"), ("srem", "__modhi3"), ("urem", "__umodhi3")]
    {
        let lines = body(op);
        let moves: Vec<_> =
            lines.iter().filter(|l| l.starts_with("st") || l.starts_with("ld") || l.starts_with("t")).collect();
        assert!(lines.iter().any(|l| l.contains(routine)), "{op}: no {routine} in {lines:?}");
        assert!(moves.is_empty(), "{op}: the arguments were moved: {lines:?}");
    }
}
