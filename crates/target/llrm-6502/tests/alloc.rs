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
