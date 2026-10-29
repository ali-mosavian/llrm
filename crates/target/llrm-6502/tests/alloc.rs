//! The register allocator path of `select::function_text`, against the cell
//! path it replaces. Every expected value is worked out by hand.

mod common;

use llrm_6502::alloc::Tuning;
use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8";

fn compile(
    ir: &str,
    name: &str,
) -> String {
    compile_on(ir, name, 16)
}

/// [`compile`] for a platform that gives the imaginary registers `zero_page`
/// bytes.
fn compile_on(
    ir: &str,
    name: &str,
    zero_page: u8,
) -> String {
    compile_at(ir, name, zero_page, Tuning::SPEED)
}

/// [`compile_on`] at a level: `Tuning::SIZE` takes a byte saved at up to eight
/// clocks.
fn compile_at(
    ir: &str,
    name: &str,
    zero_page: u8,
    tuning: Tuning,
) -> String {
    llrm_spike_mf::spill::VERIFY_SLOTS.store(true, std::sync::atomic::Ordering::Relaxed);
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    let mut module = llrm_mir::parse::module(&text).expect("parses");
    llrm_6502::prepare::split_phi_edges(&mut module);
    let mut selection = select_function(&module, name, Cpu::Mos6502).expect("selects");
    selection.zero_page = zero_page;
    selection.tuning = tuning;
    function_text(&module, name, &selection).expect("emits")
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
    let text = compile(ADD16, "add16");
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
    let text = compile(&ir, "many");
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
    let text = compile(&ir, "gcd");
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
    let text = compile(ir, "pair");
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
    let text = compile(ir, "max16");
    // the result starts as one operand and the other is moved in on one path:
    // no jump over arms
    assert!(!text.lines().any(|l| l.contains("jmp ") && l.contains("_cmd")), "{text}");
    for (a, b, lo, hi) in [(300u16, 5000u16, 0x88, 0x13), (700, 20, 0xBC, 0x02)] {
        let setup = set("rc2", a) + &set("rc3", a >> 8) + &set("rc4", b) + &set("rc5", b >> 8);
        let run = |after: &str| common::run_with("alloc-max16", &format!("{setup}  jsr max16\n{after}"), &text);
        assert_eq!(run(""), lo);
        assert_eq!(run("  txa\n"), hi);
    }
}

/// A value live across a runtime call cannot stay in a register the routine
/// clobbers: it goes to a slot and comes back. a = 5: t = 6, r = 5 * 11 = 55,
/// t + r = 61; 11 is not `2^n +- 1` times a power of two, so it stays a call.
#[test]
fn a_value_live_across_a_call_survives_it() {
    let ir =
        "define i8 @across(i8 %a) {\n  %t = add i8 %a, 1\n  %r = mul i8 %a, 11\n  %s = add i8 %r, %t\n  ret i8 %s\n}";
    let text = compile(ir, "across");
    assert!(text.contains("jsr __mulqi3") && text.contains("s_across_"), "{text}");
    assert_eq!(common::run_with("alloc-across", &format!("{}  jsr across\n", set("rc2", 5)), &text), 61);
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
/// + 1 = 10 came out as 4. They are spilled
/// to the soft stack and rec(4) is 10.
#[test]
fn a_recursive_function_keeps_its_values_on_the_soft_stack() {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{RECURSIVE}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let selection = select_function(&module, "rec", Cpu::Mos6502).expect("selects");
    let code = function_text(&module, "rec", &selection).expect("it builds");
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
    let code = compile(ir, "fibr");
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
    let text = compile(SUMP, "sump");
    assert!(text.contains("),y"), "{text}");
    let data = ".segment \"DATA\"\nvals: .byte 3, 1, 4, 1\n.segment \"CODE\"\n";
    let setup = "  lda #<vals\n  sta rc2\n  lda #>vals\n  sta rc3\n  lda #4\n  sta rc4\n";
    assert_eq!(common::run_with("alloc-sump", &format!("{setup}  jsr sump\n"), &format!("{text}{data}")), 9);
}

/// A pointer live across a call is spilled as a pair and comes back whole:
/// the second read through it sees the same bytes. data = 3 1 4 1:
/// 4 + 55 + 1 = 60.
#[test]
fn a_pointer_live_across_a_call_survives_it() {
    let ir = "define i8 @keep(ptr %p, i8 %a) {
  %x = getelementptr i8, ptr %p, i16 2
  %u = load i8, ptr %x
  %m = mul i8 %a, 11
  %y = getelementptr i8, ptr %p, i16 3
  %w = load i8, ptr %y
  %s = add i8 %u, %m
  %t = add i8 %s, %w
  ret i8 %t
}";
    assert!(on_the_allocator(ir, "keep"));
    let text = compile(ir, "keep");
    let data = ".segment \"DATA\"\nvals: .byte 3, 1, 4, 1\n.segment \"CODE\"\n";
    let setup = "  lda #<vals\n  sta rc2\n  lda #>vals\n  sta rc3\n  lda #5\n  sta rc4\n";
    assert!(text.contains("s_keep_"), "no spill:\n{text}");
    assert_eq!(common::run_with("alloc-keep", &format!("{setup}  jsr keep\n"), &format!("{text}{data}")), 60);
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
    let code = compile(ir, "loc");
    assert!(!code.contains("s_loc_"), "{code}");
    // loc(n) = loc(n-1) + n + n: loc(1) = 2, loc(2) = 6, loc(3) = 12
    for (n, want) in [(1, 2), (2, 6), (3, 12)] {
        assert_eq!(common::run_with(&format!("alloc-loc-{n}"), &format!("{}  jsr loc\n", set("rc2", n)), &code), want);
    }
}

/// Run `moves` (as `(dest, source)` register names) on sim65 with `rc2` = 1,
/// `rc3` = 2, `x` = 5 and `a` = 9, then return the byte `read` names.
fn moved(
    moves: &[(&str, &str)],
    a_live: bool,
    read: &str,
) -> i32 {
    use llrm_6502::alloc::parallel_move;
    let id = |name: &str| match name {
        "a" => llrm_6502::reg::A,
        "x" => llrm_6502::reg::X,
        "rc2" => llrm_6502::reg::RC2,
        "rc4" => llrm_6502::reg::RC4,
        _ => llrm_6502::reg::RC3,
    };
    let moves: Vec<_> = moves.iter().map(|(d, s)| (id(d), id(s))).collect();
    let mut code = String::from("pmove:\n");
    parallel_move(&mut code, &moves, a_live);
    code.push_str(&format!("  {}\n  rts\n", if read == "a" { "nop".to_owned() } else { format!("lda {read}") }));
    let main = "  lda #1\n  sta rc2\n  lda #2\n  sta rc3\n  lda #3\n  sta rc4\n  ldx #5\n  lda #9\n  jsr pmove\n";
    common::run_with(&format!("pm-{}", code.len()), main, &code)
}

/// A swap of two cells keeps a live `a` and exchanges them; moving `a` into a
/// cell another move reads (`rc3 := a; rc2 := rc3`) must not let the second
/// move clobber `a` first. The pair `PACK` of an add result did that
/// (`lda rc5; sta rc2; sta rc3`), and the array-index loop returned 254 for 17.
#[test]
fn parallel_moves_swap_and_keep_a() {
    let swap = [("rc2", "rc3"), ("rc3", "rc2")];
    assert_eq!(moved(&swap, true, "a"), 9);
    assert_eq!(moved(&swap, true, "rc2"), 2);
    assert_eq!(moved(&swap, true, "rc3"), 1);
    let through = [("rc3", "a"), ("rc2", "rc3")];
    assert_eq!(moved(&through, false, "rc3"), 9);
    assert_eq!(moved(&through, false, "rc2"), 2);
    // `a := rc2` must come after `rc3 := rc4`, which goes through `a`: the
    // unpack of a pointer into `a` and a cell returned `rc4` in `a` (slen
    // looped forever)
    let into_a = [("a", "rc2"), ("rc3", "rc4")];
    assert_eq!(moved(&into_a, false, "a"), 1);
    assert_eq!(moved(&into_a, false, "rc3"), 3);
    let to_x = [("rc2", "x"), ("x", "rc2")];
    assert_eq!(moved(&to_x, true, "rc2"), 5);
}

/// A byte loaded, widened and used in a word subtract was a zero-extending
/// load no pattern selected (`CannotSelect(Load)`): scmp, crc16 and sum16 of
/// the second bench set, and any `s += p[i]` on an `unsigned`. k = 1000, the
/// byte 7: 1000 - 7 = 993 = 0x03E1.
#[test]
fn a_loaded_byte_widened_into_a_word_subtract_runs() {
    let ir = "define i16 @widen(ptr %p, i16 %k) {\n  %b = load i8, ptr %p\n  %w = zext i8 %b to i16\n  %r = sub i16 %k, %w\n  ret i16 %r\n}";
    let text = compile(ir, "widen");
    let data = ".segment \"DATA\"\nval: .byte 7\n.segment \"CODE\"\n";
    let setup = "  lda #<val\n  sta rc2\n  lda #>val\n  sta rc3\n  lda #<1000\n  sta rc4\n  lda #>1000\n  sta rc5\n";
    let run = |after: &str| {
        common::run_with("alloc-widen", &format!("{setup}  jsr widen\n{after}"), &format!("{text}{data}"))
    };
    assert_eq!(run(""), 0xE1);
    assert_eq!(run("  txa\n"), 0x03);
}

const CRC16: &str = "define i16 @crc16(ptr %p, i8 %n) {\n\
entry:\n\
  %z = icmp eq i8 %n, 0\n\
  br i1 %z, label %done, label %loop\n\
loop:\n\
  %i = phi i8 [ %i2, %loop ], [ 0, %entry ]\n\
  %c = phi i16 [ %c8, %loop ], [ -1, %entry ]\n\
  %iw = zext i8 %i to i16\n\
  %q = getelementptr i8, ptr %p, i16 %iw\n\
  %b = load i8, ptr %q\n\
  %bw = zext i8 %b to i16\n\
  %sh = shl i16 %bw, 8\n\
  %x0 = xor i16 %sh, %c\n\
  %s0 = shl i16 %x0, 1\n\
  %e0 = xor i16 %s0, 4129\n\
  %neg0 = icmp slt i16 %x0, 0\n\
  %v0 = select i1 %neg0, i16 %e0, i16 %s0\n\
  %s1 = shl i16 %v0, 1\n\
  %e1 = xor i16 %s1, 4129\n\
  %neg1 = icmp slt i16 %v0, 0\n\
  %v1 = select i1 %neg1, i16 %e1, i16 %s1\n\
  %s2 = shl i16 %v1, 1\n\
  %e2 = xor i16 %s2, 4129\n\
  %neg2 = icmp slt i16 %v1, 0\n\
  %v2 = select i1 %neg2, i16 %e2, i16 %s2\n\
  %s3 = shl i16 %v2, 1\n\
  %e3 = xor i16 %s3, 4129\n\
  %neg3 = icmp slt i16 %v2, 0\n\
  %v3 = select i1 %neg3, i16 %e3, i16 %s3\n\
  %s4 = shl i16 %v3, 1\n\
  %e4 = xor i16 %s4, 4129\n\
  %neg4 = icmp slt i16 %v3, 0\n\
  %v4 = select i1 %neg4, i16 %e4, i16 %s4\n\
  %s5 = shl i16 %v4, 1\n\
  %e5 = xor i16 %s5, 4129\n\
  %neg5 = icmp slt i16 %v4, 0\n\
  %v5 = select i1 %neg5, i16 %e5, i16 %s5\n\
  %s6 = shl i16 %v5, 1\n\
  %e6 = xor i16 %s6, 4129\n\
  %neg6 = icmp slt i16 %v5, 0\n\
  %v6 = select i1 %neg6, i16 %e6, i16 %s6\n\
  %s7 = shl i16 %v6, 1\n\
  %e7 = xor i16 %s7, 4129\n\
  %neg7 = icmp slt i16 %v6, 0\n\
  %v7 = select i1 %neg7, i16 %e7, i16 %s7\n\
  %c8 = add i16 %v7, 0\n\
  %i2 = add i8 %i, 1\n\
  %more = icmp eq i8 %i2, %n\n\
  br i1 %more, label %done, label %loop\n\
done:\n\
  %r = phi i16 [ -1, %entry ], [ %c8, %loop ]\n\
  ret i16 %r\n\
}";

/// Spill reloads (`ldx`, `ldy`) set N and Z, and one sat between a compare and
/// the `bmi` of a word select that reads it: the select chose by the reload's
/// flags and crc16 over 12 bytes returned 0x3E66 for 0xA166 (bench set 2).
#[test]
fn a_reload_between_a_compare_and_its_branch_keeps_the_flags() {
    let text = compile(CRC16, "crc16");
    let data = ".segment \"DATA\"\nbytes: .byte 3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8\n.segment \"CODE\"\n";
    let setup = "  lda #<bytes\n  sta rc2\n  lda #>bytes\n  sta rc3\n  lda #12\n  sta rc4\n";
    let run = |after: &str| {
        common::run_with("alloc-crc16", &format!("{setup}  jsr crc16\n{after}"), &format!("{text}{data}"))
    };
    assert_eq!(run(""), 0x66);
    assert_eq!(run("  txa\n"), 0xA1);
}

const SCHR: &str = "define i8 @schr(ptr %0, i8 %1)  {\n\
  br label %3\n\
\n\
3:                                                ; preds = %3, %2\n\
  %.0 = phi ptr [ %0, %2 ], [ %5, %3 ]\n\
  %4 = load i8, ptr %.0, align 1\n\
  %.not = icmp eq i8 %4, 0\n\
  %.not8 = icmp eq i8 %4, %1\n\
  %or.cond = or i1 %.not, %.not8\n\
  %5 = getelementptr inbounds nuw i8, ptr %.0, i16 1\n\
  br i1 %or.cond, label %.critedge, label %3\n\
\n\
.critedge:                                        ; preds = %3\n\
  %6 = ptrtoint ptr %.0 to i16\n\
  %7 = ptrtoint ptr %0 to i16\n\
  %8 = sub i16 %6, %7\n\
  %extract.t = trunc i16 %8 to i8\n\
  %.off0 = select i1 %.not, i8 -1, i8 %extract.t\n\
  ret i8 %.off0\n\
}\n\
";

/// A loop leaving on `c == 0 || c == k`, then selecting on one of the two
/// tests, never finished compiling (bench set 2, `schr`): the process spun.
#[test]
fn a_loop_leaving_on_either_of_two_tests_compiles() {
    let (done, wait) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(compile(SCHR, "schr").len());
    });
    wait.recv_timeout(std::time::Duration::from_secs(10)).expect("compiling schr does not return");
}

fn refusal(
    ir: &str,
    name: &str,
) -> String {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    match select_function(&module, name, Cpu::Mos6502) {
        Err(why) => why,
        Ok(selection) => function_text(&module, name, &selection).expect_err("is refused"),
    }
}

/// A spill slot of a recursive function sits above its locals; one past
/// `(rc0),y`'s 255 is refused with its reason, not emitted with a wrapped
/// offset.
#[test]
fn a_spill_slot_beyond_256_bytes_is_refused_with_a_message() {
    let ir = "define i8 @rec(i8 %n) {\nentry:\n  %a = alloca [300 x i8]\n  store i8 %n, ptr %a\n  %z = icmp eq i8 %n, 0\n  \
              br i1 %z, label %base, label %step\nbase:\n  ret i8 0\nstep:\n  %m = sub i8 %n, 1\n  \
              %r = call i8 @rec(i8 %m)\n  %s = add i8 %r, %n\n  %p = load i8, ptr %a\n  %t = add i8 %s, %p\n  ret i8 %t\n}";
    assert!(refusal(ir, "rec").contains("a frame beyond 256 bytes"));
}

/// A switch into a block with phis is compiled (per-edge blocks carry the phi
/// copies): `sw(1)` is 7, any other value 9.
#[test]
fn a_switch_to_a_block_with_phis_selects_the_right_value() {
    let ir = "define i8 @sw(i8 %x) {\nentry:\n  switch i8 %x, label %other [ i8 1, label %join ]\nother:\n  br label %join\n\
              join:\n  %r = phi i8 [ 7, %entry ], [ 9, %other ]\n  ret i8 %r\n}";
    let text = compile(ir, "sw");
    let run = |x: u16| common::run_with("alloc-sw", &format!("{}  jsr sw\n", set("rc2", x)), &text);
    assert_eq!((run(1), run(2), run(0)), (7, 9, 9));
}

/// `f(p, x)` calls the function `p` points to with `x`: an indirect call, which
/// the 6502 has no instruction for, was refused ("an indirect call").
const CALL_THROUGH: &str = "define i8 @twice(i8 %x) {\n  %r = add i8 %x, %x\n  ret i8 %r\n}\n\
    define i8 @via(ptr %f, i8 %x) {\n  %r = call i8 %f(i8 %x)\n  %s = add i8 %r, 1\n  ret i8 %s\n}";

#[test]
fn an_indirect_call_enters_the_callee_and_returns() {
    let text = compile(CALL_THROUGH, "via") + &compile(CALL_THROUGH, "twice");
    // via(&twice, 20) = 41
    let setup = "  lda #<twice\n  sta rc2\n  lda #>twice\n  sta rc3\n  lda #20\n  sta rc4\n";
    assert_eq!(common::run_with("alloc-indirect", &format!("{setup}  jsr via\n"), &text), 41);
}

/// A function that calls itself through a pointer is as reentrant as one that
/// calls itself by name: its values live across the call must be on the soft
/// stack, not in static slots.
#[test]
fn a_function_reentered_through_a_pointer_keeps_its_values() {
    let ir = "define i8 @sum(ptr %f, i8 %n) {\nentry:\n  %z = icmp eq i8 %n, 0\n  br i1 %z, label %base, label %step\nbase:\n  \
              ret i8 0\nstep:\n  %m = sub i8 %n, 1\n  %r = call i8 %f(ptr %f, i8 %m)\n  %s = add i8 %r, %n\n  ret i8 %s\n}";
    let text = compile(ir, "sum");
    // sum(&sum, 4) = 4 + 3 + 2 + 1 = 10
    let setup = "  lda #<sum\n  sta rc2\n  lda #>sum\n  sta rc3\n  lda #4\n  sta rc4\n";
    assert_eq!(common::run_with("alloc-reenter", &format!("{setup}  jsr sum\n"), &text), 10);
}

const STRUCTS: &str = "%S = type { i16, i16 }\n\
define i16 @sumv(ptr byval(%S) %s) {\n  %a = load i16, ptr %s\n  %q = getelementptr i8, ptr %s, i16 2\n  %b = load i16, ptr %q\n  %r = add i16 %a, %b\n  ret i16 %r\n}\n\
define void @mk(ptr sret(%S) %o, i16 %x) {\n  store i16 %x, ptr %o\n  %q = getelementptr i8, ptr %o, i16 2\n  %y = add i16 %x, 1\n  store i16 %y, ptr %q\n  ret void\n}\n\
define i16 @pass(i16 %x) {\n  %l = alloca %S\n  store i16 %x, ptr %l\n  %q = getelementptr i8, ptr %l, i16 2\n  %y = add i16 %x, 1\n  store i16 %y, ptr %q\n  %r = call i16 @sumv(ptr byval(%S) %l)\n  ret i16 %r\n}\n\
define i16 @take(i16 %x) {\n  %o = alloca %S\n  call void @mk(ptr sret(%S) %o, i16 %x)\n  %a = load i16, ptr %o\n  %q = getelementptr i8, ptr %o, i16 2\n  %b = load i16, ptr %q\n  %r = add i16 %a, %b\n  ret i16 %r\n}";

fn structs(entry: &str) -> String {
    ["sumv", "mk", entry].iter().map(|f| compile(STRUCTS, f)).collect()
}

/// A struct passed by value (`byval`) and one returned through `sret`, the way
/// a C frontend lowers them: `pass(5)` is 5 + 6, `take(5)` the same through
/// `mk`.
#[test]
fn a_struct_passed_byval_and_returned_through_sret_round_trips() {
    for entry in ["pass", "take"] {
        let text = structs(entry);
        let run =
            common::run_with("alloc-struct", &format!("{}  jsr {entry}\n", set("rc2", 5) + &set("rc3", 0)), &text);
        assert_eq!(run, 11, "{entry}");
    }
}

/// `x < 0` on a word did `cmp #0; sbc #0` over both bytes: the sign is the high
/// byte alone.
#[test]
fn a_word_sign_test_reads_only_the_high_byte() {
    let ir = "define i8 @neg(i16 %x) {\n  %c = icmp slt i16 %x, 0\n  %r = select i1 %c, i8 7, i8 9\n  ret i8 %r\n}";
    let text = compile(ir, "neg");
    assert!(!text.contains("sbc"), "{text}");
    let run =
        |x: u16| common::run_with("alloc-neg", &format!("{}  jsr neg\n", set("rc2", x) + &set("rc3", x >> 8)), &text);
    assert_eq!((run(0xFFFF), run(0x8000), run(0x7FFF), run(0), run(5)), (7, 7, 9, 9, 9));
}

/// `(b << 8) ^ c` with `b` a widened byte (crc16's `c ^= p[i] << 8`) was eight
/// `asl`/`rol` pairs on a word: the byte is the high half.
#[test]
fn a_widened_byte_shifted_by_eight_and_xored_is_a_byte_move() {
    let ir = "define i16 @sh(i8 %b, i16 %c) {\n  %w = zext i8 %b to i16\n  %s = shl nuw i16 %w, 8\n  %r = xor i16 %s, %c\n  ret i16 %r\n}";
    let text = compile(ir, "sh");
    assert!(!text.contains("rol"), "{text}");
    // b in rc2, c in rc4:rc5
    let setup = set("rc2", 0x34) + &set("rc4", 0x1234) + &set("rc5", 0x12);
    let low = common::run_with("alloc-sh", &format!("{setup}  jsr sh\n"), &text);
    let high = common::run_with("alloc-sh", &format!("{setup}  jsr sh\n  txa\n"), &text);
    assert_eq!(((high as u16) << 8) | (low as u16 & 0xFF), 0x3400 ^ 0x1234);
}

/// A pointer walked through a loop (`p += 1`, `p += 3`) went UNPACK, a word
/// add, PACK on every trip: a dozen moves and a clobbered `a`. It is the low
/// byte bumped and the high one on carry, in the pair.
#[test]
fn a_pointer_bumped_in_a_loop_stays_a_pair() {
    for k in [1u16, 3] {
        let ir = format!(
            "define i16 @walk(ptr %p, i8 %n) {{\nentry:\n  br label %loop\nloop:\n  %q = phi ptr [ %p, %entry ], [ %q2, %loop ]\n  \
             %i = phi i8 [ 0, %entry ], [ %i2, %loop ]\n  %v = load volatile i8, ptr %q\n  %q2 = getelementptr i8, ptr %q, i16 {k}\n  \
             %i2 = add i8 %i, 1\n  %more = icmp ult i8 %i2, %n\n  br i1 %more, label %loop, label %done\ndone:\n  \
             %r = ptrtoint ptr %q2 to i16\n  ret i16 %r\n}}"
        );
        let text = compile(&ir, "walk");
        assert!(text.contains(if k == 1 { "inc rc" } else { "bcc walk_pa" }), "k={k}: {text}");
        assert!(!text.contains("tay") && !text.contains("s_walk"), "k={k}: {text}");
        for (p, n) in [(0x0100u16, 4u16), (0x01FE, 2), (0x01FF, 3), (0x12FF, 1)] {
            let setup = set("rc2", p) + &set("rc3", p >> 8) + &set("rc4", n);
            let low = common::run_with("alloc-walk", &format!("{setup}  jsr walk\n"), &text);
            let high = common::run_with("alloc-walk", &format!("{setup}  jsr walk\n  txa\n"), &text);
            assert_eq!(((high as u16) << 8) | (low as u16 & 0xFF), p + k * n, "k={k} p={p:#06x} n={n}");
        }
    }
}

/// A branch on `c == 0 || c == k` was two 0/1 bytes through two selects, an
/// `and` and a test (schr, 20 bytes over): it is one compare and branch per
/// test, no `and` of two bytes.
#[test]
fn a_branch_on_two_compares_is_two_branches() {
    let text = compile(SCHR, "schr");
    assert!(!text.contains("  and rc"), "{text}");
    let run = |c: u16| {
        // "ab\0" at the pointer in rc2:rc3 = data, c in rc4: returns the index
        // of c, or -1 at the end
        let data = ".segment \"DATA\"\ns: .byte 97, 98, 0\n.segment \"CODE\"\n";
        let setup = "  lda #<s\n  sta rc2\n  lda #>s\n  sta rc3\n".to_owned() + &set("rc4", c);
        common::run_with("alloc-schr", &format!("{setup}  jsr schr\n"), &format!("{text}{data}"))
    };
    assert_eq!((run(97), run(98), run(99)), (0, 1, 255));
}

/// `p + zext(i)` kept a cell holding zero just for the high byte's `adc`: the
/// zero is an immediate.
#[test]
fn a_widened_byte_added_to_a_word_adds_an_immediate_zero() {
    let ir = "define i16 @addb(i16 %a, i8 %b) {\n  %w = zext i8 %b to i16\n  %r = add i16 %a, %w\n  ret i16 %r\n}";
    let text = compile(ir, "addb");
    assert!(text.contains("adc #0"), "{text}");
    for (a, b) in [(0x01FFu16, 1u16), (0x1234, 0xFF), (0, 0)] {
        let setup = set("rc2", a) + &set("rc3", a >> 8) + &set("rc4", b);
        let low = common::run_with("alloc-addb", &format!("{setup}  jsr addb\n"), &text);
        let high = common::run_with("alloc-addb", &format!("{setup}  jsr addb\n  txa\n"), &text);
        assert_eq!(((high as u16) << 8) | (low as u16 & 0xFF), a + b);
    }
}

/// `p[i]` with `i` a byte did a word add of `p` and `i` into a scratch pair
/// before the load; it is `lda (p),y` with `i` in `y`.
#[test]
fn a_byte_index_into_a_pointer_loads_and_stores_through_y() {
    let ir = "define i8 @bump(ptr %p, i8 %i) {\n  %w = zext i8 %i to i16\n  %q = getelementptr i8, ptr %p, i16 %w\n  \
              %v = load i8, ptr %q\n  %n = add i8 %v, 1\n  store i8 %n, ptr %q\n  ret i8 %v\n}";
    let text = compile(ir, "bump");
    assert!(text.contains("),y"), "{text}");
    assert!(!text.contains("adc rc"), "{text}");
    // table[3] = 7 becomes 8 and 7 comes back
    let data = ".segment \"DATA\"\ntable: .byte 1, 2, 3, 7, 5\n.segment \"CODE\"\n";
    let setup = "  lda #<table\n  sta rc2\n  lda #>table\n  sta rc3\n  lda #3\n  sta rc4\n";
    let run =
        |after: &str| common::run_with("alloc-index", &format!("{setup}  jsr bump\n{after}"), &format!("{text}{data}"));
    assert_eq!(run(""), 7);
    assert_eq!(run("  ldy #3\n  lda table,y\n"), 8);
}

/// The sum of a byte table (`s += p[i]`) took the word-add path for the address
/// on every trip: the widened load is the same indexed load.
#[test]
fn a_widened_byte_loaded_at_a_byte_index_uses_y() {
    let ir = "define i16 @sum(ptr %p, i8 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i8 [ 0, %entry ], [ %i2, %loop ]\n  \
              %s = phi i16 [ 0, %entry ], [ %s2, %loop ]\n  %w = zext i8 %i to i16\n  %q = getelementptr i8, ptr %p, i16 %w\n  \
              %v = load i8, ptr %q\n  %x = zext i8 %v to i16\n  %s2 = add i16 %s, %x\n  %i2 = add i8 %i, 1\n  \
              %more = icmp ult i8 %i2, %n\n  br i1 %more, label %loop, label %done\ndone:\n  ret i16 %s2\n}";
    let text = compile(ir, "sum");
    assert!(!text.contains("adc rc2") && !text.contains("sta rc3"), "{text}");
    let data = ".segment \"DATA\"\ntable: .byte 1, 2, 3, 200, 5\n.segment \"CODE\"\n";
    let setup = "  lda #<table\n  sta rc2\n  lda #>table\n  sta rc3\n  lda #4\n  sta rc4\n";
    let run =
        |after: &str| common::run_with("alloc-at", &format!("{setup}  jsr sum\n{after}"), &format!("{text}{data}"));
    // 1 + 2 + 3 + 200 = 206
    assert_eq!((run(""), run("  txa\n")), (206, 0));
}

const DIGITS: &str = "define i8 @digits(i16 %v) {\nentry:\n  %big = icmp ugt i16 %v, 9\n  br i1 %big, label %loop, label %done\nloop:\n  \
    %n = phi i8 [ %n2, %loop ], [ 1, %entry ]\n  %x = phi i16 [ %q, %loop ], [ %v, %entry ]\n  %q = udiv i16 %x, 10\n  %n2 = add i8 %n, 1\n  \
    %more = icmp ugt i16 %x, 99\n  br i1 %more, label %loop, label %done\ndone:\n  %r = phi i8 [ 1, %entry ], [ %n2, %loop ]\n  ret i8 %r\n}";

/// The constants 10 and 0 of `v / 10` were stored to static slots and loaded
/// back before the call (`ldx #10; stx s_digits_5`): they are loaded again
/// where they are read.
#[test]
fn a_constant_is_loaded_again_not_stored_to_a_slot() {
    let text = compile(DIGITS, "digits");
    assert!(!text.contains("ldx #10\n  stx s_"), "{text}");
    assert!(!text.contains("ldx #0\n  stx s_digits_4"), "{text}");
}

const BSEARCH8: &str = "define i8 @bsearch8(ptr %0, i8 %1, i8 %2) {
entry:
  %.not = icmp eq i8 %1, 0
  br i1 %.not, label %exit, label %loop
loop:
  %lo = phi i8 [ %lo2, %loop ], [ 0, %entry ]
  %hi = phi i8 [ %hi2, %loop ], [ %1, %entry ]
  %t4 = zext i8 %hi to i16
  %t5 = zext i8 %lo to i16
  %t6 = add nuw nsw i16 %t5, %t4
  %t7 = lshr i16 %t6, 1
  %t8 = getelementptr inbounds nuw i8, ptr %0, i16 %t7
  %t9 = load i8, ptr %t8
  %t10 = icmp ult i8 %t9, %2
  %t11 = trunc nuw i16 %t7 to i8
  %t12 = add nuw i8 %t11, 1
  %hi2 = select i1 %t10, i8 %hi, i8 %t11
  %lo2 = select i1 %t10, i8 %t12, i8 %lo
  %t13 = icmp ult i8 %lo2, %hi2
  br i1 %t13, label %loop, label %exit
exit:
  %r = phi i8 [ 0, %entry ], [ %lo2, %loop ]
  ret i8 %r
}";

fn slot_count(text: &str) -> usize {
    text.lines().filter(|line| line.contains(": .res")).count()
}

/// Copies of a spilled value were moved slot to slot through a register, each
/// in a slot of its own: bsearch8 had eight slots and 152 bytes (set 2), now
/// four and 128. Slots never live together share one.
#[test]
fn spilled_copies_of_an_argument_share_its_slot() {
    let text = compile(BSEARCH8, "bsearch8");
    assert!(slot_count(&text) <= 4, "{}\n{text}", slot_count(&text));
}

/// digits kept the loop's word and counter in fifteen slots, copying each
/// from the last (269 bytes); the copies whose source is not written while
/// they live share its slot.
#[test]
fn a_loop_variable_and_its_copy_share_a_slot() {
    let text = compile(DIGITS, "digits");
    assert!(slot_count(&text) <= 8, "{}\n{text}", slot_count(&text));
}

/// A call defined no register, so the accumulator looked live from the entry
/// to the call's result and every slot-to-cell move was wrapped in `pha`/`pla`
/// (digits, set 2: 289 bytes, 2804 cycles; now 273 and 2577).
#[test]
fn nothing_is_pushed_around_moves_before_a_call() {
    let text = compile(DIGITS, "digits");
    // the only pushes are the callee-saved cells on entry
    let saves = (8..16).filter(|n| text.contains(&format!("  lda rc{n}\n  pha\n"))).count();
    assert_eq!(text.matches("  pha\n").count(), saves, "{text}");
}

/// A loop whose latch comes first in the layout: the latch's phi is read only
/// on the edge back to the header, a block laid out right after it and before
/// the body that writes the phi. It failed with "phi input was never written"
/// (lz4, set 3). `fwd8` carries a byte, `fwd16` a word.
fn forwarding(
    width: &str,
    extra: &str,
) -> String {
    format!(
        "define i8 @fwd{width}(i8 %n) {{\nentry:\n  %z = icmp eq i8 %n, 0\n  br i1 %z, label %done, label %head\n\
         latch:\n  %f = phi {t} [ %g, %body ]\n  %k = phi i8 [ %h, %body ]\n  %c = icmp ult i8 %k, 100\n  \
           br i1 %c, label %head, label %done\n\
         head:\n  %fw = phi {t} [ %f, %latch ], [ 1, %entry ]\n  %kk = phi i8 [ %k, %latch ], [ 0, %entry ]\n  br label %body\n\
         body:\n{ext}  %g = add {t} %fw, {kx}\n  %h = add i8 %kk, 7\n  br label %latch\n\
         done:\n  %fe = phi {t} [ 0, %entry ], [ %f, %latch ]\n  %ke = phi i8 [ 0, %entry ], [ %k, %latch ]\n{tail}\
           ret i8 %r\n}}",
        t = if width == "8" { "i8" } else { "i16" },
        ext = if width == "8" { "" } else { "  %kz = zext i8 %kk to i16\n" },
        kx = if width == "8" { "%kk" } else { "%kz" },
        tail = extra,
    )
}

#[test]
fn a_phi_forwarded_on_a_back_edge_is_known_before_its_block_is_laid_out() {
    let byte = forwarding("8", "  %r = xor i8 %fe, %ke\n");
    let word = forwarding(
        "16",
        "  %lo = trunc i16 %fe to i8\n  %hi16 = lshr i16 %fe, 8\n  %hi = trunc i16 %hi16 to i8\n  %x = xor i8 %lo, %ke\n  %r = xor i8 %x, %hi\n",
    );
    // n = 5 runs the body until k reaches 105; n = 0 skips the loop
    for (ir, name, want) in [(&byte, "fwd8", 137), (&word, "fwd16", 139)] {
        let text = compile(ir, name);
        for (arg, expected) in [(5, want), (0, 0)] {
            let setup = format!("  lda #{arg}\n  sta rc2\n");
            assert_eq!(
                common::run_with(&format!("alloc-{name}-{arg}"), &format!("{setup}  jsr {name}\n"), &text),
                expected,
                "{name}({arg})"
            );
        }
    }
}

/// Four pointers advance through a table by different strides, with a sum and a
/// counter: more live values than `rc2`..`rc7` hold. `quad` is a leaf (static
/// save slots); `quadcall` ends in an indirect call, so it runs on the soft
/// stack. buf = 3 1 4 1 5 9 2 6 5 3 5 8 9 7 9 3; strides 1, 2, 3, 1 from
/// offsets 1, 2, 3, 4; four trips sum (a + b) + (c ^ d) to 58; `quadcall`
/// doubles that, 116.
fn quad_ir(
    name: &str,
    call: bool,
) -> String {
    let (head, tail) = if call {
        ("ptr %p, i8 %n, ptr %f", "  %out = call i8 %f(i8 %r)\n  ret i8 %out")
    } else {
        ("ptr %p, i8 %n", "  ret i8 %r")
    };
    format!(
        "define i8 @{name}({head}) {{\nentry:\n  %p1 = getelementptr i8, ptr %p, i16 1\n  %p2 = getelementptr i8, ptr %p, i16 2\n  \
           %p3 = getelementptr i8, ptr %p, i16 3\n  %p4 = getelementptr i8, ptr %p, i16 4\n  %z = icmp eq i8 %n, 0\n  \
           br i1 %z, label %done, label %loop\n\
         loop:\n  %i = phi i8 [ 0, %entry ], [ %i2, %loop ]\n  %s = phi i8 [ 0, %entry ], [ %s2, %loop ]\n  \
           %q1 = phi ptr [ %p1, %entry ], [ %n1, %loop ]\n  %q2 = phi ptr [ %p2, %entry ], [ %n2, %loop ]\n  \
           %q3 = phi ptr [ %p3, %entry ], [ %n3, %loop ]\n  %q4 = phi ptr [ %p4, %entry ], [ %n4, %loop ]\n  \
           %a = load i8, ptr %q1\n  %b = load i8, ptr %q2\n  %c = load i8, ptr %q3\n  %d = load i8, ptr %q4\n  \
           %t1 = add i8 %a, %b\n  %t2 = xor i8 %c, %d\n  %t3 = add i8 %t1, %t2\n  %s2 = add i8 %s, %t3\n  \
           %n1 = getelementptr i8, ptr %q1, i16 1\n  %n2 = getelementptr i8, ptr %q2, i16 2\n  \
           %n3 = getelementptr i8, ptr %q3, i16 3\n  %n4 = getelementptr i8, ptr %q4, i16 1\n  \
           %i2 = add i8 %i, 1\n  %e = icmp eq i8 %i2, %n\n  br i1 %e, label %done, label %loop\n\
         done:\n  %r = phi i8 [ 0, %entry ], [ %s2, %loop ]\n{tail}\n}}\n\
         define i8 @twice(i8 %x) {{\n  %r = add i8 %x, %x\n  ret i8 %r\n}}"
    )
}

/// Callee-saved zero-page bytes (`rc8`..`rc15`): a function that needs more
/// than `rc2`..`rc7` uses them, saves what it touches on entry and restores it
/// before returning. Before, it spilled; the caller's `rc8`..`rc15` must come
/// back unchanged and the result be right, in a leaf and in a function on the
/// soft stack.
#[test]
fn a_function_short_of_cells_saves_and_restores_the_callee_saved_ones() {
    let data = ".segment \"DATA\"\ntab: .byte 3,1,4,1,5,9,2,6,5,3,5,8,9,7,9,3\n.segment \"CODE\"\n";
    let keep: [u8; 8] = [0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0x17, 0x28];
    let mut set = String::new();
    let mut check = String::from("  lda #0\n  sta rc2\n");
    for (i, v) in keep.iter().enumerate() {
        set += &format!("  lda #{v}\n  sta rc{}\n", 8 + i);
        check += &format!("  lda rc{}\n  eor #{v}\n  ora rc2\n  sta rc2\n", 8 + i);
    }
    check += "  lda rc2\n";
    let args = "  lda #<tab\n  sta rc2\n  lda #>tab\n  sta rc3\n  lda #4\n  sta rc4\n  lda #<twice\n  sta rc6\n  lda #>twice\n  sta rc7\n";
    for (name, call, want) in [("quad", false, 58), ("quadcall", true, 116)] {
        let ir = quad_ir(name, call);
        let text = compile(&ir, name) + &compile(&ir, "twice");
        assert!((8..16).any(|n| text.contains(&format!("rc{n}"))), "uses no callee-saved cell:\n{text}");
        let program = format!("{text}{data}");
        let result = common::run_with(&format!("alloc-{name}"), &format!("{set}{args}  jsr {name}\n"), &program);
        assert_eq!(result, want, "{name}");
        let kept =
            common::run_with(&format!("alloc-{name}-kept"), &format!("{set}{args}  jsr {name}\n{check}"), &program);
        assert_eq!(kept, 0, "{name} changed a callee-saved cell");
    }
}

/// A value live across one call: a callee-saved cell costs a save and a restore
/// (about eight bytes), a slot a store and a load; with both allocations priced
/// by the timing table the smaller is kept. A loop that calls on every trip
/// (digits) keeps the cell, which saves a reload per trip.
const ACROSS: &str = "define i8 @g(i8 %x) {\n  %r = add i8 %x, 3\n  ret i8 %r\n}\n\
    define i8 @across(i8 %x) {\n  %a = call i8 @g(i8 %x)\n  %b = add i8 %a, %x\n  ret i8 %b\n}";

#[test]
fn a_callee_saved_cell_is_taken_only_when_it_is_the_smaller_code() {
    let single = compile(ACROSS, "across") + &compile(ACROSS, "g");
    assert!(!single.contains("rc8"), "one call, one value:\n{single}");
    // across(5) = g(5) + 5 = 8 + 5
    let program = format!("{single}");
    assert_eq!(common::run_with("alloc-single-call", &format!("{}  jsr across\n", set("rc2", 5)), &program), 13);
    let looped = compile(DIGITS, "digits");
    assert!(
        (8..16).any(|n| looped.contains(&format!("rc{n}"))),
        "a loop of calls keeps its values in a cell:\n{looped}"
    );
}

/// The word result of `name` for a word argument in rc2:rc3 (low byte in A,
/// high byte in X).
fn run_word(
    tag: &str,
    text: &str,
    name: &str,
    x: u16,
) -> u16 {
    let setup = set("rc2", x) + &set("rc3", x >> 8);
    let low = common::run_with(tag, &format!("{setup}  jsr {name}\n"), text);
    let high = common::run_with(tag, &format!("{setup}  jsr {name}\n  txa\n"), text);
    ((high as u16) << 8) | (low as u16 & 0xFF)
}

/// A word shifted right by 8 or more moves one byte across (`shl 8` was the
/// only count tested): both right shifts, a count of exactly 8 and past it. The
/// arithmetic one fills the high byte with the sign (`ashr 9` through `ashr
/// 15`); that was seven `cmp #$80; ror a` for the sign alone.
#[test]
fn word_shifts_right_of_eight_or_more_move_one_byte() {
    for (op, count, f) in [
        ("lshr", 8u32, (|x: u16, k: u32| x >> k) as fn(u16, u32) -> u16),
        ("lshr", 13, |x, k| x >> k),
        ("ashr", 8, |x, k| ((x as i16) >> k) as u16),
        ("ashr", 12, |x, k| ((x as i16) >> k) as u16),
        ("ashr", 15, |x, k| ((x as i16) >> k) as u16),
    ] {
        let ir = format!("define i16 @sh(i16 %x) {{\n  %r = {op} i16 %x, {count}\n  ret i16 %r\n}}");
        let text = compile(&ir, "sh");
        assert!(!text.contains("rol"), "{op} {count}: {text}");
        // the sign of the high byte takes no `ror`; the low byte takes `count -
        // 8`, and is the sign too at 15
        let rors = text.matches("ror").count();
        assert_eq!(rors, if op == "ashr" && count != 15 { count as usize - 8 } else { 0 }, "{op} {count}: {text}");
        for x in [0x1234u16, 0x80FF, 0xFFFF, 0x0001, 0x7FFF] {
            assert_eq!(run_word("alloc-shn", &text, "sh", x), f(x, count), "{op} {count} of {x:#06x}");
        }
    }
}

/// Sign extension is the sign alone in the high byte: seven `cmp #$80; ror a`
/// (21 bytes, 28 cycles) for `sext i8 to i16`; an `ashr i8 x, 7` is the same
/// value.
#[test]
fn a_sign_extension_is_the_sign_splatted_not_seven_rotates() {
    let ir = "define i16 @ext(i8 %x) {\n  %r = sext i8 %x to i16\n  ret i16 %r\n}\n\
              define i8 @sign(i8 %x) {\n  %r = ashr i8 %x, 7\n  ret i8 %r\n}";
    let ext = compile(ir, "ext");
    assert!(!ext.contains("ror"), "{ext}");
    for x in [0u16, 1, 127, 128, 200, 255] {
        let want = x as u8 as i8 as i16 as u16;
        let setup = set("rc2", x);
        let low = common::run_with("alloc-ext", &format!("{setup}  jsr ext\n"), &ext);
        let high = common::run_with("alloc-ext", &format!("{setup}  jsr ext\n  txa\n"), &ext);
        assert_eq!(((high as u16) << 8) | (low as u16 & 0xFF), want, "sext {x}");
    }
    let sign = compile(ir, "sign");
    assert!(!sign.contains("ror"), "{sign}");
    for x in [0u16, 1, 127, 128, 255] {
        let got = common::run_with("alloc-sign", &format!("{}  jsr sign\n", set("rc2", x)), &sign);
        assert_eq!(got, if x >= 128 { 255 } else { 0 }, "ashr {x}, 7");
    }
}

/// `x >= 0` on a word reads the sign of the high byte alone, like `x < 0` (PL
/// beside MI).
#[test]
fn a_word_non_negative_test_reads_only_the_high_byte() {
    let ir = "define i8 @pos(i16 %x) {\n  %c = icmp sge i16 %x, 0\n  %r = select i1 %c, i8 7, i8 9\n  ret i8 %r\n}";
    let text = compile(ir, "pos");
    assert!(!text.contains("sbc"), "{text}");
    let run =
        |x: u16| common::run_with("alloc-pos", &format!("{}  jsr pos\n", set("rc2", x) + &set("rc3", x >> 8)), &text);
    assert_eq!((run(0xFFFF), run(0x8000), run(0x7FFF), run(0), run(5)), (9, 9, 7, 7, 7));
}

/// A branch on `c == 3 || c == 0` is two compare-and-branches (the shape of
/// `schr`, with the operands the other way round and no loop): no `ora` of two
/// 0/1 bytes.
#[test]
fn a_branch_on_an_or_of_two_compares_is_two_branches() {
    let ir = "define i8 @either(i8 %c) {\nentry:\n  %a = icmp eq i8 %c, 3\n  %b = icmp eq i8 %c, 0\n  %o = or i1 %a, %b\n  \
              br i1 %o, label %yes, label %no\nyes:\n  ret i8 1\nno:\n  ret i8 2\n}";
    let text = compile(ir, "either");
    assert!(!text.contains("  ora rc"), "{text}");
    let run = |c: u16| common::run_with("alloc-either", &format!("{}  jsr either\n", set("rc2", c)), &text);
    assert_eq!((run(3), run(0), run(1), run(255)), (1, 1, 2, 2));
}

/// No `pha`/`pla` round a move when a call is in the function: two calls with
/// values live across them (the digits kernel had one call in a loop).
#[test]
fn nothing_is_pushed_around_moves_between_two_calls() {
    let ir = "define i16 @two(i16 %a, i16 %b) {\n  %q = udiv i16 %a, 10\n  %r = urem i16 %b, 7\n  %s = add i16 %q, %r\n  \
              %t = add i16 %s, %a\n  ret i16 %t\n}";
    let text = compile(ir, "two");
    let saves = (8..16).filter(|n| text.contains(&format!("  lda rc{n}\n  pha\n"))).count();
    assert_eq!(text.matches("  pha\n").count(), saves, "{text}");
}

/// Constants 100 and 7 (not the 10 and 0 of digits) are loaded where read, not
/// stored to a slot.
#[test]
fn constants_of_a_division_are_loaded_again_not_stored() {
    let ir = "define i16 @cent(i16 %v) {\n  %q = udiv i16 %v, 100\n  %r = urem i16 %v, 7\n  %s = add i16 %q, %r\n  ret i16 %s\n}";
    let text = compile(ir, "cent");
    assert!(!text.contains("ldx #100\n  stx s_") && !text.contains("ldx #7\n  stx s_"), "{text}");
    for v in [0u16, 99, 100, 12345, 65535] {
        assert_eq!(run_word("alloc-cent", &text, "cent", v), (v / 100).wrapping_add(v % 7), "v={v}");
    }
}

/// A word `==` or `!=` against a constant loaded the constant into two
/// zero-page cells for the compare (`lda #0; sta rcN` twice before every `x ==
/// 0`: 75 bytes of lz4). The constant is two immediates, and zero is `lda lo;
/// ora hi`.
#[test]
fn a_word_equality_against_a_constant_has_no_constant_cells() {
    for (op, k) in [("eq", 0u16), ("ne", 0), ("eq", 300), ("ne", 0x2C00)] {
        let ir = format!(
            "define i8 @cmpk(i16 %x) {{\n  %c = icmp {op} i16 %x, {k}\n  %r = select i1 %c, i8 7, i8 9\n  ret i8 %r\n}}"
        );
        let text = compile(&ir, "cmpk");
        assert!(!text.contains("lda #0\n  sta rc") && !text.contains("sta rc4"), "{op} {k}: {text}");
        if k == 0 {
            assert!(text.contains("ora rc3"), "{op} {k}: {text}");
        }
        for x in [0u16, 1, 0x0100, 300, 0x002C, 0x012D, 0x2C00, 0x2C01, 0xFFFF] {
            let run =
                common::run_with("alloc-cmpk", &format!("{}  jsr cmpk\n", set("rc2", x) + &set("rc3", x >> 8)), &text);
            let equal = x == k;
            assert_eq!(run, if (op == "eq") == equal { 7 } else { 9 }, "{op} {k} against {x:#06x}");
        }
    }
}

/// Saving a callee-saved cell costs a save and a restore (ten bytes), so it
/// goes to a value that is used in a loop and not to one read once after it:
/// `i`, `n` and `hot` live across a call in the loop, `cold` across all of it.
/// The choice was all or none for the function, so `cold` took a fourth cell;
/// it is a spill slot.
#[test]
fn a_callee_saved_cell_goes_to_the_loop_values_only() {
    let ir = "declare i8 @ext(i8)\n\
              define i8 @mix(i8 %cold, i8 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i8 [ 0, %entry ], [ %i2, %loop ]\n  \
              %hot = phi i8 [ 0, %entry ], [ %hot2, %loop ]\n  %r = call i8 @ext(i8 %i)\n  %hot2 = add i8 %hot, %r\n  %i2 = add i8 %i, 1\n  \
              %more = icmp ult i8 %i2, %n\n  br i1 %more, label %loop, label %done\ndone:\n  %out = add i8 %hot2, %cold\n  ret i8 %out\n}";
    let text = compile(ir, "mix");
    let saved = (8..16).filter(|n| text.contains(&format!("  lda rc{n}\n  pha"))).count();
    assert_eq!(saved, 3, "{text}");
    assert!(text.contains("  lda rc2\n  sta s_mix"), "cold is in a slot: {text}");
}

/// `shl i16 x, k` for k from 9 to 15 ran k - 8 asl/rol pairs over the whole
/// word and then moved the low byte up (the high byte of every `rol` was dead).
/// The result's high byte is the low byte shifted by k - 8: no `rol`.
#[test]
fn a_word_shift_left_of_nine_or_more_shifts_one_byte() {
    for count in [9u32, 11, 15] {
        let ir = format!("define i16 @shl(i16 %x) {{\n  %r = shl i16 %x, {count}\n  ret i16 %r\n}}");
        let text = compile(&ir, "shl");
        assert!(!text.contains("rol"), "shl {count}: {text}");
        for x in [0x1234u16, 0x80FF, 0xFFFF, 0x0001] {
            assert_eq!(run_word("alloc-shl", &text, "shl", x), x << count, "shl {count} of {x:#06x}");
        }
    }
}

/// `q = p + 1; x = *p`: the read of the old pointer follows the bump in the IR,
/// so `p` stayed live across it and the bump took a copy of the pair (`lda rc2;
/// sta rc6; lda rc3; sta rc7`, then `inc rc6`: 170 bytes of lz4). The read goes
/// first and the bump is done in `p`'s own cells. A load and a store through
/// the old pointer, in a loop.
#[test]
fn a_read_of_the_old_pointer_goes_before_its_bump() {
    for (name, body, result) in [
        ("rd", "%v = load volatile i8, ptr %q\n  %s2 = add i8 %s, %v\n", "%s2"),
        ("wr", "store volatile i8 %i, ptr %q\n  %s2 = add i8 %s, %i\n", "%s2"),
    ] {
        let ir = format!(
            "define i8 @{name}(ptr %p, i8 %n) {{\nentry:\n  br label %loop\nloop:\n  %q = phi ptr [ %p, %entry ], [ %q2, %loop ]\n  \
             %i = phi i8 [ 0, %entry ], [ %i2, %loop ]\n  %s = phi i8 [ 0, %entry ], [ %s2, %loop ]\n  %q2 = getelementptr i8, ptr %q, i16 1\n  \
             {body}  %i2 = add i8 %i, 1\n  %more = icmp ult i8 %i2, %n\n  br i1 %more, label %loop, label %done\ndone:\n  ret i8 {result}\n}}"
        );
        let text = compile(&ir, name);
        // the cell that is bumped is the one the pointer was read through
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        let bumped = (0..lines.len() - 1)
            .find_map(|i| lines[i].strip_prefix("inc rc").filter(|_| lines[i + 1].starts_with("bne")))
            .expect("a pointer bump (inc, then bne for the carry)");
        assert!(text.contains(&format!("(rc{bumped})")), "{name}: {text}");
    }
}

/// A leaf saves its callee-saved cells on the hardware stack and pops them
/// before `rts`; the result waiting in `a` (and `x`) must come through the
/// pops, and the caller's cells must be as they were. A byte result (the quad
/// test), a word result in `a` and `x`, and no result at all.
#[test]
fn cells_saved_on_the_stack_leave_the_result_and_the_callers_cells_alone() {
    let data = ".segment \"DATA\"\ntab: .byte 3,1,4,1,5,9,2,6,5,3,5,8,9,7,9,3\nout: .res 1\n.segment \"CODE\"\n";
    let keep: [u8; 8] = [0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0x17, 0x28];
    let mut set = String::new();
    let mut check = String::from("  lda #0\n  sta rc2\n");
    for (i, v) in keep.iter().enumerate() {
        set += &format!("  lda #{v}\n  sta rc{}\n", 8 + i);
        check += &format!("  lda rc{}\n  eor #{v}\n  ora rc2\n  sta rc2\n", 8 + i);
    }
    check += "  lda rc2\n";
    let args = "  lda #<tab\n  sta rc2\n  lda #>tab\n  sta rc3\n  lda #4\n  sta rc4\n";
    let base = quad_ir("quad", false);
    let word = base
        .replace("define i8 @quad(", "define i16 @quad(")
        .replacen(
            "  ret i8 %r\n}",
            "  %w = zext i8 %r to i16\n  %h = shl i16 %w, 8\n  %o = or i16 %h, %w\n  ret i16 %o\n}",
            1,
        );
    let void = base
        .replace("define i8 @quad(", "define void @quad(ptr %o, ")
        .replacen("  ret i8 %r\n}", "  store i8 %r, ptr %o\n  ret void\n}", 1);
    // the word: 58 in both halves
    let text = compile(&word, "quad");
    assert!(text.contains("  pla\n") && text.contains("  tay\n"), "pops with a result live:\n{text}");
    let program = format!("{text}{data}");
    let low = common::run_with("alloc-pushed-lo", &format!("{set}{args}  jsr quad\n"), &program);
    let high = common::run_with("alloc-pushed-hi", &format!("{set}{args}  jsr quad\n  txa\n"), &program);
    assert_eq!((low, high), (58, 58));
    let kept = common::run_with("alloc-pushed-kept", &format!("{set}{args}  jsr quad\n{check}"), &program);
    assert_eq!(kept, 0, "the word version changed a callee-saved cell");
    // no result: the output byte arrives through a pointer in rc2:rc3, the
    // table in rc4:rc5, the count in rc6
    let text = compile(&void, "quad");
    assert!(!text.contains("  tay\n  pla"), "no result to keep:\n{text}");
    let program = format!("{text}{data}");
    let args = "  lda #<out\n  sta rc2\n  lda #>out\n  sta rc3\n  lda #<tab\n  sta rc4\n  lda #>tab\n  sta rc5\n  lda #4\n  sta rc6\n";
    let result = common::run_with("alloc-pushed-void", &format!("{set}{args}  jsr quad\n  lda out\n"), &program);
    assert_eq!(result, 58);
    let kept = common::run_with("alloc-pushed-void-kept", &format!("{set}{args}  jsr quad\n{check}"), &program);
    assert_eq!(kept, 0, "the void version changed a callee-saved cell");
}

/// A zero or sign test of a value just loaded or computed in `a` compared it
/// with 0 again (`lda rc2; cmp #0; beq`, 2 bytes and 2 cycles each, popcount):
/// the load set the flags. Tests of a byte and of the high byte of a word,
/// equal, not equal and negative, each branching on a value that reached `a` a
/// different way.
#[test]
fn a_zero_or_sign_test_does_not_compare_again() {
    for (name, ir) in [
        ("isz", "define i8 @isz(i8 %x) {\n  %c = icmp eq i8 %x, 0\n  %r = select i1 %c, i8 7, i8 9\n  ret i8 %r\n}"),
        ("nz", "define i8 @nz(i8 %x) {\n  %c = icmp ne i8 %x, 0\n  %r = select i1 %c, i8 7, i8 9\n  ret i8 %r\n}"),
        ("neg", "define i8 @neg(i8 %x) {\n  %c = icmp slt i8 %x, 0\n  %r = select i1 %c, i8 7, i8 9\n  ret i8 %r\n}"),
        (
            "sum",
            "define i8 @sum(i8 %x, i8 %y) {\n  %s = add i8 %x, %y\n  %c = icmp eq i8 %s, 0\n  %r = select i1 %c, i8 7, i8 9\n  ret i8 %r\n}",
        ),
    ] {
        let text = compile(ir, name);
        assert!(!text.contains("cmp #0"), "{name}: {text}");
        let run = |x: u16, y: u16| {
            common::run_with("alloc-nzt", &format!("{}{}  jsr {name}\n", set("rc2", x), set("rc3", y)), &text)
        };
        let want = |x: u16, y: u16| -> i32 {
            let (x, y) = (x as u8, y as u8);
            let yes = match name {
                "isz" => x == 0,
                "nz" => x != 0,
                "neg" => (x as i8) < 0,
                _ => x.wrapping_add(y) == 0,
            };
            if yes { 7 } else { 9 }
        };
        for (x, y) in [(0, 0), (1, 0), (0x80, 0), (0x7F, 1), (0xFF, 1), (0x80, 0x80)] {
            assert_eq!(run(x, y), want(x, y), "{name}({x:#x}, {y:#x})");
        }
    }
}

/// `b * a` swapped the two words in place (`ldx rc2; ldy rc3; ...; stx rc4; sty
/// rc5`, 8 bytes) because the routine takes its first operand in rc2:rc3 and
/// `a` arrives there but was named second. A product goes either way round: the
/// call takes them as they arrive, whichever order the IR names, and with one
/// operand computed.
#[test]
fn a_product_takes_its_operands_as_they_arrive() {
    let product = |first: &str, second: &str| {
        format!("define i16 @mulk(i16 %a, i16 %b) {{\n  %r = mul i16 {first}, {second}\n  ret i16 %r\n}}")
    };
    let straight = compile(&product("%a", "%b"), "mulk");
    let swapped = compile(&product("%b", "%a"), "mulk");
    assert_eq!(straight, swapped);
    assert!(!swapped.contains("stx rc4"), "{swapped}");
    // a computed operand and an argument: (a + 1) * b either way
    let sum = |first: &str, second: &str| {
        format!(
            "define i16 @mulk(i16 %a, i16 %b) {{\n  %s = add i16 %a, 1\n  %r = mul i16 {first}, {second}\n  ret i16 %r\n}}"
        )
    };
    for (first, second) in [("%s", "%b"), ("%b", "%s")] {
        let text = compile(&sum(first, second), "mulk");
        for (a, b, want) in [(5u16, 7u16, 42u16), (299, 200, 60000), (0xFFFF, 9, 0)] {
            let setup = set("rc2", a) + &set("rc3", a >> 8) + &set("rc4", b) + &set("rc5", b >> 8);
            let low = common::run_with("alloc-mulk", &format!("{setup}  jsr mulk\n"), &text);
            let high = common::run_with("alloc-mulk", &format!("{setup}  jsr mulk\n  txa\n"), &text);
            assert_eq!(((high as u16) << 8) | (low as u16 & 0xFF), want, "({first}) * ({second}) at a={a} b={b}");
        }
    }
}

/// A value kept in a callee-saved pair across a call was copied there from
/// `rc2:rc3` and straight back into `rc2:rc3` for the call (`lda rc2; sta rc8;
/// ...; lda rc8; sta rc2`: 8 bytes a trip in digits). The copy back changes
/// nothing. A loop dividing by 7 and counting until the value is small, and the
/// same with a remainder.
#[test]
fn a_copy_back_into_the_register_it_came_from_is_dropped() {
    for (name, op) in [("steps", "udiv"), ("rems", "urem")] {
        let ir = format!(
            "define i8 @{name}(i16 %v) {{\nentry:\n  br label %loop\nloop:\n  %x = phi i16 [ %v, %entry ], [ %q, %loop ]\n  \
             %n = phi i8 [ 0, %entry ], [ %n2, %loop ]\n  %q = {op} i16 %x, 7\n  %n2 = add i8 %n, 1\n  %more = icmp ugt i16 %x, 200\n  \
             br i1 %more, label %loop, label %done\ndone:\n  ret i8 %n2\n}}"
        );
        let text = compile(&ir, name);
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        for at in 0..lines.len().saturating_sub(1) {
            if let (Some(from), Some(to)) = (lines[at].strip_prefix("lda "), lines[at + 1].strip_prefix("sta ")) {
                let back = [format!("lda {to}"), format!("sta {from}")];
                let returned = (at + 2..(at + 10).min(lines.len().saturating_sub(1)))
                    .any(|i| lines[i] == back[0] && lines[i + 1] == back[1]);
                assert!(!returned || from.starts_with('#'), "{name}: {from} -> {to} and back:\n{text}");
            }
        }
    }
}

/// A walk that returns the pointer it stopped at (the old one, not the bumped
/// one) kept both pointers: the bump was made before the exit test, so the old
/// pointer lived on beside it and the loop copied back (slen 67 B against
/// cc65's 47). The bump sinks into the block that loops and is done in place. A
/// scan for a zero byte returning its address, and one returning the distance.
#[test]
fn a_bump_the_exit_does_not_need_is_made_only_on_the_looping_path() {
    let end = "define ptr @endp(ptr %p) {\nentry:\n  br label %loop\nloop:\n  %q = phi ptr [ %p, %entry ], [ %q2, %loop ]\n  \
               %c = load i8, ptr %q\n  %q2 = getelementptr i8, ptr %q, i16 1\n  %z = icmp eq i8 %c, 0\n  br i1 %z, label %done, label %loop\n\
               done:\n  ret ptr %q\n}";
    let dist = "define i16 @dist(ptr %p) {\nentry:\n  br label %loop\nloop:\n  %q = phi ptr [ %p, %entry ], [ %q2, %loop ]\n  \
                %c = load i8, ptr %q\n  %q2 = getelementptr i8, ptr %q, i16 1\n  %z = icmp eq i8 %c, 0\n  br i1 %z, label %done, label %loop\n\
                done:\n  %a = ptrtoint ptr %q to i16\n  %b = ptrtoint ptr %p to i16\n  %d = sub i16 %a, %b\n  ret i16 %d\n}";
    let data = ".segment \"DATA\"\ns: .byte \"hello world\",0\n.segment \"CODE\"\n";
    let setup = "  lda #<s\n  sta rc2\n  lda #>s\n  sta rc3\n";
    for (name, ir, tail) in [("endp", end, "  sec\n  sbc #<s\n"), ("dist", dist, "")] {
        let text = compile(ir, name);
        // the cell that is bumped is the one the byte was read through: no copy
        // of the pointer
        let bumped = text.lines().find_map(|l| l.trim().strip_prefix("inc rc")).map(str::to_owned).expect("an inc rc");
        assert!(text.contains(&format!("(rc{bumped}),y")) || text.contains(&format!("(rc{bumped})")), "{name}: {text}");
        let run = common::run_with(
            &format!("alloc-{name}"),
            &format!("{setup}  jsr {name}\n{tail}"),
            &format!("{text}{data}"),
        );
        assert_eq!(run, 11, "{name}");
    }
}

/// The sink moves an instruction into the one path that reads it, which can
/// cost a jump and a callee-saved cell on the path that stays hot: in the
/// two-exit loops of lz4 it made the function 13 bytes larger and 666 cycles
/// slower. The sunk form is kept only if it is no larger and no slower. lz4
/// (clang -O2 IR of `tests/bench/kernels.c`) stays at its 447 bytes and 37224
/// cycles of before; the walk above, where the sink pays, still sinks.
#[test]
fn the_sink_is_kept_only_where_it_is_no_larger_and_no_slower() {
    let ir = include_str!("bench/lz4.ll");
    let text = compile(ir, "lz4");
    let priced = llrm_6502::timing::estimate(Cpu::Mos6502, &text).expect("prices");
    assert!(priced.bytes <= 447, "lz4 is {} bytes:\n{text}", priced.bytes);
}

/// A byte select jumped over one of two loads (`bcc t; lda f; jmp d; t: lda t;
/// d:`, gcd). When the value to keep is the compare's left side it is already
/// in `a` and one load goes; on a carry condition the load leaves the carry
/// alone, so the value for a true condition goes in first and the branch skips
/// the other. Both ways, with a constant arm, on registers, and on an equality
/// (neither: it keeps the jump).
#[test]
fn a_byte_select_loads_once_when_a_holds_a_value_or_the_condition_is_the_carry() {
    let select = |name: &str, cmp: &str, t: &str, f: &str| {
        format!(
            "define i8 @{name}(i8 %x, i8 %y) {{\n  %c = icmp {cmp} i8 %x, %y\n  %r = select i1 %c, i8 {t}, i8 {f}\n  ret i8 %r\n}}"
        )
    };
    let cases: [(&str, String, bool, fn(u8, u8) -> u8); 5] = [
        ("keeplhs", select("keeplhs", "ult", "%x", "0"), true, |x, y| if x < y { x } else { 0 }),
        ("keepfalse", select("keepfalse", "ult", "0", "%x"), true, |x, y| if x < y { 0 } else { x }),
        ("carryk", select("carryk", "uge", "5", "9"), true, |x, y| if x >= y { 5 } else { 9 }),
        ("carryr", select("carryr", "ult", "%y", "%x"), true, |x, y| if x < y { y } else { x }),
        ("equal", select("equal", "eq", "%y", "3"), false, |x, y| if x == y { y } else { 3 }),
    ];
    for (name, ir, no_jump, f) in cases {
        let text = compile(&ir, name);
        if no_jump {
            assert!(!text.lines().any(|l| l.contains("jmp ") && l.contains("_cmd")), "{name}: {text}");
        }
        for (x, y) in [(0u8, 0u8), (1, 2), (2, 1), (200, 100), (100, 200), (255, 255), (7, 7)] {
            let main = format!("{}{}  jsr {name}\n", set("rc2", u16::from(x)), set("rc3", u16::from(y)));
            assert_eq!(
                common::run_with(&format!("alloc-{name}"), &main, &text),
                i32::from(f(x, y)),
                "{name}({x}, {y}):\n{text}"
            );
        }
    }
}

/// Two selects on one compare with a subtraction between them compared twice
/// (`lda b; cmp a` again, 4 bytes, in gcd's loop), because the subtraction sets
/// the carry. The second select now follows the first and reads its carry.
#[test]
fn two_selects_on_one_carry_compare_make_the_compare_once() {
    let ir = "define i8 @split(i8 %a, i8 %b) {
  %gt = icmp ugt i8 %a, %b
  %x = select i1 %gt, i8 %b, i8 0
  %p = sub i8 %a, %x
  %y = select i1 %gt, i8 0, i8 %a
  %q = sub i8 %b, %y
  %r = xor i8 %p, %q
  ret i8 %r
}";
    let text = compile(ir, "split");
    assert_eq!(text.matches("cmp ").count(), 1, "the compare is made twice:\n{text}");
    for (a, b) in [(5u8, 3u8), (3, 5), (4, 4), (200, 7), (0, 9), (255, 0)] {
        let (x, y) = if a > b { (b, 0) } else { (0, a) };
        let want = (a - x) ^ (b - y);
        let setup = set("rc2", a.into()) + &set("rc3", b.into());
        assert_eq!(
            common::run_with(&format!("alloc-split-{a}-{b}"), &format!("{setup}  jsr split\n"), &text),
            want.into()
        );
    }
}

/// Sibling: a select on a zero flag is not shared (its loads set the zero flag,
/// so a second select would read the wrong one); both selects still pick by the
/// one compare.
#[test]
fn two_selects_on_an_equality_still_pick_by_it() {
    let ir = "define i8 @same(i8 %a, i8 %b) {
  %eq = icmp eq i8 %a, %b
  %x = select i1 %eq, i8 7, i8 %b
  %p = sub i8 %a, %x
  %y = select i1 %eq, i8 %a, i8 9
  %q = add i8 %b, %y
  %r = xor i8 %p, %q
  ret i8 %r
}";
    let text = compile(ir, "same");
    for (a, b) in [(5u8, 3u8), (4, 4), (0, 0), (200, 7), (9, 9)] {
        let (x, y) = if a == b { (7u8, a) } else { (b, 9) };
        let want = a.wrapping_sub(x) ^ b.wrapping_add(y);
        let setup = set("rc2", a.into()) + &set("rc3", b.into());
        assert_eq!(
            common::run_with(&format!("alloc-same-{a}-{b}"), &format!("{setup}  jsr same\n"), &text),
            want.into()
        );
    }
}

/// A word times `2^n +- 1` (and the negative, and with factors of two) called
/// `__mulhi3`: hash16's `h * 33` was 17 bytes of setup and a routine of 300
/// cycles a byte. It is a shift and an add or subtract. The high byte of the
/// product is checked as well as the low.
#[test]
fn a_word_times_a_power_of_two_plus_or_minus_one_is_a_shift_and_add() {
    for (c, name) in [(33i32, "m33"), (15, "m15"), (-9, "mneg9"), (12, "m12"), (40, "m40")] {
        let ir = format!(
            "define i8 @{name}(i16 %x) {{\n  %p = mul i16 %x, {c}\n  %h = lshr i16 %p, 8\n  %t = trunc i16 %h to i8\n  ret i8 %t\n}}"
        );
        let text = compile(&ir, name);
        assert!(!text.contains("__mulhi3"), "{c}: a call to the multiply:\n{text}");
        for x in [0u16, 1, 7, 300, 1234, 65535] {
            let want = (x.wrapping_mul(c as u16) >> 8) as i32;
            let setup = set("rc2", x & 0xFF) + &set("rc3", x >> 8);
            assert_eq!(
                common::run_with(&format!("alloc-{name}-{x}"), &format!("{setup}  jsr {name}\n"), &text),
                want,
                "{x} * {c}"
            );
        }
    }
}

/// Sibling: a constant that is not of that form (100 = 0b1100100) still
/// multiplies, by the routine.
#[test]
fn a_word_times_another_constant_still_multiplies() {
    let ir = "define i8 @m100(i16 %x) {\n  %p = mul i16 %x, 100\n  %h = lshr i16 %p, 8\n  %t = trunc i16 %h to i8\n  ret i8 %t\n}";
    let text = compile(ir, "m100");
    assert!(text.contains("__mulhi3"), "{text}");
}

/// `x > C` and `x <= C` against a constant compared the constant first (`lda
/// #99; ldx #0; cmp lo; txa; sbc hi`, 11 bytes in digits' loop test). They are
/// `x >= C + 1` and `x < C + 1` with the constant an immediate (10 bytes), and
/// the edges where `C + 1` wraps keep their meaning.
#[test]
fn a_greater_than_a_constant_compares_against_the_next_constant() {
    let cases: [(&str, &str, i32); 8] = [
        ("ugt", "i16", 99),
        ("ule", "i16", 99),
        ("sgt", "i16", -2),
        ("sle", "i16", 7),
        ("ugt", "i16", 65535),
        ("sgt", "i16", 32767),
        ("ugt", "i8", 254),
        ("sle", "i8", -1),
    ];
    for (n, (pred, ty, c)) in cases.into_iter().enumerate() {
        let bits = if ty == "i16" { 16 } else { 8 };
        let ir = format!(
            "define i8 @t{n}({ty} %x) {{\n  %c = icmp {pred} {ty} %x, {c}\n  %r = zext i1 %c to i8\n  ret i8 %r\n}}"
        );
        let text = compile(&ir, &format!("t{n}"));
        if (pred, c) == ("ugt", 99) {
            assert!(text.contains("cmp #100") && !text.contains("lda #99"), "{text}");
        }
        let wrap = |v: i32| (v & ((1 << bits) - 1)) as u32;
        let signed = |v: u32| if v >> (bits - 1) == 1 { v as i32 - (1 << bits) } else { v as i32 };
        for x in [0u32, 1, 7, 8, 98, 99, 100, 254, 255, 256, 32767, 32768, 65534, 65535] {
            let x = x & wrap(-1);
            let (sx, ux, sc, uc) = (signed(x), x, c, wrap(c));
            let truth = match pred {
                "ugt" => ux > uc,
                "ule" => ux <= uc,
                "sgt" => sx > sc,
                _ => sx <= sc,
            };
            let setup = set("rc2", (x & 0xFF) as u16) + &set("rc3", (x >> 8) as u16);
            let got = common::run_with(&format!("alloc-cc{n}-{x}"), &format!("{setup}  jsr t{n}\n"), &text);
            assert_eq!(got, i32::from(truth), "{pred} {ty} {x} vs {c}");
        }
    }
}

/// `select c, 1, 2` is `sext(c) + 2` to the combiner, and nothing selected a
/// sign extension from a bit: "cannot select SignExtend". Both orders of two
/// consecutive constants pick by `c`.
#[test]
fn a_select_between_two_consecutive_constants_picks_by_the_condition() {
    for (n, (yes, no)) in [(1u16, 2u16), (7, 8), (2, 1), (9, 8), (255, 0)].into_iter().enumerate() {
        let ir = format!(
            "define i8 @k{n}(i16 %x) {{\n  %c = icmp ult i16 %x, 99\n  %r = select i1 %c, i8 {yes}, i8 {no}\n  ret i8 %r\n}}"
        );
        let text = compile(&ir, &format!("k{n}"));
        for (x, want) in [(0u16, yes), (98, yes), (99, no), (1000, no)] {
            let setup = set("rc2", x & 0xFF) + &set("rc3", x >> 8);
            let got = common::run_with(&format!("alloc-k{n}-{x}"), &format!("{setup}  jsr k{n}\n"), &text);
            assert_eq!(got, i32::from(want), "x = {x}, {yes} or {no}");
        }
    }
}

/// Ten sums are live at once. With 16 bytes of imaginary registers the
/// allocator spills one (and saves a kept cell on the stack); with the 26 of a
/// platform that has the room, they fit in scratch cells that need no save, and
/// the result is the same: 4.
#[test]
fn a_platform_with_more_zero_page_keeps_more_values_in_scratch_cells() {
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
    let (narrow, wide) = (compile_on(&ir, "many", 16), compile_on(&ir, "many", 26));
    assert!(narrow.contains("s_many_0") || narrow.contains("pha"), "no pressure at 16:\n{narrow}");
    assert!(!wide.contains("s_many_") && !wide.contains("pha"), "the wider platform still spills or saves:\n{wide}");
    let setup: String = (1..=6).map(|n| set(&format!("rc{}", n + 1), n)).collect();
    assert_eq!(common::run_with("alloc-many-wide", &format!("{setup}  jsr many\n"), &wide), 4);
}

/// Scratch cells are caller-saved: loop values live across a call stay out of
/// them, because the callee may use every cell but `rc8`..`rc15`. The callee
/// here fills rc2..rc7 and rc16..rc25 and returns 3; four calls add up to 12.
#[test]
fn loop_values_live_across_a_call_are_not_left_in_a_scratch_cell() {
    let ir = "declare i8 @clob(i8)
define i8 @sum(i8 %n) {
entry:
  br label %loop
loop:
  %i = phi i8 [ 0, %entry ], [ %i2, %loop ]
  %acc = phi i8 [ 0, %entry ], [ %acc2, %loop ]
  %r = call i8 @clob(i8 %i)
  %acc2 = add i8 %acc, %r
  %i2 = add i8 %i, 1
  %c = icmp ult i8 %i2, %n
  br i1 %c, label %loop, label %done
done:
  ret i8 %acc2
}";
    let text = compile_on(ir, "sum", 26);
    let fill: String = (2..8).chain(16..26).map(|n| format!("  sta rc{n}\n")).collect();
    let callee = format!("clob:\n  lda #$EE\n{fill}  lda #3\n  rts\n");
    let setup = set("rc2", 4);
    assert_eq!(
        common::run_with("alloc-sum-wide", &format!("{setup}  jsr sum\n"), &format!("{text}{callee}.export clob\n")),
        12
    );
}

/// A word shifted by n bits is n `asl`/`rol` pairs (4 bytes each, 20 for the
/// shift by 5 in hash16) or, with an index register free, a counted loop of 9
/// bytes that takes 5 more clocks a bit. The level decides: at size (12 clocks
/// a byte) every shift of 3 to 7 bits is the loop; at speed (1 clock a
/// byte) none is. Both bytes of each result are checked, the shifted word added
/// to the word so that the shift stays in cells.
#[test]
fn a_word_shift_is_a_counted_loop_at_the_size_level_and_unrolled_at_speed() {
    for (op, k) in [("shl", 3u32), ("shl", 4), ("shl", 5), ("shl", 7), ("lshr", 3), ("lshr", 6)] {
        let name = format!("{op}{k}");
        let ir =
            format!("define i16 @{name}(i16 %x) {{\n  %p = {op} i16 %x, {k}\n  %q = add i16 %p, %x\n  ret i16 %q\n}}");
        let looped = compile_at(&ir, &name, 16, Tuning::SIZE);
        let unrolled = compile_at(&ir, &name, 16, Tuning::SPEED);
        assert!(looped.contains("dex") || looped.contains("dey"), "{op} {k} at size:\n{looped}");
        assert!(!unrolled.contains("dex") && !unrolled.contains("dey"), "{op} {k} at speed:\n{unrolled}");
        for text in [&looped, &unrolled] {
            for x in [0u16, 1, 0x00FF, 0x0100, 0x1234, 0xABCD, 0xFFFF] {
                let shifted = if op == "shl" { x << k } else { x >> k };
                let want = shifted.wrapping_add(x);
                let setup = set("rc2", x & 0xFF) + &set("rc3", x >> 8);
                let run = |after: &str| {
                    common::run_with(&format!("alloc-{name}-{x}"), &format!("{setup}  jsr {name}\n{after}"), text)
                };
                assert_eq!(run(""), i32::from(want & 0xFF), "{op} {k} of {x:#x}, low byte");
                assert_eq!(run("  txa\n"), i32::from(want >> 8), "{op} {k} of {x:#x}, high byte");
            }
        }
    }
}

/// Three callee-saved cells cost 18 bytes of `lda`/`pha` and `pla`/`sta`. At
/// the size level they are saved by `__csave` and restored by a jump to
/// `__crest` (11 bytes, 99 clocks more); at speed they stay inline. digits(5) =
/// 1, digits(10) = 2, digits(99) = 2, digits(100) = 3, digits(65535) = 5.
#[test]
fn three_saved_cells_are_shared_helpers_at_the_size_level_only() {
    let ir = "define i8 @digits(i16 %v) {
entry:
  %c = icmp ugt i16 %v, 9
  br i1 %c, label %loop, label %done
loop:
  %n = phi i8 [ 1, %entry ], [ %n2, %loop ]
  %x = phi i16 [ %v, %entry ], [ %q, %loop ]
  %q = udiv i16 %x, 10
  %n2 = add i8 %n, 1
  %g = icmp ugt i16 %x, 99
  br i1 %g, label %loop, label %out
out:
  ret i8 %n2
done:
  ret i8 1
}";
    let shared = compile_at(ir, "digits", 16, Tuning::SIZE);
    let inline = compile_at(ir, "digits", 16, Tuning::SPEED);
    assert!(shared.contains("jsr __csave") && shared.contains("jmp __crest") && !shared.contains("pha"), "{shared}");
    assert!(inline.contains("pha") && !inline.contains("__csave"), "{inline}");
    for (v, want) in [(5u16, 1), (10, 2), (99, 2), (100, 3), (65535, 5)] {
        for (name, text) in [("shared", &shared), ("inline", &inline)] {
            let setup =
                set("rc2", v & 0xFF) + &set("rc3", v >> 8) + &set("rc8", 0xA1) + &set("rc9", 0xB2) + &set("rc10", 0xC3);
            // the callee-saved cells come back as they were
            let after = "  lda rc8\n  cmp #$A1\n  bne bad\n  lda rc9\n  cmp #$B2\n  bne bad\n  lda rc10\n  cmp #$C3\n  bne bad\n  pla\n  rts\nbad:\n  lda #99\n  rts\n";
            let program = format!("{setup}  jsr digits\n  pha\n{}", after);
            assert_eq!(
                common::run_with(&format!("alloc-digits-{name}-{v}"), &program, text),
                want,
                "{name} digits({v})"
            );
        }
    }
}

/// `__csave` and `__crest` link as 34 bytes, so a module takes them only where
/// its users together save more than that: one function that keeps three cells
/// saved 9 bytes and cost 25 net (digits: 165 linked at speed, 189 at size with
/// the helpers). Three such functions gain 27, four gain 36.
#[test]
fn the_save_helpers_are_linked_only_where_their_users_pay_for_them() {
    let digits = |name: &str| {
        format!(
            "define i8 @{name}(i16 %v) {{
entry:
  %c = icmp ugt i16 %v, 9
  br i1 %c, label %loop, label %done
loop:
  %n = phi i8 [ 1, %entry ], [ %n2, %loop ]
  %x = phi i16 [ %v, %entry ], [ %q, %loop ]
  %q = udiv i16 %x, 10
  %n2 = add i8 %n, 1
  %g = icmp ugt i16 %x, 99
  br i1 %g, label %loop, label %out
out:
  ret i8 %n2
done:
  ret i8 1
}}\n"
        )
    };
    let module_of = |users: usize| {
        let names: Vec<String> = (0..users).map(|n| format!("dig{n}")).collect();
        let ir: String = names.iter().map(|n| digits(n)).collect();
        let text = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
        let mut module = llrm_mir::parse::module(&text).expect("parses");
        llrm_6502::prepare::split_phi_edges(&mut module);
        let selections: Vec<_> = names
            .iter()
            .map(|n| {
                let mut selection = select_function(&module, n, Cpu::Mos6502).expect("selects");
                selection.tuning = Tuning::SIZE;
                selection
            })
            .collect();
        let parts: Vec<(&str, &llrm_6502::dagisel::function::Selection)> =
            names.iter().map(String::as_str).zip(&selections).collect();
        (names.clone(), llrm_6502::select::module_text(&module, &parts).expect("emits"))
    };
    for (users, shared) in [(1, false), (3, false), (4, true)] {
        let (names, texts) = module_of(users);
        let all: String = texts.concat();
        assert_eq!(all.contains("jsr __csave"), shared, "{users} users:\n{all}");
        if !shared {
            assert!(all.contains("pha") && !all.contains("__crest"), "{users} users:\n{all}");
        }
        for (v, want) in [(5u16, 1), (100, 3), (65535, 5)] {
            for name in &names {
                let setup = set("rc2", v & 0xFF)
                    + &set("rc3", v >> 8)
                    + &set("rc8", 0xA1)
                    + &set("rc9", 0xB2)
                    + &set("rc10", 0xC3);
                let after = "  lda rc8\n  cmp #$A1\n  bne bad\n  lda rc9\n  cmp #$B2\n  bne bad\n  lda rc10\n  cmp #$C3\n  bne bad\n  pla\n  rts\nbad:\n  lda #99\n  rts\n";
                let program = format!("{setup}  jsr {name}\n  pha\n{after}");
                assert_eq!(
                    common::run_with(&format!("alloc-{users}-{name}-{v}"), &program, &all),
                    want,
                    "{users} users, {name}({v})"
                );
            }
        }
    }
}

/// A rotate (`(x << 1) | (x >> 15)` is `rotl` to opt) was "cannot select Rotl":
/// found by the differential tests, where every program folds its checksum that
/// way. By constants and by a count.
#[test]
fn a_rotate_is_two_shifts_and_an_or() {
    let ir = "declare i16 @llvm.fshl.i16(i16, i16, i16)
declare i8 @llvm.fshl.i8(i8, i8, i8)
define i16 @rot1(i16 %x) {
  %r = call i16 @llvm.fshl.i16(i16 %x, i16 %x, i16 1)
  ret i16 %r
}
define i16 @rotv(i16 %x, i8 %n) {
  %c = zext i8 %n to i16
  %r = call i16 @llvm.fshl.i16(i16 %x, i16 %x, i16 %c)
  ret i16 %r
}
define i8 @rot3(i8 %x) {
  %r = call i8 @llvm.fshl.i8(i8 %x, i8 %x, i8 3)
  ret i8 %r
}";
    let rotl16 = |x: u16, n: u32| x.rotate_left(n);
    let word = |name: &str, x: u16, n: u8| {
        let text = compile(ir, name);
        let setup = set("rc2", x & 0xFF) + &set("rc3", x >> 8) + &set("rc4", u16::from(n));
        let run = |after: &str| {
            common::run_with(&format!("alloc-{name}-{x}-{n}"), &format!("{setup}  jsr {name}\n{after}"), &text)
        };
        (run(""), run("  txa\n"))
    };
    for x in [0u16, 1, 0x8000, 0x1234, 0xFFFF] {
        let want = rotl16(x, 1);
        assert_eq!(word("rot1", x, 0), (i32::from(want & 0xFF), i32::from(want >> 8)), "rotl {x:#x} by 1");
        for n in [0u8, 1, 5, 15, 16, 21] {
            let want = rotl16(x, u32::from(n) % 16);
            assert_eq!(word("rotv", x, n), (i32::from(want & 0xFF), i32::from(want >> 8)), "rotl {x:#x} by {n}");
        }
    }
    let text = compile(ir, "rot3");
    for x in [0u16, 1, 0x81, 0xFF, 0x5A] {
        let want = (x as u8).rotate_left(3);
        assert_eq!(
            common::run_with(&format!("alloc-rot3-{x}"), &format!("{}  jsr rot3\n", set("rc2", x)), &text),
            i32::from(want)
        );
    }
}

/// `llvm.lifetime.start` on a local array reached selection as a node nothing
/// handled: "LifetimeStart is not a machine node" (differential tests, every
/// program with a local array at -O2).
#[test]
fn lifetime_markers_on_a_local_array_are_ignored() {
    let ir = "declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)
define i8 @arr(i8 %x) {
  %a = alloca [4 x i8]
  call void @llvm.lifetime.start.p0(i64 4, ptr %a)
  store i8 %x, ptr %a
  %p = getelementptr i8, ptr %a, i16 1
  store i8 7, ptr %p
  %u = load i8, ptr %a
  %w = load i8, ptr %p
  %s = add i8 %u, %w
  call void @llvm.lifetime.end.p0(i64 4, ptr %a)
  ret i8 %s
}";
    let text = compile(ir, "arr");
    assert_eq!(common::run_with("alloc-arr", &format!("{}  jsr arr\n", set("rc2", 5)), &text), 12);
}

/// The labels the emitter numbers inside a function (`_cmt1`, `_cmd1`, `_pa1`)
/// were the same in every function, so two functions in one file defined them
/// twice (differential tests: "Symbol '_far1' is already defined"). Each
/// function names its own: max of 3 and 9, then of 4 and 2, from one file.
#[test]
fn two_functions_in_one_file_do_not_share_internal_labels() {
    let one =
        "define i8 @maxa(i8 %a, i8 %b) {\n  %c = icmp ugt i8 %a, %b\n  %r = select i1 %c, i8 %a, i8 %b\n  ret i8 %r\n}";
    let two =
        "define i8 @maxb(i8 %a, i8 %b) {\n  %c = icmp ugt i8 %a, %b\n  %r = select i1 %c, i8 %a, i8 %b\n  ret i8 %r\n}";
    let text = compile(one, "maxa") + &compile(two, "maxb");
    let main = format!(
        "{}{}  jsr maxa\n  pha\n{}{}  jsr maxb\n  sta rc2\n  pla\n  clc\n  adc rc2\n",
        set("rc2", 3),
        set("rc3", 9),
        set("rc2", 4),
        set("rc3", 2)
    );
    assert_eq!(common::run_with("alloc-two-files", &main, &text), 13);
}
/// A select on a compare of two bits looped the scheduler forever (a `select`
/// on itself, as its zero-extended condition) and found by the differential
/// tests. Bits compare as bytes of 0 or 1.
#[test]
fn a_select_on_a_compare_of_bits_compiles_and_runs() {
    let ir = "define i8 @eqz(i8 %a, i8 %e) {
  %d = and i8 %a, 1
  %c = icmp eq i8 %d, 0
  %h = select i1 %c, i8 230, i8 %e
  ret i8 %h
}
define i8 @slt(i8 %a, i8 %e) {
  %x = trunc i8 %a to i1
  %y = trunc i8 %e to i1
  %c = icmp slt i1 %x, %y
  %h = select i1 %c, i8 7, i8 9
  ret i8 %h
}";
    let run = |name: &str, a: u16, e: u16| {
        let text = compile_within_ten_seconds(ir, name);
        let setup = set("rc2", a) + &set("rc3", e);
        common::run_with(&format!("alloc-bits-{name}-{a}-{e}"), &format!("{setup}  jsr {name}\n"), &text)
    };
    for (a, e) in [(0u16, 77u16), (1, 77), (2, 5), (3, 5)] {
        assert_eq!(run("eqz", a, e), i32::from(if a & 1 == 0 { 230 } else { e }), "eqz {a} {e}");
    }
    // a set bit is -1: signed, 1 < 0 and 0 > 1
    for (a, e) in [(0u16, 0u16), (1, 0), (0, 1), (1, 1)] {
        assert_eq!(run("slt", a, e), if a == 1 && e == 0 { 7 } else { 9 }, "slt {a} {e}");
    }
}

/// A select between two bits, itself a select's condition, looped the scheduler
/// forever: its zero-extension legalized to a select on the same bit (found by
/// the differential tests, seed 0).
#[test]
fn a_select_of_bits_as_a_condition_compiles_and_runs() {
    let ir = "define i8 @bitsel(i8 %a, i8 %b, i8 %c) {
  %x = trunc i8 %a to i1
  %y = icmp eq i8 %b, 0
  %z = trunc i8 %c to i1
  %s = select i1 %x, i1 %y, i1 %z
  %r = select i1 %s, i8 3, i8 4
  ret i8 %r
}";
    let text = compile_within_ten_seconds(ir, "bitsel");
    for (a, b, c) in [(0u16, 0u16, 0u16), (0, 0, 1), (1, 0, 0), (1, 1, 1), (1, 1, 0), (0, 1, 1)] {
        let setup = set("rc2", a) + &set("rc3", b) + &set("rc4", c);
        let want = if (if a & 1 == 1 { b == 0 } else { c & 1 == 1 }) { 3 } else { 4 };
        assert_eq!(
            common::run_with(&format!("alloc-bitsel-{a}{b}{c}"), &format!("{setup}  jsr bitsel\n"), &text),
            want,
            "{a} {b} {c}"
        );
    }
}

/// A bit that is a truncation from a word, or frozen, as a select's condition
/// looped the scheduler forever like the bit selects above (differential seeds
/// 4, 21).
#[test]
fn a_truncated_or_frozen_bit_as_a_condition_compiles_and_runs() {
    let ir = "define i8 @tword(i16 %w) {
  %b = trunc i16 %w to i1
  %r = select i1 %b, i8 3, i8 4
  ret i8 %r
}
define i8 @tfrozen(i8 %a) {
  %b = trunc i8 %a to i1
  %f = freeze i1 %b
  %r = select i1 %f, i8 3, i8 4
  ret i8 %r
}";
    for (name, args) in [("tword", [0x0100u16, 0]), ("tword", [0x0001, 0]), ("tfrozen", [2, 0]), ("tfrozen", [3, 0])] {
        let text = compile_within_ten_seconds(ir, name);
        let want = if args[0] & 1 == 1 { 3 } else { 4 };
        let setup = set("rc2", args[0] & 0xFF) + &set("rc3", args[0] >> 8);
        assert_eq!(
            common::run_with(&format!("alloc-trunc-{name}-{}", args[0]), &format!("{setup}  jsr {name}\n"), &text),
            want,
            "{name} {:#x}",
            args[0]
        );
    }
}

/// A sign-extended byte read through a pointer was `CannotSelect(Load)`
/// (differential seeds 9-11): the combiner makes an extending load of it, which
/// the legalizer turns back into a load and an extension. With a constant
/// operation after the extension the combine runs again.
#[test]
fn a_sign_extended_load_is_a_load_and_an_extension() {
    let ir = "define i16 @sload(ptr %p) {
  %b = load i8, ptr %p
  %s = sext i8 %b to i16
  ret i16 %s
}
define i16 @sloadx(ptr %p) {
  %b = load i8, ptr %p
  %s = sext i8 %b to i16
  %x = xor i16 %s, 1456
  ret i16 %x
}";
    for name in ["sload", "sloadx"] {
        let text = compile_within_ten_seconds(ir, name);
        for byte in [0u16, 5, 0x7F, 0x80, 0xFF] {
            let setup = format!("{}{}  lda #{byte}\n  sta $0300\n", set("rc2", 0), set("rc3", 3));
            let extended = byte as u8 as i8 as i16 as u16;
            let want = i32::from(if name == "sloadx" { extended ^ 1456 } else { extended });
            let low = common::run_with(&format!("alloc-{name}-{byte}"), &format!("{setup}  jsr {name}\n"), &text);
            let high =
                common::run_with(&format!("alloc-{name}-h-{byte}"), &format!("{setup}  jsr {name}\n  txa\n"), &text);
            assert_eq!((high << 8) | low, want, "{name} {byte:#x}");
        }
    }
}

/// A constant used where a compare's flags are live was loaded again between
/// the compare and its `CMOV16`, and the load reset Z: `bne` never branched,
/// and a 0/1 flag was always 1 (differential seed 36: 255 for 127).
#[test]
fn a_constant_is_not_loaded_again_between_a_compare_and_its_select() {
    let ir = "declare i16 @llvm.fshl.i16(i16, i16, i16)
@buf = external global [16 x i8]
define i16 @remat() {
entry:
  %a = alloca [8 x i32], align 2
  br label %fill
fill:
  %i = phi i8 [ 0, %entry ], [ %i1, %fill ]
  %iz = zext i8 %i to i16
  %p = getelementptr inbounds [8 x i32], ptr %a, i16 0, i16 %iz
  store i32 1, ptr %p, align 2
  %i1 = add i8 %i, 1
  %fd = icmp eq i8 %i1, 8
  br i1 %fd, label %mid, label %fill
mid:
  %b = load i8, ptr @buf, align 1
  %z = icmp eq i8 %b, 0
  %a0 = load i32, ptr %a, align 2
  %a0t = trunc i32 %a0 to i16
  %o = or i16 %a0t, 1
  %m = urem i16 239, %o
  %zx = zext i1 %z to i32
  store i32 %zx, ptr %a, align 2
  br label %sum
sum:
  %j = phi i8 [ 0, %mid ], [ %j1, %sum ]
  %ck = phi i16 [ %m, %mid ], [ %ck1, %sum ]
  %jz = zext i8 %j to i16
  %r = call i16 @llvm.fshl.i16(i16 %ck, i16 %ck, i16 1)
  %q = getelementptr inbounds [8 x i32], ptr %a, i16 0, i16 %jz
  %l = load i32, ptr %q, align 2
  %lt = trunc i32 %l to i16
  %ck1 = xor i16 %r, %lt
  %j1 = add i8 %j, 1
  %sd = icmp eq i8 %j1, 8
  br i1 %sd, label %done, label %sum
done:
  ret i16 %ck1
}";
    let text = compile_within_ten_seconds(ir, "remat");
    // a0 is 1 when buf[0] is 0 and 0 otherwise; the sum folds a0 and seven ones
    for (first, want) in [(3, 127), (0, 255)] {
        let data = format!(".export buf\nbuf: .byte {first},1,4,1,5,9,2,6,5,3,5,8,9,7,9,3\n");
        let got = common::run_with(&format!("alloc-remat-{first}"), "  jsr remat\n", &format!("{text}{data}"));
        assert_eq!(got, want, "buf[0] = {first}");
    }
}

/// The low byte of `c ^ (p << 8)` was `c.lo ^ 0`: a zero loaded, stored and
/// xored in (four instructions, seven bytes, in every crc16 inner loop).
#[test]
fn a_byte_op_with_its_identity_is_a_copy() {
    let ir = "@buf = external global [16 x i8]
define i8 @idt() {
  %c = load i16, ptr @buf, align 1
  %b = load i8, ptr getelementptr inbounds nuw (i8, ptr @buf, i16 2), align 1
  %z = zext i8 %b to i16
  %s = shl i16 %z, 8
  %x = xor i16 %c, %s
  %h = lshr i16 %x, 8
  %t = trunc i16 %h to i8
  ret i8 %t
}";
    let text = compile_within_ten_seconds(ir, "idt");
    let function = text.split("idt:").nth(1).expect("the function is emitted").split(".segment").next().unwrap();
    assert_eq!(function.matches("eor").count(), 1, "only the high byte is xored:\n{function}");
    let data = ".export buf\nbuf: .byte 9,12,10\n";
    let got = common::run_with("alloc-identity", "  jsr idt\n", &format!("{text}{data}"));
    assert_eq!(got, 12 ^ 10);
}

/// The same with the zero first, `(p << 8) ^ c`: crc16 wrote it that way (99
/// bytes, 6442 cycles; 92 and 6369 now).
#[test]
fn a_byte_op_with_its_identity_on_the_left_is_a_copy() {
    let ir = "@buf = external global [16 x i8]
define i8 @idt() {
  %c = load i16, ptr @buf, align 1
  %b = load i8, ptr getelementptr inbounds nuw (i8, ptr @buf, i16 2), align 1
  %z = zext i8 %b to i16
  %s = shl i16 %z, 8
  %x = xor i16 %s, %c
  %h = lshr i16 %x, 8
  %t = trunc i16 %h to i8
  ret i8 %t
}";
    let text = compile_within_ten_seconds(ir, "idt");
    let function = text.split("idt:").nth(1).expect("the function is emitted").split(".segment").next().unwrap();
    assert_eq!(function.matches("eor").count(), 1, "only the high byte is xored:\n{function}");
    let data = ".export buf\nbuf: .byte 9,12,10\n";
    let got = common::run_with("alloc-identity-left", "  jsr idt\n", &format!("{text}{data}"));
    assert_eq!(got, 12 ^ 10);
}

/// A register made by spill code could not be spilled again and was refused the
/// only colours that fit (callee-saved cells, not "worth" saving for its
/// weight): "no register for v217 ... and nothing to spill" (differential seed
/// 127, once an unrelated change moved the pressure; needs the 26 cells of
/// sim65).
#[test]
fn a_spill_temporary_takes_a_colour_that_costs_a_save() {
    let ir = "define fastcc i16 @spt(i32 %0, i8 %1, i32 %2, i16 %3, ptr %4) {
  br label %6
6:
  br label %6
7:
  %8 = icmp ne i32 %0, 1
  %9 = load i32, ptr %4, align 2
  %10 = icmp ult i32 %2, 1
  %11 = select i1 %10, i16 1, i16 %3
  store i32 %0, ptr null, align 2
  %12 = icmp eq i8 0, %1
  %13 = zext i1 %12 to i32
  br label %14
14:
  %p = phi i16 [ %11, %7 ], [ 0, %14 ]
  br label %14
15:
  %16 = xor i32 0, %13
  ret i16 0
}";
    let text = compile_on(ir, "spt", 26);
    assert!(text.contains("spt:"), "{text}");
}

/// `(x & 1) == 0 ? 0 : x` combines to a sign extension from one bit, which had
/// no action at `i1` ("CannotSelect(SignExtendInReg)": differential seed 123
/// once the programs started from opaque values).
#[test]
fn a_select_on_the_low_bit_compiles_and_runs() {
    let ir = "@buf = external global [16 x i8]
define i16 @lowbit() {
  %x = load i8, ptr @buf, align 1
  %m = and i8 %x, 1
  %c = icmp eq i8 %m, 0
  %r = select i1 %c, i8 0, i8 %x
  %w = zext i8 %r to i16
  ret i16 %w
}";
    let text = compile_within_ten_seconds(ir, "lowbit");
    for (first, want) in [(3, 3), (4, 0), (255, 255)] {
        let data = format!(".export buf\nbuf: .byte {first},1,4,1,5,9,2,6,5,3,5,8,9,7,9,3\n");
        let got = common::run_with(&format!("alloc-lowbit-{first}"), "  jsr lowbit\n", &format!("{text}{data}"));
        assert_eq!(got, want, "buf[0] = {first}");
    }
}

/// A byte load widened to 32 bits was formed as one extending load of a type
/// that is not a register, and came out of type legalization as a `Truncate` of
/// it that nothing selects ("CannotSelect(Truncate)": differential seed 209).
#[test]
fn a_byte_load_used_at_32_bits_and_truncated_compiles_and_runs() {
    let ir = "@buf = external global [16 x i8]
define i16 @wide() {
  %slot = alloca i32, align 2
  %a = load i8, ptr @buf, align 1
  %z = zext i8 %a to i32
  %m = and i32 %z, 1
  store i32 %m, ptr %slot, align 2
  %b = load i8, ptr getelementptr inbounds nuw (i8, ptr @buf, i16 1), align 1
  %e = icmp eq i8 %a, %b
  %ez = zext i1 %e to i32
  %x = or i32 %ez, %z
  %t = trunc i32 %x to i16
  ret i16 %t
}";
    let text = compile_within_ten_seconds(ir, "wide");
    for (first, second, want) in [(3, 3, 3), (4, 4, 5), (4, 5, 4)] {
        let data = format!(".export buf\nbuf: .byte {first},{second},4,1,5,9,2,6,5,3,5,8,9,7,9,3\n");
        let got = common::run_with(&format!("alloc-wide-{first}-{second}"), "  jsr wide\n", &format!("{text}{data}"));
        assert_eq!(got, want, "buf = {first}, {second}");
    }
}

/// An undefined value reached selection as `IMPLICIT_DEF` ("no description of
/// opcode 10": differential seed 198); until the MIR has `undef` it reads as
/// zero.
#[test]
fn an_undefined_value_is_stored_as_zero() {
    let ir = "define i16 @undef() {
  %slot = alloca i32, align 2
  store i32 poison, ptr %slot, align 2
  %v = load i32, ptr %slot, align 2
  %t = trunc i32 %v to i16
  %r = add i16 %t, 7
  ret i16 %r
}";
    let text = compile_within_ten_seconds(ir, "undef");
    let got = common::run_with("alloc-undef", "  jsr undef\n", &text);
    assert_eq!(got, 7);
}

/// `sta m; lda m` reloaded what the register held (97 times in a
/// 3,100-instruction generated function, differential seed 55), unless a branch
/// reads the flags the load set.
#[test]
fn a_reload_of_the_cell_just_stored_goes_unless_a_branch_reads_its_flags() {
    use llrm_6502::alloc::drop_reloads;
    assert_eq!(
        drop_reloads("  lda rc4\n  sta rc2\n  lda rc2\n  ora rc3\n  rts\n"),
        "  lda rc4\n  sta rc2\n  ora rc3\n  rts\n"
    );
    assert_eq!(drop_reloads("  stx s_f_3\n  ldx s_f_3\n  inx\n"), "  stx s_f_3\n  inx\n");
    let branch = "  sta rc2\n  lda rc2\n  bne l\n";
    assert_eq!(drop_reloads(branch), branch, "bne reads the zero flag the load set");
    let other = "  sta rc2\n  lda rc3\n  ldx rc2\n  sta (rc2),y\n  lda (rc2),y\n  sty rc2\n  lda rc2\n  rts\n";
    assert_eq!(drop_reloads(other), other, "only the same register and cell");
}

/// A spilled byte used as the second operand of an ALU op is read from its cell
/// (`eor s_f_3`). Slot sharing took that read plus the store of its result for
/// a copy of one slot into the other and gave them one cell: 184 of 240
/// differential programs miscompiled while the fold was being written.
#[test]
fn spilled_bytes_read_by_alu_ops_keep_their_own_cells() {
    const N: usize = 16;
    let mut ir = String::from("@buf = external global [16 x i8]\ndefine i8 @fold() {\n");
    for i in 0..N {
        ir +=
            &format!("  %p{i} = getelementptr inbounds i8, ptr @buf, i16 {i}\n  %b{i} = load i8, ptr %p{i}, align 1\n");
    }
    for i in 0..N / 2 {
        ir += &format!("  %x{i} = xor i8 %b{i}, %b{}\n  %m{i} = add i8 %x{i}, {}\n", N - 1 - i, 7 * i + 3);
    }
    let mut cur = "%m0".to_owned();
    for i in 1..N / 2 {
        ir += &format!("  %s{i} = add i8 {cur}, %m{i}\n  %c{i} = and i8 %s{i}, {}\n", 255 - i);
        cur = format!("%c{i}");
    }
    ir += &format!("  ret i8 {cur}\n}}");
    let want = |buf: &[u8; 16]| {
        let m = |i: usize| (buf[i] ^ buf[N - 1 - i]).wrapping_add((7 * i + 3) as u8);
        (1..N / 2).fold(m(0), |cur, i| cur.wrapping_add(m(i)) & (255 - i) as u8)
    };
    let text = compile_on(&ir, "fold", 16);
    for (k, buf) in
        [[3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3], [200, 17, 99, 250, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]]
            .iter()
            .enumerate()
    {
        let data = format!(".export buf\nbuf: .byte {}\n", buf.iter().map(u8::to_string).collect::<Vec<_>>().join(","));
        let got = common::run_with(&format!("alloc-fold-{k}"), "  jsr fold\n", &format!("{text}{data}"));
        assert_eq!(got, i32::from(want(buf)), "buf {k}");
    }
    assert!(text.contains("s_fold_"), "the test needs spills:\n{text}");
    let folded = |l: &&str| {
        ["eor", "and", "adc", "sbc", "ora", "cmp"].iter().any(|m| l.trim_start().starts_with(&format!("{m} s_fold_")))
    };
    assert!(text.lines().filter(folded).count() > 0, "a spilled operand is read from its cell:\n{text}");
}

/// A byte shifted by 7 either way was seven one-bit shifts (1,692 runs of them
/// in 100 differential programs: the `ck >> 15` of every checksum step); one
/// bit is left, and four bytes make it.
#[test]
fn a_byte_shifted_by_seven_is_four_bytes() {
    let ir = "@buf = external global [16 x i8]
define i8 @right() {
  %b = load i8, ptr @buf, align 1
  %r = lshr i8 %b, 7
  ret i8 %r
}
define i8 @left() {
  %b = load i8, ptr @buf, align 1
  %r = shl i8 %b, 7
  ret i8 %r
}";
    let text = compile_within_ten_seconds(ir, "right") + &compile_within_ten_seconds(ir, "left");
    assert!(!text.contains("lsr a\n  lsr a"), "{text}");
    assert!(!text.contains("asl a\n  asl a"), "{text}");
    for (first, right, left) in [(0x80, 1, 0), (0x01, 0, 0x80), (0xFF, 1, 0x80), (0x7E, 0, 0)] {
        let data = format!(".export buf\nbuf: .byte {first},1,4,1,5,9,2,6,5,3,5,8,9,7,9,3\n");
        for (name, want) in [("right", right), ("left", left)] {
            let got = common::run_with(
                &format!("alloc-shift7-{name}-{first}"),
                &format!("  jsr {name}\n"),
                &format!("{text}{data}"),
            );
            assert_eq!(got, want, "{name} of {first:#x}");
        }
    }
}

/// Spills that are not live together share a cell (`StackSlotColoring`): six
/// rounds of the same pressure took six times the cells of one, 15,973 cells in
/// 98 differential programs (3,158 with it).
#[test]
fn spills_that_are_not_live_together_share_cells() {
    let rounds = |groups: usize| {
        let mut ir = String::from("@g = external global i8\ndefine i8 @fold() {\n  %acc0 = add i8 0, 0\n");
        for g in 0..groups {
            for i in 0..16 {
                ir += &format!("  %b{g}_{i} = load volatile i8, ptr @g\n");
            }
            for i in 0..8 {
                ir += &format!("  %x{g}_{i} = xor i8 %b{g}_{i}, %b{g}_{}\n", 15 - i);
            }
            let mut cur = format!("%acc{g}");
            for i in 0..8 {
                ir += &format!("  %s{g}_{i} = add i8 {cur}, %x{g}_{i}\n");
                cur = format!("%s{g}_{i}");
            }
            ir += &format!("  %acc{} = add i8 {cur}, 0\n", g + 1);
        }
        ir += &format!("  ret i8 %acc{groups}\n}}");
        let text = compile_on(&ir, "fold", 16);
        text.lines().filter(|l| l.starts_with("s_fold_") && l.contains(".res")).count()
    };
    let (one, six) = (rounds(1), rounds(6));
    assert!(one > 0, "the test needs spills");
    assert!(six <= one + 2, "six rounds took {six} cells, one took {one}");
}

/// `freeze` selects to the generic `COPY`, which the selector had no
/// description of ("no description of opcode 20": differential seeds 21 and 26
/// were rejected for it).
#[test]
fn a_frozen_value_is_a_copy() {
    let ir = "@buf = external global [16 x i8]
define i16 @frz() {
  %a = load i8, ptr getelementptr inbounds nuw (i8, ptr @buf, i16 13), align 1
  %f = freeze i8 %a
  %c = icmp ne i8 %f, -1
  %z = zext i1 %c to i16
  %q = getelementptr inbounds nuw [16 x i8], ptr @buf, i16 0, i16 %z
  %r = load i8, ptr %q, align 1
  %w = zext i8 %r to i16
  ret i16 %w
}";
    let text = compile_within_ten_seconds(ir, "frz");
    // buf[13] is 255: the load is buf[0]; otherwise buf[1]
    for (last, want) in [(255u16, 3), (7, 1)] {
        let data = format!(".export buf\nbuf: .byte 3,1,4,1,5,9,2,6,5,3,5,8,9,{last},9,3\n");
        let got = common::run_with(&format!("alloc-frz-{last}"), "  jsr frz\n", &format!("{text}{data}"));
        assert_eq!(got, want, "buf[13] = {last}");
    }
}

/// A 32-bit phi whose join block comes before the blocks that feed it: the copy
/// of its second word named a register nothing had yet ("phi input is never
/// named": differential seed 28).
#[test]
fn a_wide_phi_read_before_its_inputs_are_laid_out() {
    let ir = "define i32 @wphi(i32 %n) {
entry:
  br label %head
head:
  %acc = phi i32 [ %n, %entry ], [ %p, %join ]
  %i = phi i8 [ 0, %entry ], [ %i1, %join ]
  %c = icmp eq i8 %i, 0
  br i1 %c, label %a, label %b
join:
  %p = phi i32 [ %x, %a ], [ %y, %b ]
  %i1 = add i8 %i, 1
  %d = icmp eq i8 %i1, 3
  br i1 %d, label %exit, label %head
a:
  %x = xor i32 %acc, 5
  br label %join
b:
  %y = add i32 %acc, 65537
  br label %join
exit:
  ret i32 %p
}";
    let text = compile_within_ten_seconds(ir, "wphi");
    // n ^ 5, then + 0x10001 twice
    for n in [0u32, 0x1234_5678, 0xFFFF_FFFF] {
        let want = ((n ^ 5).wrapping_add(2 * 65537)) as u16;
        let byte = |k: u32| ((n >> (8 * k)) & 0xFF) as u16;
        let setup =
            format!("{}{}{}{}", set("rc2", byte(0)), set("rc3", byte(1)), set("rc4", byte(2)), set("rc5", byte(3)));
        let lo = common::run_with(&format!("alloc-wphi-{n}"), &format!("{setup}  jsr wphi\n"), &text);
        let hi = common::run_with(&format!("alloc-wphih-{n}"), &format!("{setup}  jsr wphi\n  txa\n"), &text);
        assert_eq!(((hi << 8) | lo) as u16, want, "n = {n:#x}");
    }
}

/// Compile on a thread and fail after ten seconds: a scheduler or allocator
/// loop with no progress bound shows as a failure here instead of a stuck run.
fn compile_within_ten_seconds(
    ir: &str,
    name: &str,
) -> String {
    let (ir, name) = (ir.to_owned(), name.to_owned());
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || send.send(compile(&ir, &name)).ok());
    receive.recv_timeout(std::time::Duration::from_secs(10)).expect("compiles within ten seconds")
}

/// The call frame's size was read from the wrong operand of `ADJCALLSTACKDOWN`
/// (the bytes popped, always 0), so a frame with locals had no room for the
/// arguments that do not fit in `rc2..rc7`: the call wrote them over the first
/// local (differential seed 2).
#[test]
fn outgoing_stack_arguments_do_not_overwrite_locals() {
    let ir = "define i16 @callee(i32 %a, i32 %b) {
  %h = lshr i32 %b, 16
  %t = trunc i32 %h to i16
  ret i16 %t
}
define i16 @callee3(i32 %a, i32 %b, i32 %c) {
  %x = xor i32 %b, %c
  %h = lshr i32 %x, 16
  %t = trunc i32 %h to i16
  ret i16 %t
}
define i16 @caller() {
  %l = alloca [4 x i16], align 2
  store i16 4660, ptr %l, align 2
  %r = call i16 @callee(i32 1, i32 1450704896)
  %v = load i16, ptr %l, align 2
  %x = xor i16 %v, %r
  ret i16 %x
}
define i16 @caller3() {
  %l = alloca [4 x i16], align 2
  store i16 4660, ptr %l, align 2
  %r = call i16 @callee3(i32 1, i32 1450704896, i32 305397760)
  %v = load i16, ptr %l, align 2
  %x = xor i16 %v, %r
  ret i16 %x
}";
    let text = format!(
        "{}{}{}{}",
        compile(ir, "caller"),
        compile(ir, "callee"),
        compile(ir, "caller3"),
        compile(ir, "callee3")
    );
    let setup = "  lda #0\n  sta rc0\n  lda #$80\n  sta rc1\n";
    for (name, want) in
        [("caller", 4660u32 ^ (1450704896 >> 16)), ("caller3", 4660 ^ ((1450704896u32 ^ 305397760) >> 16))]
    {
        let lo = common::run_with(&format!("alloc-frame-{name}"), &format!("{setup}  jsr {name}\n"), &text);
        let hi = common::run_with(&format!("alloc-frame-h-{name}"), &format!("{setup}  jsr {name}\n  txa\n"), &text);
        assert_eq!(((hi << 8) | lo) as u32, want, "{name}");
    }
}
