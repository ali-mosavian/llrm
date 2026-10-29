//! IR to ca65 through the selector and `select.rs`, run on sim65. Every expected value is worked
//! out by hand.

mod common;

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8";

/// The functions of `ir` as ca65 text.
fn compile(
    ir: &str,
    functions: &[&str],
) -> String {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    functions
        .iter()
        .map(|name| {
            let blocks = select_function(&module, name, Cpu::Mos6502).expect("selects");
            function_text(&module, name, &blocks).expect("emits")
        })
        .collect()
}

fn set(
    register: &str,
    value: u16,
) -> String {
    format!("  lda #{}\n  sta {register}\n", value & 0xFF)
}

fn run(
    name: &str,
    ir: &str,
    function: &str,
    setup: &str,
    after: &str,
) -> i32 {
    common::run_with(name, &format!("{setup}  jsr {function}\n{after}"), &compile(ir, &[function]))
}

const ADD8: &str = "define i8 @add8(i8 %a, i8 %b) {\n  %r = add i8 %a, %b\n  ret i8 %r\n}";

/// 7 + 5 = 12; 200 + 100 = 300 = 0x12C, so 44.
#[test]
fn a_byte_add_runs() {
    for (a, b, want) in [(7, 5, 12), (200, 100, 44)] {
        let setup = set("rc2", a) + &set("rc3", b);
        assert_eq!(run(&format!("add8-{a}"), ADD8, "add8", &setup, ""), want);
    }
}

/// 300 + 5000 = 5300 = 0x14B4: the low byte is 0xB4 = 180, the high 0x14 = 20.
#[test]
fn a_word_add_runs() {
    let ir = "define i16 @add16(i16 %a, i16 %b) {\n  %r = add i16 %a, %b\n  ret i16 %r\n}";
    let setup = set("rc2", 300) + &set("rc3", 300 >> 8) + &set("rc4", 5000) + &set("rc5", 5000 >> 8);
    assert_eq!(run("add16-lo", ir, "add16", &setup, ""), 180);
    assert_eq!(run("add16-hi", ir, "add16", &setup, "  txa\n"), 20);
}

/// `a < b` unsigned: 3 < 9 gives 1, 9 < 3 gives 2, 5 < 5 gives 2.
#[test]
fn an_unsigned_branch_runs() {
    let ir = "define i8 @lt(i8 %a, i8 %b) {\nentry:\n  %c = icmp ult i8 %a, %b\n  br i1 %c, label %t, label %f\nt:\n  ret i8 1\nf:\n  ret i8 2\n}";
    for (a, b, want) in [(3, 9, 1), (9, 3, 2), (5, 5, 2)] {
        let setup = set("rc2", a) + &set("rc3", b);
        assert_eq!(run(&format!("lt-{a}-{b}"), ir, "lt", &setup, ""), want);
    }
}

/// `caller(10)` calls `g(i8)`, written by hand to return `rc2 + 1` = 11.
#[test]
fn a_call_passes_a_byte() {
    let ir = "declare i8 @g(i8)\ndefine i8 @caller(i8 %a) {\n  %r = call i8 @g(i8 %a)\n  ret i8 %r\n}";
    let g = ".export g\ng:\n  clc\n  lda rc2\n  adc #1\n  rts\n";
    let text = compile(ir, &["caller"]);
    assert_eq!(common::run_with("call", &format!("{}  jsr caller\n", set("rc2", 10)), &format!("{text}{g}")), 11);
}

/// `caller(10, 30)` calls `g(i8, i8)`, written by hand to return `rc2 + rc3` = 40.
/// Before the builder called the target's `lower_call` (plan row S2) it named argument registers
/// by root and size, so `rc3` became `rc2` and the run returned 60.
#[test]
fn a_call_passes_two_bytes() {
    let ir = "declare i8 @g(i8, i8)\ndefine i8 @caller(i8 %a, i8 %b) {\n  %r = call i8 @g(i8 %a, i8 %b)\n  ret i8 %r\n}";
    let g = ".export g\ng:\n  clc\n  lda rc2\n  adc rc3\n  rts\n";
    let setup = set("rc2", 10) + &set("rc3", 30);
    let text = compile(ir, &["caller"]);
    assert_eq!(common::run_with("call-two", &format!("{setup}  jsr caller\n"), &format!("{text}{g}")), 40);
}

/// `caller(10, 300)` calls `g(i8, i16)`, written by hand to return `rc2 + rc4` = 10 + 0x2C = 54.
/// Before the hook both bytes of the word went to `rc4`, `rc5` was never set, and the run returned 11.
#[test]
fn a_call_passes_a_byte_and_a_word() {
    let ir = "declare i8 @g(i8, i16)\ndefine i8 @caller(i8 %a, i16 %b) {\n  %r = call i8 @g(i8 %a, i16 %b)\n  ret i8 %r\n}";
    let g = ".export g\ng:\n  clc\n  lda rc2\n  adc rc4\n  rts\n";
    let setup = set("rc2", 10) + &set("rc4", 300) + &set("rc5", 300 >> 8);
    let text = compile(ir, &["caller"]);
    assert_eq!(common::run_with("call-word", &format!("{setup}  jsr caller\n"), &format!("{text}{g}")), 54);
}

/// Five bytes the memory tests point at; `rc2:rc3` gets their address.
const BUFFER: &str = ".segment \"DATA\"\nbuf: .byte $10, $20, $30, $40, $50\n.segment \"CODE\"\n";
const POINT: &str = "  lda #<buf\n  sta rc2\n  lda #>buf\n  sta rc3\n";

fn memory(
    name: &str,
    ir: &str,
    function: &str,
    setup: &str,
    after: &str,
) -> i32 {
    let text = compile(ir, &[function]);
    common::run_with(name, &format!("{POINT}{setup}  jsr {function}\n{after}"), &format!("{text}{BUFFER}"))
}

/// A byte through a pointer argument: `buf[0]` = 0x10 = 16.
#[test]
fn a_byte_load_runs() {
    let ir = "define i8 @ld(ptr %p) {\n  %v = load i8, ptr %p\n  ret i8 %v\n}";
    assert_eq!(memory("ld", ir, "ld", "", ""), 16);
}

/// `p + 3` is the `(zp),y` offset: `buf[3]` = 0x40 = 64.
#[test]
fn a_load_at_an_offset_runs() {
    let ir = "define i8 @ldo(ptr %p) {\n  %q = getelementptr i8, ptr %p, i16 3\n  %v = load i8, ptr %q\n  ret i8 %v\n}";
    assert_eq!(memory("ldo", ir, "ldo", "", ""), 64);
}

/// A word load is two bytes, little endian: 0x10, 0x20 is 0x2010.
#[test]
fn a_word_load_runs() {
    let ir = "define i16 @ld16(ptr %p) {\n  %v = load i16, ptr %p\n  ret i16 %v\n}";
    assert_eq!(memory("ld16-lo", ir, "ld16", "", ""), 0x10);
    assert_eq!(memory("ld16-hi", ir, "ld16", "", "  txa\n"), 0x20);
}

/// `st(buf, 0x5A)` leaves 0x5A in `buf[0]` and the rest alone.
#[test]
fn a_byte_store_runs() {
    let ir = "define void @st(ptr %p, i8 %v) {\n  store i8 %v, ptr %p\n  ret void\n}";
    let setup = set("rc4", 0x5A);
    assert_eq!(memory("st-0", ir, "st", &setup, "  lda buf\n"), 0x5A);
    assert_eq!(memory("st-1", ir, "st", &setup, "  lda buf+1\n"), 0x20);
}

/// `st16(buf, 0x1234)` stores the low byte first: `buf[0]` = 0x34, `buf[1]` = 0x12.
#[test]
fn a_word_store_runs() {
    let ir = "define void @st16(ptr %p, i16 %v) {\n  store i16 %v, ptr %p\n  ret void\n}";
    let setup = set("rc4", 0x1234) + &set("rc5", 0x12);
    assert_eq!(memory("st16-0", ir, "st16", &setup, "  lda buf\n"), 0x34);
    assert_eq!(memory("st16-1", ir, "st16", &setup, "  lda buf+1\n"), 0x12);
}

const GLOBAL: &str = "@g = global i8 7\n";
const G_DATA: &str = ".segment \"DATA\"\ng: .byte 7\n.segment \"CODE\"\n";

fn with_global(
    name: &str,
    body: &str,
    function: &str,
    after: &str,
    setup: &str,
) -> i32 {
    let text = compile(&format!("{GLOBAL}{body}"), &[function]);
    common::run_with(name, &format!("{setup}  jsr {function}\n{after}"), &format!("{text}{G_DATA}"))
}

/// The global starts as 7.
#[test]
fn a_global_load_runs() {
    let ir = "define i8 @rd() {\n  %v = load i8, ptr @g\n  ret i8 %v\n}";
    assert_eq!(with_global("rd", ir, "rd", "", ""), 7);
}

/// `wr(0x42)` leaves 0x42 = 66 in the global.
#[test]
fn a_global_store_runs() {
    let ir = "define void @wr(i8 %v) {\n  store i8 %v, ptr @g\n  ret void\n}";
    assert_eq!(with_global("wr", ir, "wr", "  lda g\n", &set("rc2", 0x42)), 0x42);
}

/// `addr()` returns the address of `g` in `a:x`; the caller compares both bytes and returns 1.
#[test]
fn a_global_address_runs() {
    let ir = "define ptr @addr() {\n  ret ptr @g\n}";
    let after = "  cmp #<g\n  bne bad\n  txa\n  cmp #>g\n  bne bad\n  lda #1\n  rts\nbad:\n  lda #0\n";
    assert_eq!(with_global("addr", ir, "addr", after, ""), 1);
}

/// `sum(n)` adds 1..n in a loop of two phis (a do-while, so n = 1 runs once): 1+2+3+4 = 10, 1+..+5 = 15.
#[test]
fn a_phi_loop_runs() {
    let ir = "define i8 @sum(i8 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i8 [0, %entry], [%i2, %loop]\n  %s = phi i8 [0, %entry], [%s2, %loop]\n  %i2 = add i8 %i, 1\n  %s2 = add i8 %s, %i2\n  %c = icmp ult i8 %i2, %n\n  br i1 %c, label %loop, label %done\ndone:\n  ret i8 %s2\n}";
    for (n, want) in [(1, 1), (4, 10), (5, 15)] {
        assert_eq!(run(&format!("sum-{n}"), ir, "sum", &set("rc2", n), ""), want, "sum({n})");
    }
}

const ADD32: &str = "define i32 @add32(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  ret i32 %r\n}";
const PASS32: &str = "define i32 @pass32(i32 %a, i32 %b) {\n  %r = call i32 @add32(i32 %a, i32 %b)\n  ret i32 %r\n}";

/// `a` = 0x0001F234 in rs1:rs2, `b` = 0x00011100 with its low word in rs3 and its high word,
/// 0x0001, at the top of the soft stack (`rs0` = $8000). The low words carry (0xF234 + 0x1100 =
/// 0x10334), so the sum is 0x00030334; without the carry it would be 0x00020334.
const LONG_ARGS: &str = "  lda #0\n  sta rc0\n  lda #$80\n  sta rc1\n  lda #1\n  sta $8000\n  lda #0\n  sta $8001\n\
  lda #$34\n  sta rc2\n  lda #$F2\n  sta rc3\n  lda #1\n  sta rc4\n  lda #0\n  sta rc5\n\
  lda #$00\n  sta rc6\n  lda #$11\n  sta rc7\n";

/// The long comes back as `a`, `x`, `rc2`, `rc3`: 0x34, 0x03, 0x03, 0x00. The carry out of the low
/// word crosses the stack argument's address arithmetic, which once clobbered it (the high word was
/// one short); before the result parts shared one row the second word was read from `a:x` again.
#[test]
fn a_long_add_with_a_stack_argument_runs() {
    for (after, want) in [("", 0x34), ("  txa\n", 0x03), ("  lda rc2\n", 0x03), ("  lda rc3\n", 0x00)] {
        assert_eq!(run(&format!("add32-{want}-{}", after.len()), ADD32, "add32", LONG_ARGS, after), want, "{after:?}");
    }
}

/// `pass32` re-passes both longs to `add32`: it reserves two bytes below `rs0`, copies its own stack
/// argument there and the callee reads it at `rs0 + 0`. The same sum.
#[test]
fn a_call_with_a_stack_argument_runs() {
    let text = compile(&format!("{ADD32}\n{PASS32}"), &["add32", "pass32"]);
    for (after, want) in [("", 0x34), ("  txa\n", 0x03), ("  lda rc2\n", 0x03), ("  lda rc3\n", 0x00)] {
        let main = format!("{LONG_ARGS}  jsr pass32\n{after}");
        assert_eq!(common::run_with(&format!("pass32-{want}-{}", after.len()), &main, &text), want, "{after:?}");
    }
}

/// Cases 1 and 2 and a default: 1 gives 10, 2 gives 20, 0 and 3 give 30.
#[test]
fn a_switch_with_two_cases_runs() {
    let ir = "define i8 @sw(i8 %x) {\nentry:\n  switch i8 %x, label %d [i8 1, label %a\n i8 2, label %b]\na:\n  ret i8 10\nb:\n  ret i8 20\nd:\n  ret i8 30\n}";
    for (x, want) in [(1, 10), (2, 20), (0, 30), (3, 30)] {
        assert_eq!(run(&format!("sw-{x}"), ir, "sw", &set("rc2", x), ""), want, "sw({x})");
    }
}

/// Five scattered cases make the builder's binary search over several blocks: 1, 3, 7, 9 and 200
/// give 1, 3, 7, 9 and 200 doubled mod 256 (144); 2 and 255 are the default 99.
#[test]
fn a_switch_with_five_cases_runs() {
    let ir = "define i8 @sw5(i8 %x) {\nentry:\n  switch i8 %x, label %d [i8 1, label %a\n i8 3, label %b\n i8 7, label %c\n i8 9, label %e\n i8 200, label %f]\na:\n  ret i8 1\nb:\n  ret i8 3\nc:\n  ret i8 7\ne:\n  ret i8 9\nf:\n  ret i8 144\nd:\n  ret i8 99\n}";
    for (x, want) in [(1, 1), (3, 3), (7, 7), (9, 9), (200, 144), (2, 99), (255, 99)] {
        assert_eq!(run(&format!("sw5-{x}"), ir, "sw5", &set("rc2", x), ""), want, "sw5({x})");
    }
}

/// A long returned in `a`, `x`, `rc2`, `rc3`: the copies into `rc2` and `rc3` go through `a`, which
/// once overwrote the result byte in `a` (the run returned 0 for 0x10).
#[test]
fn a_long_return_keeps_the_byte_in_a() {
    let ir = "define i32 @id32(i32 %a) {\n  ret i32 %a\n}";
    let setup = set("rc2", 0x9810) + &set("rc3", 0x98) + &set("rc4", 0x02) + &set("rc5", 0);
    for (after, want) in [("", 0x10), ("  txa\n", 0x98), ("  lda rc2\n", 0x02), ("  lda rc3\n", 0x00)] {
        assert_eq!(run(&format!("id32-{want}"), ir, "id32", &setup, after), want, "{after:?}");
    }
}
