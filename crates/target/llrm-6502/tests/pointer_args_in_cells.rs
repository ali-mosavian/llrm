//! A pointer argument arrives in a pair of cells, an integer word in A:X
//! (llvm-mos's CC_MOS: `CCIfPtr` takes the pointer pairs, `i16` takes A:X
//! first): the pointer `(zp),y` walks is already in zero page.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn text(body: &str) -> String {
    let ir = format!("target datalayout = \"{}\"\n{body}", datalayout());
    assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"))
}

fn function<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    text[from..].split(".segment").next().unwrap()
}

/// `k_strlower`-style walks took +4 B for `sta rc2; stx rc3` at entry (18
/// kernel cells above the llvm-mos bar since the first word travels in A:X).
#[test]
fn a_pointer_walked_by_y_is_read_from_its_cells_at_entry() {
    let asm = text(
        "define i8 @first(ptr %p) {
  %v = load i8, ptr %p
  ret i8 %v
}
",
    );
    let first = function(&asm, "first");
    assert!(first.contains("lda (rc2),y"), "{first}");
    assert!(!first.contains("stx") && !first.contains("sta rc"), "{first}");
}

/// A pointer and a word: the pointer takes `rc2:rc3` and the word keeps A:X,
/// whichever comes first.
#[test]
fn an_integer_word_keeps_a_and_x_beside_a_pointer() {
    let asm = text(
        "declare i8 @at(ptr, i16)
declare i8 @at_swapped(i16, ptr)
@g = global [4 x i8] zeroinitializer
define i8 @first() {
  %a = call i8 @at(ptr @g, i16 5)
  %r = add i8 %a, 1
  ret i8 %r
}
define i8 @second() {
  %a = call i8 @at_swapped(i16 6, ptr @g)
  %r = add i8 %a, 1
  ret i8 %r
}
",
    );
    for caller in ["first", "second"] {
        let body = function(&asm, caller);
        assert!(body.contains("sta rc2") && body.contains("sta rc3") && body.contains("ldx #0"), "{body}");
    }
}

/// The caller of a pointer-taking routine sets its cells, not A:X.
#[test]
fn a_call_passes_a_pointer_in_its_cells() {
    let asm = text(
        "declare i8 @f(ptr)
@g = global [4 x i8] zeroinitializer
define i8 @caller() {
  %r = call i8 @f(ptr @g)
  ret i8 %r
}
",
    );
    let caller = function(&asm, "caller");
    assert!(caller.contains("sta rc2") && caller.contains("rc3"), "{caller}");
}
