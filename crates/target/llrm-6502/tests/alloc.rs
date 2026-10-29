//! The register allocator path of `select::function_text`, against the cell
//! path it replaces. Every expected value is worked out by hand.

mod common;

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8";

fn compile(
    ir: &str,
    name: &str,
) -> String {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let selection = select_function(&module, name, Cpu::Mos6502).expect("selects");
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
    let text = compile(ir, "across");
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
    let text = compile(ir, "keep");
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
    // unpack of a pointer into `a` and a cell returned `rc4` in `a` (slen looped
    // forever)
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
    let run = |after: &str| common::run_with("alloc-widen", &format!("{setup}  jsr widen\n{after}"), &format!("{text}{data}"));
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
    let run = |after: &str| common::run_with("alloc-crc16", &format!("{setup}  jsr crc16\n{after}"), &format!("{text}{data}"));
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
/// `(rc0),y`'s 255 is refused with its reason, not emitted with a wrapped offset.
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

/// `f(p, x)` calls the function `p` points to with `x`: an indirect call, which the 6502
/// has no instruction for, was refused ("an indirect call").
const CALL_THROUGH: &str = "define i8 @twice(i8 %x) {\n  %r = add i8 %x, %x\n  ret i8 %r\n}\n\
    define i8 @via(ptr %f, i8 %x) {\n  %r = call i8 %f(i8 %x)\n  %s = add i8 %r, 1\n  ret i8 %s\n}";

#[test]
fn an_indirect_call_enters_the_callee_and_returns() {
    let text = compile(CALL_THROUGH, "via") + &compile(CALL_THROUGH, "twice");
    // via(&twice, 20) = 41
    let setup = "  lda #<twice\n  sta rc2\n  lda #>twice\n  sta rc3\n  lda #20\n  sta rc4\n";
    assert_eq!(common::run_with("alloc-indirect", &format!("{setup}  jsr via\n"), &text), 41);
}

/// A function that calls itself through a pointer is as reentrant as one that calls itself by
/// name: its values live across the call must be on the soft stack, not in static slots.
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

/// A struct passed by value (`byval`) and one returned through `sret`, the way a C frontend lowers
/// them: `pass(5)` is 5 + 6, `take(5)` the same through `mk`.
#[test]
fn a_struct_passed_byval_and_returned_through_sret_round_trips() {
    for entry in ["pass", "take"] {
        let text = structs(entry);
        let run = common::run_with("alloc-struct", &format!("{}  jsr {entry}\n", set("rc2", 5) + &set("rc3", 0)), &text);
        assert_eq!(run, 11, "{entry}");
    }
}
