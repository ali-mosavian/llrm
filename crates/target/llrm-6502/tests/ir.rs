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
            let selection = select_function(&module, name, Cpu::Mos6502).expect("selects");
            function_text(&module, name, &selection).expect("emits")
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

/// The soft stack pointer `rs0` set to $8000 before the call.
const STACK: &str = "  lda #0\n  sta rc0\n  lda #$80\n  sta rc1\n";

/// A one-byte local: stored and loaded back through its frame address.
#[test]
fn a_local_byte_runs() {
    let ir = "define i8 @loc(i8 %x) {\n  %p = alloca i8\n  store i8 %x, ptr %p\n  %v = load i8, ptr %p\n  ret i8 %v\n}";
    let setup = STACK.to_owned() + &set("rc2", 77);
    assert_eq!(run("loc", ir, "loc", &setup, ""), 77);
}

/// A four-byte local array filled through offsets 0..3 and summed: 10 + 20 + 30 + 40 = 100.
#[test]
fn a_local_array_runs() {
    let mut ir = String::from("define i8 @arr() {\n  %a = alloca [4 x i8]\n");
    for (i, v) in [10, 20, 30, 40].iter().enumerate() {
        ir += &format!("  %p{i} = getelementptr i8, ptr %a, i16 {i}\n  store i8 {v}, ptr %p{i}\n");
    }
    for i in 0..4 {
        ir += &format!("  %v{i} = load i8, ptr %p{i}\n");
    }
    ir += "  %s1 = add i8 %v0, %v1\n  %s2 = add i8 %s1, %v2\n  %s3 = add i8 %s2, %v3\n  ret i8 %s3\n}";
    assert_eq!(run("arr", &ir, "arr", STACK, ""), 100);
}

/// `outer(5)` keeps 5 in a local across the call to `inner(6)`, which has a local of its own and
/// returns 6 + 1: 5 + 7 = 12. With overlapping frames `inner` overwrites `outer`'s byte and the sum
/// is 13.
#[test]
fn nested_frames_do_not_overlap() {
    let ir = "define i8 @inner(i8 %x) {\n  %p = alloca i8\n  store i8 %x, ptr %p\n  %v = load i8, ptr %p\n  %r = add i8 %v, 1\n  ret i8 %r\n}\n\
define i8 @outer(i8 %x) {\n  %p = alloca i8\n  store i8 %x, ptr %p\n  %y = add i8 %x, 1\n  %c = call i8 @inner(i8 %y)\n  %v = load i8, ptr %p\n  %r = add i8 %v, %c\n  ret i8 %r\n}";
    let text = compile(ir, &["inner", "outer"]);
    let main = format!("{STACK}{}  jsr outer\n", set("rc2", 5));
    assert_eq!(common::run_with("nested", &main, &text), 12);
}

/// `a op b` on bytes or words through `f(a, b)`; the result is `a` (and `x` for a word).
fn binary(
    ty: &str,
    op: &str,
    a: u16,
    b: u16,
) -> u32 {
    let ir = format!("define {ty} @func({ty} %a, {ty} %b) {{\n  %r = {op} {ty} %a, %b\n  ret {ty} %r\n}}");
    let word = ty == "i16";
    let setup = if word {
        set("rc2", a) + &set("rc3", a >> 8) + &set("rc4", b) + &set("rc5", b >> 8)
    } else {
        set("rc2", a) + &set("rc3", b)
    };
    let key = format!("{ty}-{op}-{a}-{b}");
    let low = run(&format!("{key}-lo"), &ir, "func", &setup, "") as u32;
    if word { (run(&format!("{key}-hi"), &ir, "func", &setup, "  txa\n") as u32) << 8 | low } else { low }
}

/// Subtraction and the runtime operations on bytes, worked out by hand: 50 - 8 = 42, 5 - 10 = 251;
/// 7 * 6 = 42, 200 * 2 = 400 = 0x190; 200 / 7 = 28 r 4; -7 / 2 = -3 r -1 (249, 253, 255);
/// 3 << 4 = 48, 0xF0 >> 4 = 15 logical and 255 arithmetic.
#[test]
fn byte_operations_run() {
    for (op, a, b, want) in [
        ("sub", 50, 8, 42),
        ("sub", 5, 10, 251),
        ("mul", 7, 6, 42),
        ("mul", 200, 2, 0x90),
        ("udiv", 200, 7, 28),
        ("urem", 200, 7, 4),
        ("sdiv", 249, 2, 253),
        ("srem", 249, 2, 255),
        ("shl", 3, 4, 48),
        ("lshr", 0xF0, 4, 15),
        ("ashr", 0xF0, 4, 255),
    ] {
        assert_eq!(binary("i8", op, a, b), want, "i8 {op} {a} {b}");
    }
}

/// The same on words: 5000 - 300 = 4700 = 0x125C; 300 * 5 = 1500; 50000 / 7 = 7142 = 0x1BE6;
/// -1000 % 7 = -6 = 0xFFFA; 0x0123 << 4 = 0x1230; 0x8100 >> 7 = 0x0102 logical, 0xFF02 arithmetic.
#[test]
fn word_operations_run() {
    for (op, a, b, want) in [
        ("sub", 5000, 300, 0x125C),
        ("mul", 300, 5, 1500),
        ("udiv", 50000, 7, 0x1BE6),
        ("srem", 64536, 7, 0xFFFA),
        ("shl", 0x0123, 4, 0x1230),
        ("lshr", 0x8100, 7, 0x0102),
        ("ashr", 0x8100, 7, 0xFF02),
    ] {
        assert_eq!(binary("i16", op, a, b), want, "i16 {op} {a} {b}");
    }
}

/// A comparison as a value (`zext i1`): 0 or 1, signed and unsigned, byte and word.
#[test]
fn comparisons_as_values_run() {
    for (ty, cc, a, b, want) in [
        ("i8", "eq", 5, 5, 1),
        ("i8", "eq", 5, 6, 0),
        ("i16", "ne", 300, 300, 0),
        ("i16", "ne", 300, 301, 1),
        ("i8", "slt", 253, 2, 1),
        ("i8", "slt", 2, 253, 0),
        ("i16", "sge", 64536, 5, 0),
        ("i16", "sge", 5, 64536, 1),
        ("i16", "sge", 7, 7, 1),
        ("i8", "ugt", 200, 100, 1),
        ("i8", "ugt", 100, 200, 0),
        ("i16", "ule", 60000, 60000, 1),
        ("i16", "ule", 60001, 60000, 0),
    ] {
        let ir = format!("define i8 @func({ty} %a, {ty} %b) {{\n  %c = icmp {cc} {ty} %a, %b\n  %r = zext i1 %c to i8\n  ret i8 %r\n}}");
        let setup = if ty == "i16" {
            set("rc2", a) + &set("rc3", a >> 8) + &set("rc4", b) + &set("rc5", b >> 8)
        } else {
            set("rc2", a) + &set("rc3", b)
        };
        assert_eq!(run(&format!("cmp-{ty}-{cc}-{a}-{b}"), &ir, "func", &setup, ""), want, "{ty} {cc} {a} {b}");
    }
}

/// A byte widened by its sign: 0x80 gives 0xFF80, 5 gives 0x0005.
#[test]
fn a_signed_byte_widens_by_its_sign() {
    let ir = "define i16 @func(i8 %a) {\n  %r = sext i8 %a to i16\n  ret i16 %r\n}";
    for (a, low, high) in [(0x80, 0x80, 0xFF), (5, 5, 0)] {
        assert_eq!(run(&format!("sx-{a}-lo"), ir, "func", &set("rc2", a), ""), low);
        assert_eq!(run(&format!("sx-{a}-hi"), ir, "func", &set("rc2", a), "  txa\n"), high);
    }
}

/// `a op b` on longs through `func(a, b)`: `a` in rs1:rs2, `b` in rs3 and at the top of the soft
/// stack; the result is read from `a`, `x`, `rc2`, `rc3`.
fn long_binary(
    op: &str,
    a: u32,
    b: u32,
) -> u32 {
    let ir = format!("define i32 @func(i32 %a, i32 %b) {{\n  %r = {op} i32 %a, %b\n  ret i32 %r\n}}");
    let mut setup = STACK.to_owned();
    for (at, cell) in ["rc2", "rc3", "rc4", "rc5"].iter().enumerate() {
        setup += &set(cell, (a >> (8 * at)) as u16);
    }
    setup += &(set("rc6", b as u16) + &set("rc7", (b >> 8) as u16));
    setup += &format!("  lda #{}\n  sta $8000\n  lda #{}\n  sta $8001\n", (b >> 16) & 0xFF, b >> 24);
    let key = format!("{op}-{a}-{b}");
    [("", 0), ("  txa\n", 8), ("  lda rc2\n", 16), ("  lda rc3\n", 24)]
        .iter()
        .map(|(after, shift)| (run(&format!("long-{key}-{shift}"), &ir, "func", &setup, after) as u32) << shift)
        .sum()
}

/// Longs through the 32-bit runtime, expected values from integer arithmetic on paper:
/// 70000 * 3 = 0x33450; 4000000000 / 7 = 571428571 r 3; -100000 / 7 = -14285 r -5;
/// 1 << 31 = 0x80000000; 0x80000000 >> 31 is 1 logical, 0xFFFFFFFF arithmetic.
#[test]
fn long_operations_run() {
    for (op, a, b, want) in [
        ("mul", 70000u32, 3u32, 0x33450u32),
        ("mul", 100000, 100000, 0x540B_E400),
        ("udiv", 4_000_000_000, 7, 571_428_571),
        ("urem", 4_000_000_000, 7, 3),
        ("sdiv", (-100000i32) as u32, 7, (-14285i32) as u32),
        ("srem", (-100000i32) as u32, 7, (-5i32) as u32),
        ("shl", 1, 31, 0x8000_0000),
        ("lshr", 0x8000_0000, 31, 1),
        ("ashr", 0x8000_0000, 31, 0xFFFF_FFFF),
    ] {
        assert_eq!(long_binary(op, a, b), want, "i32 {op} {a} {b}");
    }
}

/// A shift of a long by a constant needs no runtime: 0x12345678 << 4 = 0x23456780.
#[test]
fn a_long_shift_by_a_constant_runs() {
    let ir = "define i32 @func(i32 %a) {\n  %r = shl i32 %a, 4\n  ret i32 %r\n}";
    let mut setup = STACK.to_owned();
    for (at, cell) in ["rc2", "rc3", "rc4", "rc5"].iter().enumerate() {
        setup += &set(cell, (0x1234_5678u32 >> (8 * at)) as u16);
    }
    let got: u32 = [("", 0), ("  txa\n", 8), ("  lda rc2\n", 16), ("  lda rc3\n", 24)]
        .iter()
        .map(|(after, shift)| (run(&format!("shl4-{shift}"), ir, "func", &setup, after) as u32) << shift)
        .sum();
    assert_eq!(got, 0x2345_6780);
}

/// A local struct `{ i8, i16, i8 }` (offsets 0, 1, 3: the layout aligns nothing), its fields stored
/// through constant GEPs and read back: 5 + 0x1234 + 7, the low byte of which is 5 + 0x34 + 7 = 0x40.
#[test]
fn a_local_struct_runs() {
    let ir = "define i8 @func() {\n  %s = alloca { i8, i16, i8 }\n\
  %a = getelementptr { i8, i16, i8 }, ptr %s, i32 0, i32 0\n  store i8 5, ptr %a\n\
  %b = getelementptr { i8, i16, i8 }, ptr %s, i32 0, i32 1\n  store i16 4660, ptr %b\n\
  %c = getelementptr { i8, i16, i8 }, ptr %s, i32 0, i32 2\n  store i8 7, ptr %c\n\
  %x = load i8, ptr %a\n  %y = load i16, ptr %b\n  %z = load i8, ptr %c\n\
  %yl = trunc i16 %y to i8\n  %t = add i8 %x, %yl\n  %r = add i8 %t, %z\n  ret i8 %r\n}";
    assert_eq!(run("struct", ir, "func", STACK, ""), 0x40);
}

/// An `[4 x i16]` filled and read through a variable index (scaled by 2): `a[i] = i * 3 + 1`, then
/// `a[2] + a[3]` = 7 + 10 = 17 (the index loop is a phi).
#[test]
fn a_local_array_with_a_variable_index_runs() {
    let ir = "define i8 @func() {\nentry:\n  %a = alloca [4 x i16]\n  br label %fill\n\
fill:\n  %i = phi i16 [0, %entry], [%n, %fill]\n  %m = mul i16 %i, 3\n  %v = add i16 %m, 1\n\
  %p = getelementptr [4 x i16], ptr %a, i16 0, i16 %i\n  store i16 %v, ptr %p\n  %n = add i16 %i, 1\n\
  %c = icmp ult i16 %n, 4\n  br i1 %c, label %fill, label %sum\n\
sum:\n  %p2 = getelementptr [4 x i16], ptr %a, i16 0, i16 2\n  %p3 = getelementptr [4 x i16], ptr %a, i16 0, i16 3\n\
  %x = load i16, ptr %p2\n  %y = load i16, ptr %p3\n  %s = add i16 %x, %y\n  %r = trunc i16 %s to i8\n  ret i8 %r\n}";
    assert_eq!(run("array", ir, "func", STACK, ""), 17);
}

/// A struct passed by pointer, not by value: `bump` adds 1 to field 0 and 2 to field 1 of a
/// `{ i8, i8 }`; the caller's local goes in as (3, 4) and comes out (4, 6): the sum is 10.
#[test]
fn a_struct_passed_by_pointer_runs() {
    let ir = "define void @bump(ptr %p) {\n  %q = getelementptr { i8, i8 }, ptr %p, i32 0, i32 1\n\
  %a = load i8, ptr %p\n  %b = load i8, ptr %q\n  %a1 = add i8 %a, 1\n  %b2 = add i8 %b, 2\n\
  store i8 %a1, ptr %p\n  store i8 %b2, ptr %q\n  ret void\n}\n\
define i8 @func() {\n  %s = alloca { i8, i8 }\n  %q = getelementptr { i8, i8 }, ptr %s, i32 0, i32 1\n\
  store i8 3, ptr %s\n  store i8 4, ptr %q\n  call void @bump(ptr %s)\n\
  %a = load i8, ptr %s\n  %b = load i8, ptr %q\n  %r = add i8 %a, %b\n  ret i8 %r\n}";
    let text = compile(ir, &["bump", "func"]);
    assert_eq!(common::run_with("bump", &format!("{STACK}  jsr func\n"), &text), 10);
}

/// A global table of pairs summed through a scaled index: `{ 1, 2 }, { 3, 4 }, { 5, 6 }` give
/// `t[0].a + t[1].b + t[2].a` = 1 + 4 + 5 = 10.
#[test]
fn a_global_array_of_structs_runs() {
    let ir = "@tab = global [3 x { i8, i8 }] zeroinitializer\n\
define i8 @func() {\n  %p0 = getelementptr [3 x { i8, i8 }], ptr @tab, i16 0, i16 0, i32 0\n\
  %p1 = getelementptr [3 x { i8, i8 }], ptr @tab, i16 0, i16 1, i32 1\n\
  %p2 = getelementptr [3 x { i8, i8 }], ptr @tab, i16 0, i16 2, i32 0\n\
  %a = load i8, ptr %p0\n  %b = load i8, ptr %p1\n  %c = load i8, ptr %p2\n  %s = add i8 %a, %b\n  %r = add i8 %s, %c\n  ret i8 %r\n}";
    let text = compile(ir, &["func"]);
    let data = ".segment \"DATA\"\ntab: .byte 1, 2, 3, 4, 5, 6\n.segment \"CODE\"\n";
    assert_eq!(common::run_with("tab", &format!("{STACK}  jsr func\n"), &format!("{text}{data}")), 10);
}

/// `pick(0)` takes the `a` side, 10; `pick(5)` falls through `b`, 20. Block `b` ends in a `br` that
/// layout turns into a fall-through, so it has no terminator node: its phi copy was dropped and
/// `pick(5)` returned the stale cell, 0.
#[test]
fn a_phi_copy_in_a_fall_through_block_runs() {
    let ir = "define i8 @pick(i8 %x) {\nentry:\n  %c = icmp eq i8 %x, 0\n  br i1 %c, label %a, label %b\na:\n  br label %join\nb:\n  br label %join\njoin:\n  %r = phi i8 [ 10, %a ], [ 20, %b ]\n  ret i8 %r\n}";
    for (x, want) in [(0, 10), (5, 20)] {
        assert_eq!(run(&format!("pick-{x}"), ir, "pick", &set("rc2", x), ""), want);
    }
}

/// `lost(5)` returns the phi `%i` as the loop exits: `%i` runs 0..4 and `%next` = 5 ends it, so 4.
/// The phi copy `%i := %next` ran before the branch on both edges, so the exit saw 5 (clang's
/// `find(9)` over `buf` returned 6, not 5).
#[test]
fn a_phi_copy_does_not_run_on_the_exit_edge() {
    let ir = "define i8 @lost(i8 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i8 [ 0, %entry ], [ %next, %loop ]\n  %next = add i8 %i, 1\n  %done = icmp eq i8 %next, %n\n  br i1 %done, label %exit, label %loop\nexit:\n  ret i8 %i\n}";
    assert_eq!(run("lost", ir, "lost", &set("rc2", 5), ""), 4);
}

/// `swap(n)` swaps `%a` and `%b` on each back edge and returns `%a`: n = 1 returns 1 (no swap yet),
/// n = 3 two swaps, back to 1. Sequential copies turned `a, b` into `b, b` (clang's `fib(12)` over
/// 16-bit phis returned 0).
#[test]
fn phi_copies_on_an_edge_are_parallel() {
    let ir = "define i8 @swap(i8 %n) {\nentry:\n  br label %loop\nloop:\n  %a = phi i8 [ 1, %entry ], [ %b, %loop ]\n  %b = phi i8 [ 2, %entry ], [ %a, %loop ]\n  %k = phi i8 [ %n, %entry ], [ %k1, %loop ]\n  %k1 = sub i8 %k, 1\n  %z = icmp eq i8 %k1, 0\n  br i1 %z, label %exit, label %loop\nexit:\n  ret i8 %a\n}";
    for (n, want) in [(1, 1), (2, 2), (3, 1)] {
        assert_eq!(run(&format!("swap-{n}"), ir, "swap", &set("rc2", n), ""), want);
    }
}

/// A five-case `switch` (a comparison tree of blocks) to a block with phis is refused: its edges
/// shared one copy list, so `swapsw(1)` returned 2 instead of 1 (the swap ran on the exit edge).
#[test]
fn a_switch_to_a_block_with_phis_is_refused() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"\ndefine i8 @swapsw(i8 %n) {\nentry:\n  br label %loop\nloop:\n  %a = phi i8 [ 1, %entry ], [ %b, %loop ]\n  %b = phi i8 [ 2, %entry ], [ %a, %loop ]\n  %k = phi i8 [ %n, %entry ], [ %k1, %loop ]\n  %k1 = sub i8 %k, 1\n  switch i8 %k1, label %loop [ i8 0, label %exit\n  i8 10, label %exit\n  i8 20, label %exit\n  i8 30, label %exit\n  i8 40, label %exit ]\nexit:\n  ret i8 %a\n}";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let refused = select_function(&module, "swapsw", Cpu::Mos6502).err().expect("refused");
    assert!(refused.contains("switch"), "{refused}");
}

const TABLE: &str = "@t = global [4 x i8] [i8 10, i8 20, i8 30, i8 40]\n";
const T_DATA: &str = ".segment \"DATA\"\nt: .byte 10, 20, 30, 40\n.segment \"CODE\"\n";

/// `t[i]` and `t[i] = v` with a byte index are `lda t,x` and `sta t,x`, not a 16-bit address built
/// in a pointer pair: `pick(2)` reads 30, and `put(3, 99)` then `pick(3)` reads 99.
#[test]
fn a_global_indexed_by_a_byte_is_absolute_x() {
    let ir = format!("{TABLE}define i8 @pick(i8 %i) {{\n  %z = zext i8 %i to i16\n  %p = getelementptr [4 x i8], ptr @t, i16 0, i16 %z\n  %v = load i8, ptr %p\n  ret i8 %v\n}}\n\
define void @put(i8 %i, i8 %v) {{\n  %z = zext i8 %i to i16\n  %p = getelementptr [4 x i8], ptr @t, i16 0, i16 %z\n  store i8 %v, ptr %p\n  ret void\n}}");
    let text = compile(&ir, &["pick", "put"]);
    assert!(text.contains("lda t,x") && text.contains("sta t,x"), "{text}");
    assert_eq!(common::run_with("tx-pick", &format!("{}  jsr pick\n", set("rc2", 2)), &format!("{text}{T_DATA}")), 30);
    let after = format!("{}{}  jsr put\n{}  jsr pick\n", set("rc2", 3), set("rc3", 99), set("rc2", 3));
    assert_eq!(common::run_with("tx-put", &after, &format!("{text}{T_DATA}")), 99);
}
