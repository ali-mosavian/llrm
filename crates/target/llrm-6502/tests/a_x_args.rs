//! A byte argument travels in `a`, then `x`. `select.rs` held every copy into
//! `a`, `x` and `y` back to the `ret`, so the argument of `h(7)` was loaded
//! after the `jsr` (`jsr h; lda #7`) and a tail call dropped it (2026-11-08).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn body(
    ir: &str,
    name: &str,
) -> Vec<String> {
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    text[from..].split(".segment").next().unwrap().lines().map(|line| line.trim().to_owned()).collect()
}

#[test]
fn a_byte_argument_is_in_a_before_the_call() {
    let lines = body("declare void @h(i8)\ndefine void @k_one() {\n  call void @h(i8 7)\n  ret void\n}\n", "k_one");
    let call = lines.iter().position(|l| l.starts_with("jsr h") || l.starts_with("jmp h")).expect("a call");
    assert!(lines[..call].contains(&"lda #7".to_owned()), "a is not loaded before the call: {lines:?}");
}

#[test]
fn two_byte_arguments_are_in_a_and_x_before_the_call() {
    let lines =
        body("declare void @h(i8, i8)\ndefine void @k_two() {\n  call void @h(i8 7, i8 9)\n  ret void\n}\n", "k_two");
    let call = lines.iter().position(|l| l.starts_with("jsr h") || l.starts_with("jmp h")).expect("a call");
    assert!(lines[..call].contains(&"lda #7".to_owned()) && lines[..call].contains(&"ldx #9".to_owned()), "{lines:?}");
}
