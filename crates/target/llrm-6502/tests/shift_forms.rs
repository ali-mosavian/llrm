//! The form of a run of shifts is the one the cleaned text is shortest in,
//! with the block's own exit read as it is (2026-11-15).

mod common;

use std::collections::BTreeSet;

use common::pipeline::datalayout;
use llrm_6502::asm::{Asm, Op};
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::shift_forms::{CANDIDATE_LINES, Counter, Exit, Form, Site, choose};
use llrm_6502::timing::estimate;

/// `*p * 10` was `sta rc3; lda rc2; asl a; rol rc3; ldx rc3` for the last
/// shift (9 B) where the byte that leaves in `a` is held there: `asl rc2; rol
/// a; tax; lda rc2` (6 B). The region pass priced the shift with every cell
/// read at its end and kept the first (36 B against 33, 2026-11-15).
#[test]
fn a_byte_times_ten_holds_the_high_byte_in_a_for_the_last_shift() {
    let ir = format!(
        "target datalayout = \"{}\"\ndefine i16 @f(ptr %p) {{\n  %b = load i8, ptr %p\n  %z = zext i8 %b to i16\n  %m = mul i16 %z, 10\n  ret i16 %m\n}}\n",
        datalayout()
    );
    for level in [Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        let from = text.find("f:").unwrap_or_else(|| panic!("no `f:` in\n{text}"));
        let body: String = text[from..].split("  rts").next().unwrap().to_owned() + "  rts\n";
        let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
        assert!(bytes <= 33, "{level:?}: {bytes} B against 33\n{body}");
    }
}

/// The loop at lines 4..10 of `text`, as the emitter writes it.
fn site(
    run: usize,
    a_dead: bool,
) -> Site {
    Site {
        block: 0,
        start: 4,
        end: 10,
        first: Op::Asl,
        second: Op::Rol,
        one: "rc4".into(),
        two: "rc5".into(),
        run,
        counter: Some(Counter { load: Op::Ldx, step: Op::Dex, label: 1 }),
        a_dead,
        place: 0,
        base: Form::Loop,
    }
}

fn exit(
    asm: &Asm,
    cells: &[&str],
) -> Exit {
    Exit {
        terminators: asm.lines.len() - 1,
        end: asm.lines.len(),
        cells: cells.iter().map(|c| c.to_string()).collect::<BTreeSet<_>>(),
        a: false,
        x: false,
        y: false,
    }
}

const COPY: &str = "  lda rc2\n  sta rc4\n  lda rc3\n  sta rc5\n  ldx #3\n_sh1:\n  asl rc4\n  rol rc5\n  dex\n  bne _sh1\n  clc\n  lda rc4\n  adc rc2\n  tay\n  lda rc5\n  adc rc3\n  tax\n  tya\n  rts\n";

/// The copy and the reload go with the loop (the run, its byte in `a` or
/// made so by the passes after, is 26 B against 30).
#[test]
fn a_counted_shift_loop_with_a_copy_is_the_run_in_a() {
    let mut asm = Asm::parse(COPY);
    let exits = [exit(&asm, &[])];
    choose(&mut asm, &[site(3, true)], &exits);
    let text = asm.text();
    assert!(!text.contains("dex") && !text.contains("_sh1"), "{text}");
}

/// Sibling: where `a` is read after the run it cannot carry a byte through
/// it; the loop stays.
#[test]
fn a_counted_shift_loop_stays_where_a_is_live() {
    let before =
        "  lda rc7\n  sta rc2\n  sta rc3\n  ldx #3\n_sh1:\n  asl rc4\n  rol rc5\n  dex\n  bne _sh1\n  sta rc6\n  rts\n";
    let mut asm = Asm::parse(before);
    let mut live = site(3, false);
    (live.start, live.end) = (3, 9);
    let exits = [exit(&asm, &[])];
    choose(&mut asm, &[live], &exits);
    let text = asm.text();
    assert!(text.contains("_sh1") && text.contains("dex"), "{text}");
}

/// The byte the block's exit does not read need not be stored: with `rc5`
/// dead the high byte rides in `a` and its store and copy go (15 B), with it
/// read the low byte rides (17 B, a tie with the loop that the clocks break).
/// A stretch closed with every cell read priced both at 17.
#[test]
fn the_byte_the_exit_does_not_read_rides_in_a() {
    let before = "  lda rc2\n  sta rc4\n  lda rc3\n  sta rc5\n  ldx #3\n_sh1:\n  asl rc4\n  rol rc5\n  dex\n  bne _sh1\n  jmp out\n";
    let run = |cells: &[&str]| {
        let mut asm = Asm::parse(before);
        let exits = [Exit { terminators: 10, end: 11, ..exit(&asm, cells) }];
        choose(&mut asm, &[site(3, true)], &exits);
        asm.text()
    };
    let dead = run(&["rc4"]);
    assert!(dead.contains("rol a") && !dead.contains("_sh1"), "rc5 is dead after the block:\n{dead}");
    let read = run(&["rc4", "rc5"]);
    assert!(read.contains("asl a") && !read.contains("rol a"), "both cells are read:\n{read}");
}

/// Each candidate was built from the whole function and cleaned with whole-
/// function passes, so the choice cost grew with the square of the loops:
/// `select_of_constants` -Os (24k lines, ~400 loops) went from 8 s to over
/// 400 s (2026-11-08). Counted in candidate lines, not seconds: four times the
/// loops must cost about four times the work, not sixteen. Here the loops sit
/// in one block, so only the window keeps the stretches short.
#[test]
fn the_shift_form_choice_does_work_in_proportion_to_the_loops() {
    let work = |n: usize| {
        let mut text = String::new();
        let mut sites = Vec::new();
        let mut lines = 0;
        for i in 0..n {
            text += &format!(
                "  lda rc2\n  sta rc4\n  ldx #3\n_sh{i}:\n  asl rc4\n  rol rc5\n  dex\n  bne _sh{i}\n  lda rc4\n  sta rc6\n  lda rc5\n  sta rc7\n  nop\n"
            );
            let mut one = site(3, true);
            (one.start, one.end) = (lines + 2, lines + 8);
            one.counter = Some(Counter { load: Op::Ldx, step: Op::Dex, label: i });
            sites.push(one);
            lines += 13;
        }
        text += "  rts\n";
        let mut asm = Asm::parse(&text);
        assert_eq!(asm.lines.len(), lines + 1, "premise: 13 lines a loop");
        let exits = [Exit { terminators: lines, end: lines + 1, ..Exit::default() }];
        let before = CANDIDATE_LINES.with(|w| w.get());
        choose(&mut asm, &sites, &exits);
        CANDIDATE_LINES.with(|w| w.get()) - before
    };
    let (small, large) = (work(50), work(200));
    assert!(small > 0, "no candidate was built");
    assert!(large <= 6 * small, "200 loops built {large} candidate lines, 50 built {small}");
}

/// The exit code of `probe` run on sim65: the number of the first case that
/// is wrong, 0 when all agree.
fn exit_code_of(
    text: &str,
    name: &str,
) -> i32 {
    // the runtime defines the cells
    let text: String =
        text.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
    common::run_with(&format!("shift-forms-{name}"), "  jsr probe\n", &text)
}

/// `defs` (noinline functions `@f0`, `@f1`, ... returning i16) called as
/// `call` says on four inputs; the case that gives other than the value it
/// names is the exit code.
fn failing_case(
    name: &str,
    level: Level,
    defs: &str,
    call: impl Fn(usize, u8, u8, u16) -> (String, u16),
    functions: usize,
) -> i32 {
    let mut ir = format!("target datalayout = \"{}\"\n{defs}\n", datalayout());
    ir += "define i8 @probe() {\n";
    let mut previous = "0".to_owned();
    let mut case = 0;
    for function in 0..functions {
        for (c, d, x) in [(1u8, 2u8, 0x0123u16), (0xFF, 0x80, 0xA5C3), (0x40, 0x01, 0x8001), (0, 0, 0xFFFF)] {
            case += 1;
            let (arguments, want) = call(function, c, d, x);
            ir += &format!(
                "  %r{case} = call i16 @f{function}({arguments})\n  %ok{case} = icmp eq i16 %r{case}, {}\n  %a{case} = select i1 %ok{case}, i8 {previous}, i8 {case}\n",
                want as i16
            );
            previous = format!("%a{case}");
        }
    }
    ir += &format!("  ret i8 {previous}\n}}\n");
    let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
    exit_code_of(&text, name)
}

/// Four functions that hold a byte in `a` (`h`, `g`, `m`) or in `x` (`g2`)
/// across a shift of a word: the run in `a` would clobber the first, the loop
/// on `x` the second. The call argument, the sum and the compare read what
/// the shift must leave alone.
#[test]
fn a_shift_run_leaves_the_bytes_held_in_a_and_x_alone() {
    let defs = "\
declare i16 @sink(i8, i16) noinline
define i16 @sink(i8 %c, i16 %s) noinline {\n  %z = zext i8 %c to i16\n  %r = add i16 %s, %z\n  ret i16 %r\n}
define i16 @sink2(i8 %c, i8 %d, i16 %s) noinline {\n  %z = zext i8 %c to i16\n  %w = zext i8 %d to i16\n  %t = add i16 %s, %z\n  %r = xor i16 %t, %w\n  ret i16 %r\n}
define i16 @f0(i8 %c, i8 %d, i16 %x) noinline {\n  %s = shl i16 %x, 3\n  %r = call i16 @sink(i8 %c, i16 %s)\n  ret i16 %r\n}
define i16 @f1(i8 %c, i8 %d, i16 %x) noinline {\n  %s = shl i16 %x, 3\n  %z = zext i8 %c to i16\n  %r = add i16 %s, %z\n  ret i16 %r\n}
define i16 @f2(i8 %c, i8 %d, i16 %x) noinline {\n  %s = shl i16 %x, 3\n  %z = zext i8 %c to i16\n  %e = icmp eq i16 %s, %z\n  %r = select i1 %e, i16 7, i16 9\n  ret i16 %r\n}
define i16 @f3(i8 %c, i8 %d, i16 %x) noinline {\n  %s = shl i16 %x, 3\n  %r = call i16 @sink2(i8 %c, i8 %d, i16 %s)\n  ret i16 %r\n}
define i16 @f4(i8 %c, i8 %d, i16 %x) noinline {\n  %s = lshr i16 %x, 5\n  %r = call i16 @sink2(i8 %c, i8 %d, i16 %s)\n  ret i16 %r\n}
";
    let defs = defs.replace("declare i16 @sink(i8, i16) noinline\n", "");
    let call = |function: usize, c: u8, d: u8, x: u16| -> (String, u16) {
        let (c16, d16) = (u16::from(c), u16::from(d));
        let arguments = format!("i8 {}, i8 {}, i16 {}", c as i8, d as i8, x as i16);
        let want = match function {
            0 | 1 => (x << 3).wrapping_add(c16),
            2 => {
                if x << 3 == c16 {
                    7
                } else {
                    9
                }
            }
            3 => ((x << 3).wrapping_add(c16)) ^ d16,
            _ => ((x >> 5).wrapping_add(c16)) ^ d16,
        };
        (arguments, want)
    };
    for level in [Level::O2, Level::Os] {
        let failed = failing_case("held", level, &defs, call, 5);
        assert_eq!(failed, 0, "{level:?}: case {failed} (of 4 inputs a function) is wrong");
    }
}

/// A word that arrives in A:X is stored to its cells and its low byte stays in
/// `a`: the run in `a` is cheapest if the stored copy is read back, but `a`
/// is read after the run (`x * 9`, `x * 17`, `x * 5`, `(x << 3) ^ x`).
#[test]
fn a_shift_run_keeps_the_low_byte_the_word_arrived_with() {
    let defs = "\
define i16 @f0(i16 %x) noinline {\n  %s = shl i16 %x, 3\n  %r = add i16 %s, %x\n  ret i16 %r\n}
define i16 @f1(i16 %x) noinline {\n  %s = shl i16 %x, 4\n  %r = add i16 %s, %x\n  ret i16 %r\n}
define i16 @f2(i16 %x) noinline {\n  %s = shl i16 %x, 2\n  %r = add i16 %s, %x\n  ret i16 %r\n}
define i16 @f3(i16 %x) noinline {\n  %s = shl i16 %x, 3\n  %r = xor i16 %s, %x\n  ret i16 %r\n}
define i16 @f4(i16 %x) noinline {\n  %s = lshr i16 %x, 4\n  %r = add i16 %s, %x\n  ret i16 %r\n}
define i16 @f5(i16 %x) noinline {\n  %s = lshr i16 %x, 3\n  %r = xor i16 %s, %x\n  ret i16 %r\n}
";
    let call = |function: usize, _c: u8, _d: u8, x: u16| -> (String, u16) {
        let want = match function {
            0 => (x << 3).wrapping_add(x),
            1 => (x << 4).wrapping_add(x),
            2 => (x << 2).wrapping_add(x),
            3 => (x << 3) ^ x,
            4 => (x >> 4).wrapping_add(x),
            _ => (x >> 3) ^ x,
        };
        (format!("i16 {}", x as i16), want)
    };
    for level in [Level::O1, Level::O2, Level::Os] {
        let failed = failing_case("arrived", level, defs, call, 6);
        assert_eq!(failed, 0, "{level:?}: case {failed} (of 4 inputs a function) is wrong");
    }
}

/// The byte in `a` is what the compare after the shift reads: with the run
/// in `a` the compare saw the shifted byte (2026-11-15).
#[test]
fn a_shift_run_beside_a_compare_keeps_the_byte_in_a() {
    let defs = "\
define i16 @f0(i8 %c, i8 %d, i16 %x) noinline {\n  %s = shl i16 %x, 3\n  %z = zext i8 %c to i16\n  %e = icmp eq i16 %s, %z\n  %r = select i1 %e, i16 7, i16 9\n  ret i16 %r\n}
define i16 @f1(i8 %c, i8 %d, i16 %x) noinline {\n  %s = shl i16 %x, 3\n  %z = zext i8 %c to i16\n  %e = icmp ult i16 %z, %s\n  %r = select i1 %e, i16 7, i16 9\n  ret i16 %r\n}
";
    let call = |function: usize, c: u8, d: u8, x: u16| -> (String, u16) {
        let s = x << 3;
        let hit = if function == 0 { s == u16::from(c) } else { u16::from(c) < s };
        (format!("i8 {}, i8 {}, i16 {}", c as i8, d as i8, x as i16), if hit { 7 } else { 9 })
    };
    for level in [Level::O2, Level::Os] {
        let failed = failing_case("compare", level, defs, call, 2);
        assert_eq!(failed, 0, "{level:?}: case {failed} (of 4 inputs a function) is wrong");
    }
}
