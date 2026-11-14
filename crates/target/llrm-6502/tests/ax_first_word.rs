//! The first integer word argument is in A:X (llvm-mos's CC_MOS), for the program's
//! own functions and for the routines the compiler names (mem*, compiler-rt); a
//! pointer is in the pointer pairs (`pointer_args_in_cells.rs`).

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

const ADD: &str = "define i16 @add16(i16 %a, i16 %b) noinline {
  %r = add i16 %a, %b
  ret i16 %r
}
define i16 @caller(i16 %x) {
  %r = call i16 @add16(i16 %x, i16 7)
  ret i16 %r
}
";

/// `add16` was 13 B (llvm-mos and the hand code 10): both words came from
/// cells, `lda rc4; adc rc2`. The first word is now already in A:X; only the
/// second is read from `rc2`/`rc3`, and no `rc4`/`rc5` cell is used.
#[test]
fn the_first_word_arrives_in_a_and_x_and_the_second_in_the_first_pair() {
    let asm = text(ADD);
    let add = function(&asm, "add16");
    assert!(add.contains("adc rc2") && add.contains("rc3"), "{add}");
    assert!(!add.contains("rc5"), "{add}");
}

/// The caller's own first word is already in A:X: it is handed on (a tail
/// call) and the 7 goes in `rc2:rc3`.
#[test]
fn a_call_hands_the_first_word_over_in_a_and_x() {
    let asm = text(ADD);
    let caller = function(&asm, "caller");
    assert!(caller.contains("add16"), "{caller}");
    assert!(caller.contains("sty rc2") && !caller.contains("rc4"), "{caller}");
}

/// `memcpy` takes dst in `rc2:rc3`, src in `rc4:rc5` and n in A:X: a copy that
/// receives them in those registers jumps to it with nothing moved. A second
/// caller shares the library's bytes: alone, the loop is the smaller image.
#[test]
fn a_libcall_takes_its_first_word_in_a_and_x() {
    let asm = text(
        "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
define void @copy(ptr %d, ptr %s, i16 %n) {
  call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 %n, i1 false)
  ret void
}
define void @copy_again(ptr %d, ptr %s, i16 %n) {
  call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 %n, i1 false)
  ret void
}
",
    );
    let copy = function(&asm, "copy");
    assert_eq!(copy.trim(), "copy:\n  jmp memcpy", "{copy}");
}

/// `g((int)&local)` took the address as two byte copies with a `UNPACK` between
/// them; the call's uses ended the run of copies at the `UNPACK` and listed `a`
/// only, so the copy into `x` was dead and `g` got a stale high byte (the
/// struct-by-pointer run returned 7, not 10).
#[test]
fn the_high_byte_of_a_passed_address_is_set_in_x() {
    let asm = text(
        "declare void @g(i16)
define void @func() {
  %s = alloca [2 x i8]
  %a = ptrtoint ptr %s to i16
  call void @g(i16 %a)
  ret void
}
",
    );
    let func = function(&asm, "func");
    let before_call = &func[..func.find("jsr g").unwrap_or_else(|| panic!("{func}"))];
    assert!(before_call.contains("tax") || before_call.contains("ldx"), "x is never set before the call:\n{func}");
}

/// An indirect call enters its callee with an `rts`; the dead-write pass took
/// it for the function's own return, which has no `x` result, and dropped the
/// `ldx rc3` that sets the callee's second argument (`both(&plus, 5, 7)`
/// returned 14, not 19).
#[test]
fn an_argument_in_x_is_loaded_before_an_indirect_call() {
    let asm = text(
        "define i8 @t2(i8 %p, i8 %q, i8 %x, i8 %y, ptr %f) {
  %a = call i8 %f(i8 %x, i8 %y)
  ret i8 %a
}
",
    );
    let t2 = function(&asm, "t2");
    assert!(t2.contains("ldx rc3"), "x is never loaded for the callee:\n{t2}");
}

/// A word arriving in A:X and kept across a call is copied into callee-saved cells (`rc8:rc9`). The
/// copy was made before the prologue pushed those cells, so the push saved the argument and the caller's
/// `rc8` came back changed (`frameless` -O2: 1011, not 499).
#[test]
fn an_argument_copied_into_a_callee_saved_cell_is_saved_first() {
    let asm = text(
        "declare i16 @ext(i16)
define i16 @keep2(i16 %x) {
  %a = call i16 @ext(i16 %x)
  %b = add i16 %a, %x
  ret i16 %b
}
",
    );
    let functions: String =
        asm.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
    let ext = "ext:\n  clc\n  adc #1\n  bcc ext_done\n  inx\next_done:\n  rts\n";
    // keep2(10) = 11 + 10; the caller's rc8 and rc9 survive
    let main = "  lda #$A1\n  sta rc8\n  lda #$B2\n  sta rc9\n  ldx #0\n  lda #10\n  jsr keep2\n  pha\n  lda rc8\n  cmp #$A1\n  bne bad\n  lda rc9\n  cmp #$B2\n  bne bad\n  pla\n  rts\nbad:\n  lda #99\n  rts\n";
    assert_eq!(common::run_with("ax-keep2", main, &format!("{functions}{ext}")), 21, "{asm}");
}
