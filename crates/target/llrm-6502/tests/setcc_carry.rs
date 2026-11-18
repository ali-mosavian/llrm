//! A compare of two words is one chain: the borrow of the low words into the
//! high words' subtract.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

/// One compare of two `i32`, `k_<order>`, read through pointers.
fn compare(order: &str) -> String {
    format!(
        "define i16 @k_{order}(ptr %a, ptr %b) {{
  %x = load i32, ptr %a
  %y = load i32, ptr %b
  %c = icmp {order} i32 %x, %y
  %r = zext i1 %c to i16
  ret i16 %r
}}
"
    )
}

fn bytes_of(
    order: &str,
    limit: u32,
) {
    let name = format!("k_{order}");
    let ir = format!("target datalayout = \"{}\"\n{}", datalayout(), compare(order));
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    let body: String = text[from..].split(".segment").next().unwrap().to_owned();
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    assert!(bytes <= limit, "{name}: {bytes} B\n{body}");
}

/// The compare was a select of the compares of its two halves: three chains
/// and a 0/1 byte each, over 120 B for what one `sec; sbc` chain of four limbs
/// is.
#[test]
fn an_unsigned_less_than_of_two_words_is_one_borrow_chain() {
    bytes_of("ult", 80);
}

#[test]
fn an_unsigned_less_or_equal_of_two_words_is_one_borrow_chain() {
    bytes_of("ule", 90);
}

#[test]
fn a_signed_less_than_of_two_words_is_one_borrow_chain() {
    bytes_of("slt", 90);
}

#[test]
fn a_signed_less_or_equal_of_two_words_is_one_borrow_chain() {
    bytes_of("sle", 106);
}

/// `k_branch` and `k_select`: the order of two `i32` read by a branch and by a
/// select.
fn consumer(kind: &str) -> String {
    match kind {
        "branch" => "declare void @one()
declare void @two()
define void @k_branch(ptr %a, ptr %b) {
  %x = load i32, ptr %a
  %y = load i32, ptr %b
  %c = icmp uge i32 %x, %y
  br i1 %c, label %t, label %f
t:
  call void @one()
  ret void
f:
  call void @two()
  ret void
}
"
        .to_owned(),
        _ => "define i16 @k_select(ptr %a, ptr %b, i16 %p, i16 %q) {
  %x = load i32, ptr %a
  %y = load i32, ptr %b
  %c = icmp uge i32 %x, %y
  %r = select i1 %c, i16 %p, i16 %q
  ret i16 %r
}
"
        .to_owned(),
    }
}

fn consumer_bytes(
    kind: &str,
    limit: u32,
) {
    let name = format!("k_{kind}");
    let ir = format!("target datalayout = \"{}\"\n{}", datalayout(), consumer(kind));
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    let body: String = text[from..].split(".segment").next().unwrap().to_owned();
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    assert!(bytes <= limit, "{name}: {bytes} B\n{body}");
}

/// A branch on `x >= y` made the borrow a byte (`lda #0; sbc #0; and #1`),
/// selected it to 0 or 1 and tested it again: 77 B for the branch and 94 B for
/// the select, where the flags of the last `sbc` are 70 B and 85 B. `int32.c`'s
/// `udivmod` took it per call, `div_constants_short -Os` ran 25% more cycles.
#[test]
fn a_branch_on_the_order_of_two_words_reads_the_flags_of_the_chain() {
    // 72 B before the first pointer arrived in A:X (stored to cells)
    consumer_bytes("branch", 73);
}

#[test]
fn a_select_on_the_order_of_two_words_reads_the_flags_of_the_chain() {
    consumer_bytes("select", 88);
}

/// `k_valuege`: `x >= y` of two `i32` as a value. "Not below" was a select of
/// the borrow byte: `bne; lda #1; jmp; lda #0; and #1`, a branch for a bit.
fn value_ge() -> String {
    "define i16 @k_valuege(ptr %a, ptr %b) {
  %x = load i32, ptr %a
  %y = load i32, ptr %b
  %c = icmp uge i32 %x, %y
  %r = zext i1 %c to i16
  ret i16 %r
}
"
    .to_owned()
}

#[test]
fn an_unsigned_greater_or_equal_of_two_words_as_a_value_is_straight_line() {
    let ir = format!("target datalayout = \"{}\"\n{}", datalayout(), value_ge());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("k_valuege:").unwrap_or_else(|| panic!("no k_valuege in\n{text}"));
    let body: String = text[from..].split(".segment").next().unwrap().to_owned();
    assert!(!body.contains("jmp"), "a bit made through a branch:\n{body}");
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    // ratchet: 82 B while row 6502-SELECT-NZ is open (the select of the bit
    // branches on Z, so its head move cannot follow the compare: `ldy #0`
    // for `txa`); exit <= 81 B by 2026-11-20
    assert!(bytes <= 82, "6502-SELECT-NZ: k_valuege: {bytes} B, want <= 81 B by 2026-11-20\n{body}");
}

/// `k_sign`: a branch on `x >= 0` of an `i32` (`x <= -1` is the same sign).
fn sign_ir() -> String {
    "declare void @one()
declare void @two()
define void @k_sign(ptr %a) {
  %x = load i32, ptr %a
  %c = icmp sge i32 %x, 0
  br i1 %c, label %t, label %f
t:
  call void @one()
  ret void
f:
  call void @two()
  ret void
}
"
    .to_owned()
}

/// `x >= 0` went through the four-limb chain with the sign bits flipped (`lda
/// #255; cmp; sbc ...; sbc rc6`), where the sign of the high word is `bpl`:
/// `__fixunssfsi` and `__fixunsdfdi` grew by 20 B and ran slower.
#[test]
fn a_long_at_or_above_zero_is_the_sign_of_its_high_word() {
    let ir = format!("target datalayout = \"{}\"\n{}", datalayout(), sign_ir());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("k_sign:").unwrap_or_else(|| panic!("no k_sign in\n{text}"));
    let body: String = text[from..].split(".segment").next().unwrap().to_owned();
    assert!(!body.contains("sbc"), "a chain for a sign:\n{body}");
}

/// The sign of a long, run: the branch tests the sign of the high word, so a
/// value whose low words are all ones or all zeros must not change it.
#[test]
fn the_sign_of_a_long_decides_the_branch_when_run() {
    for (value, want) in [(-1i32, 2), (0, 1), (1, 1), (0x0001_0000, 1), (-0x0001_0000, 2), (i32::MIN, 2), (i32::MAX, 1)]
    {
        let ir = format!(
            "target datalayout = \"{}\"\n@g = global i32 {value}\n\
             define i8 @t() {{\nentry:\n  %x = load volatile i32, ptr @g\n  %c = icmp sge i32 %x, 0\n  \
             br i1 %c, label %p, label %n\np:\n  ret i8 1\nn:\n  ret i8 2\n}}\n",
            datalayout()
        );
        let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
        let functions: String = text.lines().filter(|l| !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
        let got = common::run_with(&format!("sign-{value}"), "  jsr t\n", &functions);
        assert_eq!(got, want, "x = {value}:\n{text}");
    }
}
