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
    let mut module = llrm_mir::parse::module(&text).expect("parses");
    llrm_6502::prepare::split_phi_edges(&mut module);
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
    // the result starts as one operand and the other is moved in on one path: no jump over arms
    assert!(!text.contains("jmp _cmd"), "{text}");
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

/// `x < 0` on a word did `cmp #0; sbc #0` over both bytes: the sign is the high byte alone.
#[test]
fn a_word_sign_test_reads_only_the_high_byte() {
    let ir = "define i8 @neg(i16 %x) {\n  %c = icmp slt i16 %x, 0\n  %r = select i1 %c, i8 7, i8 9\n  ret i8 %r\n}";
    let text = compile(ir, "neg");
    assert!(!text.contains("sbc"), "{text}");
    let run = |x: u16| common::run_with("alloc-neg", &format!("{}  jsr neg\n", set("rc2", x) + &set("rc3", x >> 8)), &text);
    assert_eq!((run(0xFFFF), run(0x8000), run(0x7FFF), run(0), run(5)), (7, 7, 9, 9, 9));
}

/// `(b << 8) ^ c` with `b` a widened byte (crc16's `c ^= p[i] << 8`) was eight `asl`/`rol` pairs
/// on a word: the byte is the high half.
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

/// A pointer walked through a loop (`p += 1`, `p += 3`) went UNPACK, a word add, PACK on every
/// trip: a dozen moves and a clobbered `a`. It is the low byte bumped and the high one on carry,
/// in the pair.
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
        assert!(text.contains(if k == 1 { "inc rc" } else { "bcc _pa" }), "k={k}: {text}");
        assert!(!text.contains("tay") && !text.contains("s_walk"), "k={k}: {text}");
        for (p, n) in [(0x0100u16, 4u16), (0x01FE, 2), (0x01FF, 3), (0x12FF, 1)] {
            let setup = set("rc2", p) + &set("rc3", p >> 8) + &set("rc4", n);
            let low = common::run_with("alloc-walk", &format!("{setup}  jsr walk\n"), &text);
            let high = common::run_with("alloc-walk", &format!("{setup}  jsr walk\n  txa\n"), &text);
            assert_eq!(((high as u16) << 8) | (low as u16 & 0xFF), p + k * n, "k={k} p={p:#06x} n={n}");
        }
    }
}

/// A branch on `c == 0 || c == k` was two 0/1 bytes through two selects, an `and` and a test
/// (schr, 20 bytes over): it is one compare and branch per test, no `and` of two bytes.
#[test]
fn a_branch_on_two_compares_is_two_branches() {
    let text = compile(SCHR, "schr");
    assert!(!text.contains("  and rc"), "{text}");
    let run = |c: u16| {
        // "ab\0" at the pointer in rc2:rc3 = data, c in rc4: returns the index of c, or -1 at the end
        let data = ".segment \"DATA\"\ns: .byte 97, 98, 0\n.segment \"CODE\"\n";
        let setup = "  lda #<s\n  sta rc2\n  lda #>s\n  sta rc3\n".to_owned() + &set("rc4", c);
        common::run_with("alloc-schr", &format!("{setup}  jsr schr\n"), &format!("{text}{data}"))
    };
    assert_eq!((run(97), run(98), run(99)), (0, 1, 255));
}

/// `p + zext(i)` kept a cell holding zero just for the high byte's `adc`: the zero is an immediate.
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

/// `p[i]` with `i` a byte did a word add of `p` and `i` into a scratch pair before the load; it is
/// `lda (p),y` with `i` in `y`.
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
    let run = |after: &str| common::run_with("alloc-index", &format!("{setup}  jsr bump\n{after}"), &format!("{text}{data}"));
    assert_eq!(run(""), 7);
    assert_eq!(run("  ldy #3\n  lda table,y\n"), 8);
}

/// The sum of a byte table (`s += p[i]`) took the word-add path for the address on every trip: the
/// widened load is the same indexed load.
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
    let run = |after: &str| common::run_with("alloc-at", &format!("{setup}  jsr sum\n{after}"), &format!("{text}{data}"));
    // 1 + 2 + 3 + 200 = 206
    assert_eq!((run(""), run("  txa\n")), (206, 0));
}

const DIGITS: &str = "define i8 @digits(i16 %v) {\nentry:\n  %big = icmp ugt i16 %v, 9\n  br i1 %big, label %loop, label %done\nloop:\n  \
    %n = phi i8 [ %n2, %loop ], [ 1, %entry ]\n  %x = phi i16 [ %q, %loop ], [ %v, %entry ]\n  %q = udiv i16 %x, 10\n  %n2 = add i8 %n, 1\n  \
    %more = icmp ugt i16 %x, 99\n  br i1 %more, label %loop, label %done\ndone:\n  %r = phi i8 [ 1, %entry ], [ %n2, %loop ]\n  ret i8 %r\n}";

/// The constants 10 and 0 of `v / 10` were stored to static slots and loaded back before the call
/// (`ldx #10; stx s_digits_5`): they are loaded again where they are read.
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
    assert!(!text.contains("  pha\n"), "{text}");
}

/// A loop whose latch comes first in the layout: the latch's phi is read only on the edge back to
/// the header, a block laid out right after it and before the body that writes the phi. It failed
/// with "phi input was never written" (lz4, set 3). `fwd8` carries a byte, `fwd16` a word.
fn forwarding(width: &str, extra: &str) -> String {
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
            assert_eq!(common::run_with(&format!("alloc-{name}-{arg}"), &format!("{setup}  jsr {name}\n"), &text), expected, "{name}({arg})");
        }
    }
}

/// Four pointers advance through a table by different strides, with a sum and a counter: more live
/// values than `rc2`..`rc7` hold. `quad` is a leaf (static save slots); `quadcall` ends in an indirect
/// call, so it runs on the soft stack. buf = 3 1 4 1 5 9 2 6 5 3 5 8 9 7 9 3; strides 1, 2, 3, 1 from
/// offsets 1, 2, 3, 4; four trips sum (a + b) + (c ^ d) to 58; `quadcall` doubles that, 116.
fn quad_ir(name: &str, call: bool) -> String {
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

/// Callee-saved zero-page bytes (`rc8`..`rc15`): a function that needs more than `rc2`..`rc7` uses
/// them, saves what it touches on entry and restores it before returning. Before, it spilled; the
/// caller's `rc8`..`rc15` must come back unchanged and the result be right, in a leaf and in a
/// function on the soft stack.
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
        let kept = common::run_with(&format!("alloc-{name}-kept"), &format!("{set}{args}  jsr {name}\n{check}"), &program);
        assert_eq!(kept, 0, "{name} changed a callee-saved cell");
    }
}

/// A value live across one call: a callee-saved cell costs a save and a restore (about eight bytes), a
/// slot a store and a load; with both allocations priced by the timing table the smaller is kept. A
/// loop that calls on every trip (digits) keeps the cell, which saves a reload per trip.
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
    assert!((8..16).any(|n| looped.contains(&format!("rc{n}"))), "a loop of calls keeps its values in a cell:\n{looped}");
}

/// The word result of `name` for a word argument in rc2:rc3 (low byte in A, high byte in X).
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

/// A word shifted right by 8 or more moves one byte across (`shl 8` was the only count tested):
/// both right shifts, a count of exactly 8 and past it. The arithmetic one fills the high byte with
/// the sign (`ashr 9` through `ashr 15`); that was seven `cmp #$80; ror a` for the sign alone.
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
        // the sign of the high byte takes no `ror`; the low byte takes `count - 8`, and is the sign too at 15
        let rors = text.matches("ror").count();
        assert_eq!(rors, if op == "ashr" && count != 15 { count as usize - 8 } else { 0 }, "{op} {count}: {text}");
        for x in [0x1234u16, 0x80FF, 0xFFFF, 0x0001, 0x7FFF] {
            assert_eq!(run_word("alloc-shn", &text, "sh", x), f(x, count), "{op} {count} of {x:#06x}");
        }
    }
}

/// Sign extension is the sign alone in the high byte: seven `cmp #$80; ror a` (21 bytes, 28
/// cycles) for `sext i8 to i16`; an `ashr i8 x, 7` is the same value.
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

/// `x >= 0` on a word reads the sign of the high byte alone, like `x < 0` (PL beside MI).
#[test]
fn a_word_non_negative_test_reads_only_the_high_byte() {
    let ir = "define i8 @pos(i16 %x) {\n  %c = icmp sge i16 %x, 0\n  %r = select i1 %c, i8 7, i8 9\n  ret i8 %r\n}";
    let text = compile(ir, "pos");
    assert!(!text.contains("sbc"), "{text}");
    let run = |x: u16| common::run_with("alloc-pos", &format!("{}  jsr pos\n", set("rc2", x) + &set("rc3", x >> 8)), &text);
    assert_eq!((run(0xFFFF), run(0x8000), run(0x7FFF), run(0), run(5)), (9, 9, 7, 7, 7));
}

/// A branch on `c == 3 || c == 0` is two compare-and-branches (the shape of `schr`, with the
/// operands the other way round and no loop): no `ora` of two 0/1 bytes.
#[test]
fn a_branch_on_an_or_of_two_compares_is_two_branches() {
    let ir = "define i8 @either(i8 %c) {\nentry:\n  %a = icmp eq i8 %c, 3\n  %b = icmp eq i8 %c, 0\n  %o = or i1 %a, %b\n  \
              br i1 %o, label %yes, label %no\nyes:\n  ret i8 1\nno:\n  ret i8 2\n}";
    let text = compile(ir, "either");
    assert!(!text.contains("  ora rc"), "{text}");
    let run = |c: u16| common::run_with("alloc-either", &format!("{}  jsr either\n", set("rc2", c)), &text);
    assert_eq!((run(3), run(0), run(1), run(255)), (1, 1, 2, 2));
}

/// No `pha`/`pla` round a move when a call is in the function: two calls with values live across
/// them (the digits kernel had one call in a loop).
#[test]
fn nothing_is_pushed_around_moves_between_two_calls() {
    let ir = "define i16 @two(i16 %a, i16 %b) {\n  %q = udiv i16 %a, 10\n  %r = urem i16 %b, 7\n  %s = add i16 %q, %r\n  \
              %t = add i16 %s, %a\n  ret i16 %t\n}";
    let text = compile(ir, "two");
    assert!(!text.contains("  pha\n"), "{text}");
}

/// Constants 100 and 7 (not the 10 and 0 of digits) are loaded where read, not stored to a slot.
#[test]
fn constants_of_a_division_are_loaded_again_not_stored() {
    let ir = "define i16 @cent(i16 %v) {\n  %q = udiv i16 %v, 100\n  %r = urem i16 %v, 7\n  %s = add i16 %q, %r\n  ret i16 %s\n}";
    let text = compile(ir, "cent");
    assert!(!text.contains("ldx #100\n  stx s_") && !text.contains("ldx #7\n  stx s_"), "{text}");
    for v in [0u16, 99, 100, 12345, 65535] {
        assert_eq!(run_word("alloc-cent", &text, "cent", v), (v / 100).wrapping_add(v % 7), "v={v}");
    }
}

/// A word `==` or `!=` against a constant loaded the constant into two zero-page cells for the
/// compare (`lda #0; sta rcN` twice before every `x == 0`: 75 bytes of lz4). The constant is two
/// immediates, and zero is `lda lo; ora hi`.
#[test]
fn a_word_equality_against_a_constant_has_no_constant_cells() {
    for (op, k) in [("eq", 0u16), ("ne", 0), ("eq", 300), ("ne", 0x2C00)] {
        let ir = format!("define i8 @cmpk(i16 %x) {{\n  %c = icmp {op} i16 %x, {k}\n  %r = select i1 %c, i8 7, i8 9\n  ret i8 %r\n}}");
        let text = compile(&ir, "cmpk");
        assert!(!text.contains("lda #0\n  sta rc") && !text.contains("sta rc4"), "{op} {k}: {text}");
        if k == 0 {
            assert!(text.contains("ora rc3"), "{op} {k}: {text}");
        }
        for x in [0u16, 1, 0x0100, 300, 0x002C, 0x012D, 0x2C00, 0x2C01, 0xFFFF] {
            let run = common::run_with("alloc-cmpk", &format!("{}  jsr cmpk\n", set("rc2", x) + &set("rc3", x >> 8)), &text);
            let equal = x == k;
            assert_eq!(run, if (op == "eq") == equal { 7 } else { 9 }, "{op} {k} against {x:#06x}");
        }
    }
}
