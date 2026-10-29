//! The register allocator path of `select::function_text`, against the cell
//! path it replaces. Every expected value is worked out by hand.

mod common;

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text_with;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8";

fn compile(
    ir: &str,
    name: &str,
    allocate: bool,
) -> String {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let selection = select_function(&module, name, Cpu::Mos6502).expect("selects");
    function_text_with(&module, name, &selection, allocate).expect("emits")
}

fn set(
    register: &str,
    value: u16,
) -> String {
    format!("  lda #{}\n  sta {register}\n", value & 0xFF)
}

const ADD16: &str = "define i16 @add16(i16 %a, i16 %b) {\n  %r = add i16 %a, %b\n  ret i16 %r\n}";

/// A word add kept its low byte in `y` while the high byte was added; the cell
/// path stored it to `rc2` and loaded it back (18 bytes, now 13).
#[test]
fn a_word_add_keeps_its_low_byte_in_a_register() {
    let text = compile(ADD16, "add16", true);
    assert!(text.contains("  tay\n") && text.contains("  tya\n"), "{text}");
    assert!(!text.contains("sta rc"), "{text}");
    // 300 + 5000 = 5300 = 0x14B4
    let setup = set("rc2", 300) + &set("rc3", 300 >> 8) + &set("rc4", 5000) + &set("rc5", 5000 >> 8);
    let run = |after: &str| common::run_with("alloc-add16", &format!("{setup}  jsr add16\n{after}"), &text);
    assert_eq!(run(""), 0xB4);
    assert_eq!(run("  txa\n"), 0x14);
}

/// Ten sums are live at once, more than `a`, `x`, `y` and six zero-page bytes
/// can hold: the allocator spills one and the result is still right. With
/// a, b, c, d, e, f = 1..6 the sums are 3 5 7 9 11 4 6 8 10 7 and their xor is
/// 4.
#[test]
fn more_live_values_than_registers_spill_and_still_compute() {
    let pairs = [
        ("a", "b"),
        ("b", "c"),
        ("c", "d"),
        ("d", "e"),
        ("e", "f"),
        ("a", "c"),
        ("b", "d"),
        ("c", "e"),
        ("d", "f"),
        ("a", "f"),
    ];
    let mut ir = String::from("define i8 @many(i8 %a, i8 %b, i8 %c, i8 %d, i8 %e, i8 %f) {\n");
    for (n, (x, y)) in pairs.iter().enumerate() {
        ir += &format!("  %t{n} = add i8 %{x}, %{y}\n");
    }
    ir += "  %x0 = xor i8 %t0, %t1\n";
    for n in 2..pairs.len() {
        ir += &format!("  %x{} = xor i8 %x{}, %t{n}\n", n - 1, n - 2);
    }
    ir += &format!("  ret i8 %x{}\n}}", pairs.len() - 2);
    let text = compile(&ir, "many", true);
    assert!(text.contains("s_many_0"), "no spill in:\n{text}");
    let setup: String = (1..=6).map(|n| set(&format!("rc{}", n + 1), n)).collect();
    assert_eq!(common::run_with("alloc-many", &format!("{setup}  jsr many\n"), &text), 4);
}

const GCD: &str = "define i8 @gcd(i8 %0, i8 %1) {
  %e = icmp eq i8 %0, %1
  br i1 %e, label %done, label %loop
loop:
  %b = phi i8 [ %b2, %loop ], [ %1, %2 ]
  %a = phi i8 [ %a2, %loop ], [ %0, %2 ]
  %gt = icmp ugt i8 %a, %b
  %narrow = select i1 %gt, i8 %b, i8 0
  %a2 = sub i8 %a, %narrow
  %z = select i1 %gt, i8 0, i8 %a
  %b2 = sub i8 %b, %z
  %same = icmp eq i8 %a2, %b2
  br i1 %same, label %done, label %loop
done:
  %r = phi i8 [ %0, %2 ], [ %a2, %loop ]
  ret i8 %r
}";

/// A loop with phis, selects and two exits: a block that only jumps is
/// threaded away (the cell path kept `beq`, `jmp` and an edge block; 109 bytes,
/// now 72) and gcd(12, 18) = 6, gcd(7, 13) = 1, gcd(9, 9) = 9.
#[test]
fn a_loop_with_selects_and_phis_runs_without_edge_jumps() {
    let ir =
        GCD.replace("%2", "%entry").replace("define i8 @gcd(i8 %0, i8 %1) {", "define i8 @gcd(i8 %0, i8 %1) {\nentry:");
    let text = compile(&ir, "gcd", true);
    assert!(!text.contains("gcd_e0_2"), "an edge block that only jumps is still there:\n{text}");
    for (a, b, want) in [(12, 18, 6), (7, 13, 1), (9, 9, 9)] {
        let setup = set("rc2", a) + &set("rc3", b);
        assert_eq!(common::run_with(&format!("alloc-gcd-{a}"), &format!("{setup}  jsr gcd\n"), &text), want);
    }
}

/// Bytes of a global at constant offsets are `lda buf+1`, not a pointer in the
/// zero page and `(rc),y` (the cell path: 40 bytes, now 28). With buf = 1 2 3
/// 4, buf[1] + buf[3] = 6.
#[test]
fn a_global_at_a_constant_offset_is_an_absolute_load() {
    let ir = "@buf = global [4 x i8] [i8 1, i8 2, i8 3, i8 4]\n\
        define i8 @pair() {\n  %p = getelementptr i8, ptr @buf, i16 1\n  %a = load i8, ptr %p\n  %q = getelementptr i8, ptr @buf, i16 3\n  %b = load i8, ptr %q\n  %r = add i8 %a, %b\n  ret i8 %r\n}";
    let text = compile(ir, "pair", true);
    assert!(text.contains("lda buf+1") && text.contains("buf+3"), "{text}");
    assert!(!text.contains("),y"), "{text}");
    let data = ".segment \"DATA\"\nbuf: .byte 1, 2, 3, 4\n.segment \"CODE\"\n";
    assert_eq!(common::run_with("alloc-pair", "  jsr pair\n", &format!("{text}{data}")), 6);
}

/// A word select keeps both bytes of the winner whichever side it is: max(300,
/// 5000) = 5000 = 0x1388 and max(700, 20) = 700 = 0x02BC, low byte in `a`,
/// high in `x`.
#[test]
fn a_word_select_returns_the_larger_word() {
    let ir = "define i16 @max16(i16 %a, i16 %b) {\n  %c = icmp ugt i16 %a, %b\n  %r = select i1 %c, i16 %a, i16 %b\n  ret i16 %r\n}";
    let text = compile(ir, "max16", true);
    for (a, b, lo, hi) in [(300u16, 5000u16, 0x88, 0x13), (700, 20, 0xBC, 0x02)] {
        let setup = set("rc2", a) + &set("rc3", a >> 8) + &set("rc4", b) + &set("rc5", b >> 8);
        let run = |after: &str| common::run_with("alloc-max16", &format!("{setup}  jsr max16\n{after}"), &text);
        assert_eq!(run(""), lo);
        assert_eq!(run("  txa\n"), hi);
    }
}

/// A value live across a runtime call cannot stay in a register the routine
/// clobbers: it goes to a slot and comes back. a = 5: t = 6, r = 5 * 3 = 15,
/// t + r = 21.
#[test]
fn a_value_live_across_a_call_survives_it() {
    let ir =
        "define i8 @across(i8 %a) {\n  %t = add i8 %a, 1\n  %r = mul i8 %a, 3\n  %s = add i8 %r, %t\n  ret i8 %s\n}";
    let text = compile(ir, "across", true);
    assert!(text.contains("jsr __mulqi3") && text.contains("s_across_"), "{text}");
    assert_eq!(common::run_with("alloc-across", &format!("{}  jsr across\n", set("rc2", 5)), &text), 21);
}

const RECURSIVE: &str = "define i8 @rec(i8 %n) {
entry:
  %z = icmp eq i8 %n, 0
  br i1 %z, label %base, label %go
base:
  ret i8 0
go:
  %m = sub i8 %n, 1
  %r = call i8 @rec(i8 %m)
  %s = add i8 %r, %n
  ret i8 %s
}";

/// Values live across a call were static (cells or slots), so a function that
/// calls itself read the deepest call's `n` after the return: rec(4) = 4 + 3 +
/// 2
/// + 1 = 10 came out as 4 on both paths. On the allocator path they are spilled
/// to the soft stack and rec(4) is 10; the cell path still refuses it.
#[test]
fn a_recursive_function_keeps_its_values_on_the_soft_stack() {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{RECURSIVE}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let selection = select_function(&module, "rec", Cpu::Mos6502).expect("selects");
    let why = function_text_with(&module, "rec", &selection, false).expect_err("the cell path refuses recursion");
    assert!(why.contains("recursive"), "{why}");
    let code = function_text_with(&module, "rec", &selection, true).expect("the allocator path builds it");
    assert!(!code.contains("s_rec_"), "a static slot in a recursive function:\n{code}");
    for (n, want) in [(4, 10), (1, 1), (0, 0)] {
        assert_eq!(common::run_with(&format!("alloc-rec-{n}"), &format!("{}  jsr rec\n", set("rc2", n)), &code), want);
    }
}

/// fib(n) = fib(n-1) + fib(n-2): two calls, the first result live across the
/// second. fib(10) = 55, fib(7) = 13.
#[test]
fn a_doubly_recursive_function_runs() {
    let ir = "define i8 @fibr(i8 %n) {
entry:
  %small = icmp ult i8 %n, 2
  br i1 %small, label %base, label %go
base:
  ret i8 %n
go:
  %a = sub i8 %n, 1
  %x = call i8 @fibr(i8 %a)
  %b = sub i8 %n, 2
  %y = call i8 @fibr(i8 %b)
  %s = add i8 %x, %y
  ret i8 %s
}";
    let code = compile(ir, "fibr", true);
    for (n, want) in [(10, 55), (7, 13)] {
        assert_eq!(
            common::run_with(&format!("alloc-fibr-{n}"), &format!("{}  jsr fibr\n", set("rc2", n)), &code),
            want
        );
    }
}

fn on_the_allocator(
    ir: &str,
    name: &str,
) -> bool {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let selection = select_function(&module, name, Cpu::Mos6502).expect("selects");
    let (items, _) = llrm_6502::select::function_items(&module, name, &selection).expect("items");
    llrm_6502::alloc::build(name, &items).is_ok()
}

const SUMP: &str = "define i8 @sump(ptr %p, i8 %n) {
entry:
  %z = icmp eq i8 %n, 0
  br i1 %z, label %done, label %loop
loop:
  %i = phi i8 [ 0, %entry ], [ %i2, %loop ]
  %s = phi i8 [ 0, %entry ], [ %s2, %loop ]
  %w = zext i8 %i to i16
  %q = getelementptr i8, ptr %p, i16 %w
  %v = load i8, ptr %q
  %s2 = add i8 %s, %v
  %i2 = add i8 %i, 1
  %c = icmp eq i8 %i2, %n
  br i1 %c, label %done, label %loop
done:
  %r = phi i8 [ 0, %entry ], [ %s2, %loop ]
  ret i8 %r
}";

/// A byte sum through a pointer argument: the pointer is a zero-page pair
/// (`lda (rc),y`), and the function is built by the allocator, not the cell
/// path (which had no pair class). data = 3 1 4 1, n = 4: 9.
#[test]
fn a_sum_through_a_pointer_runs_on_the_allocator() {
    assert!(on_the_allocator(SUMP, "sump"));
    let text = compile(SUMP, "sump", true);
    assert!(text.contains("),y"), "{text}");
    let data = ".segment \"DATA\"\nvals: .byte 3, 1, 4, 1\n.segment \"CODE\"\n";
    let setup = "  lda #<vals\n  sta rc2\n  lda #>vals\n  sta rc3\n  lda #4\n  sta rc4\n";
    assert_eq!(common::run_with("alloc-sump", &format!("{setup}  jsr sump\n"), &format!("{text}{data}")), 9);
}

/// A pointer live across a call is spilled as a pair and comes back whole:
/// the second read through it sees the same bytes. data = 3 1 4 1:
/// 4 + 15 + 1 = 20.
#[test]
fn a_pointer_live_across_a_call_survives_it() {
    let ir = "define i8 @keep(ptr %p, i8 %a) {
  %x = getelementptr i8, ptr %p, i16 2
  %u = load i8, ptr %x
  %m = mul i8 %a, 3
  %y = getelementptr i8, ptr %p, i16 3
  %w = load i8, ptr %y
  %s = add i8 %u, %m
  %t = add i8 %s, %w
  ret i8 %t
}";
    assert!(on_the_allocator(ir, "keep"));
    let text = compile(ir, "keep", true);
    let data = ".segment \"DATA\"\nvals: .byte 3, 1, 4, 1\n.segment \"CODE\"\n";
    let setup = "  lda #<vals\n  sta rc2\n  lda #>vals\n  sta rc3\n  lda #5\n  sta rc4\n";
    assert!(text.contains("s_keep_"), "no spill:\n{text}");
    assert_eq!(common::run_with("alloc-keep", &format!("{setup}  jsr keep\n"), &format!("{text}{data}")), 20);
}

/// A recursive function with a local array and a value live across the call:
/// the spill area and the local share one frame and neither overwrites the
/// other. Each level adds its own `n` twice, once read back from its local
/// and once from the register that was spilled across the call.
#[test]
fn a_recursive_function_with_a_local_array_keeps_both() {
    let ir = "define i8 @loc(i8 %n) {
entry:
  %buf = alloca [2 x i8]
  store i8 %n, ptr %buf
  %z = icmp eq i8 %n, 0
  br i1 %z, label %base, label %go
base:
  ret i8 0
go:
  %m = sub i8 %n, 1
  %r = call i8 @loc(i8 %m)
  %v = load i8, ptr %buf
  %s = add i8 %r, %v
  %t = add i8 %s, %n
  ret i8 %t
}";
    let code = compile(ir, "loc", true);
    assert!(!code.contains("s_loc_"), "{code}");
    // loc(n) = loc(n-1) + n + n: loc(1) = 2, loc(2) = 6, loc(3) = 12
    for (n, want) in [(1, 2), (2, 6), (3, 12)] {
        assert_eq!(common::run_with(&format!("alloc-loc-{n}"), &format!("{}  jsr loc\n", set("rc2", n)), &code), want);
    }
}
