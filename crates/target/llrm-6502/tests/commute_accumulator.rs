//! A commutative byte op reads the argument that arrives in `a` from `a`.
//! `add i8 %a, %b` stored `a` to a cell and moved `x` into `a` (`sta rc2; clc;
//! txa; adc rc2`, 7 bytes) whichever way round the IR named its operands
//! (2026-11-08); one store of `x` does (`stx rc2; clc; adc rc2`, 6 bytes).

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
    text[from..].split(".segment").next().unwrap().lines().skip(1).map(|line| line.trim().to_owned()).collect()
}

fn byte_op(
    op: &str,
    flipped: bool,
) -> Vec<String> {
    let (l, r) = if flipped { ("%b", "%a") } else { ("%a", "%b") };
    body(&format!("define i8 @f(i8 %a, i8 %b) {{\n  %r = {op} i8 {l}, {r}\n  ret i8 %r\n}}\n"), "f")
}

#[test]
fn the_argument_in_a_stays_in_a_whichever_way_round_the_ir_reads() {
    for op in ["add", "and", "or", "xor"] {
        let plain = byte_op(op, false);
        assert_eq!(plain, byte_op(op, true), "{op}: the operand order changed the code");
        assert!(plain.contains(&"stx rc2".to_owned()), "{op}: x is not stored to the cell: {plain:?}");
        assert!(!plain.iter().any(|l| l == "txa" || l == "sta rc2"), "{op}: a is moved aside: {plain:?}");
    }
}

#[test]
fn a_subtraction_keeps_its_order() {
    let lines = body("define i8 @f(i8 %a, i8 %b) {\n  %r = sub i8 %a, %b\n  ret i8 %r\n}\n", "f");
    assert!(lines.contains(&"stx rc2".to_owned()) && lines.iter().any(|l| l == "sbc rc2"), "{lines:?}");
}

/// A word sum over bytes loaded in a loop stored the loaded byte to a cell to
/// read it back as the second source (`lda (rc2),y; sta rc6; clc; lda rc4; adc
/// rc6`, 2 bytes and 3 clocks a trip over `clc; adc rc4`): the byte that
/// arrives in `a` from a load goes first, not only an argument (2026-11-08).
#[test]
fn a_byte_loaded_into_a_is_added_from_a_not_stored_aside() {
    let lines = body(
        "define i16 @f(ptr %p, i8 %n) {\nentry:\n  %z = icmp eq i8 %n, 0\n  br i1 %z, label %x, label %l\n\
         l:\n  %i = phi i8 [0, %entry], [%i1, %l]\n  %s = phi i16 [0, %entry], [%s1, %l]\n  %iz = zext i8 %i to i16\n  \
         %q = getelementptr i8, ptr %p, i16 %iz\n  %v = load i8, ptr %q\n  %w = zext i8 %v to i16\n  \
         %s1 = add i16 %s, %w\n  %i1 = add nuw i8 %i, 1\n  %c = icmp ne i8 %i1, %n\n  br i1 %c, label %l, label %x\n\
         x:\n  %r = phi i16 [0, %entry], [%s1, %l]\n  ret i16 %r\n}\n",
        "f",
    );
    let load = lines.iter().position(|l| l == "lda (rc2),y").expect("the load");
    assert_eq!(&lines[load + 1..load + 3], ["clc", "adc rc4"], "{lines:?}");
}
