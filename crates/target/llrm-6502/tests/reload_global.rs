//! A global read by a shift and by an add is read again by the add (llvm-mos
//! `shouldFoldMemAccess`): `adc state`, not a zero-page copy of every byte
//! first. licm_graph_callee_footprint's `next()` (`state * 3 + 1`) copied all
//! four bytes: 305,273 clocks against 290,273, and 7 bytes more.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn function(body: &str) -> String {
    let ir = format!("target datalayout = \"{}\"\n@g = global i32 5\ndefine i32 @f() {{\n{body}\n}}\n", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let start = text.find("\n__r_f:").expect("f is emitted");
    let end = text[start..].find("\n.segment").map_or(text.len(), |at| start + at);
    text[start..end].to_owned()
}

#[test]
fn a_global_read_by_a_shift_and_an_add_is_the_adds_memory_operand() {
    let text = function(
        "  %a = load i32, ptr @g\n  %m = mul i32 %a, 3\n  %n = add i32 %m, 1\n  store i32 %n, ptr @g\n  ret i32 %n",
    );
    let reads = text.lines().filter(|line| line.trim_start().starts_with("adc g")).count();
    assert_eq!(reads, 4, "{text}");
}

/// The store of each byte of `g` lands before the add reads it, or the add
/// reads the new value: no `adc` of a byte follows the `sta` of that byte.
#[test]
fn a_store_between_the_load_and_the_add_keeps_the_register() {
    let text = function(
        "  %a = load i32, ptr @g\n  store i32 0, ptr @g\n  %m = shl i32 %a, 1\n  %n = add i32 %m, %a\n  ret i32 %n",
    );
    for byte in ["g", "g+1", "g+2", "g+2+1"] {
        let stored = text.find(&format!("sta {byte}\n")).expect("the store is emitted");
        assert!(!text[stored..].contains(&format!("adc {byte}\n")), "{byte}: {text}");
    }
}

#[test]
fn a_volatile_read_is_not_made_twice() {
    let text = function("  %a = load volatile i32, ptr @g\n  %m = shl i32 %a, 1\n  %n = add i32 %m, %a\n  ret i32 %n");
    assert_eq!(text.matches("lda g").count(), 4, "{text}");
}
