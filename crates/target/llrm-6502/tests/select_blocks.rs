//! A select is a branch over one block (`select::expand_selects`), as the pin's
//! `EmitInstrWithCustomInserter` makes it.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::{estimate, estimate_calling};

fn function(
    ir: &str,
    name: &str,
    level: Level,
) -> String {
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    text[from..].split(".segment").next().unwrap().to_owned()
}

/// `0 <= c - 48 < 10` as one unsigned compare and a 0/1 select.
const ISDIGIT: &str = "define i16 @k_isdigit(i16 %c) {
  %d = add i16 %c, -48
  %b = icmp ult i16 %d, 10
  %r = zext i1 %b to i16
  ret i16 %r
}";

/// The false arm was copied before the compare redone at the branch, with the
/// compare's operand live in A: `pha; txa; tay; pla`, and the constant 1 was
/// parked in a cell in the head. isdigit +8 B, isalpha +8 B, strcmp +5 B
/// against the select as one instruction (2026-11-09).
#[test]
fn a_select_on_the_carry_moves_after_the_compare_and_loads_its_constant_in_the_arm() {
    let asm = function(ISDIGIT, "k_isdigit", Level::O2);
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let branch = lines.iter().position(|l| l.starts_with("bcs ")).unwrap_or_else(|| panic!("no bcs in\n{asm}"));
    assert_eq!(lines[branch - 1], "txa", "the false arm's move sits between the compare and the branch:\n{asm}");
    assert!(!lines.iter().any(|l| matches!(*l, "pha" | "pla" | "tay")), "A was saved around a move: {asm}");
    assert!(
        lines[..branch].iter().all(|l| !l.ends_with("#1")),
        "the constant is loaded in the head, not in its arm:\n{asm}"
    );
    assert!(lines[branch + 1..].iter().any(|l| *l == "lda #1"), "the arm loads the 1:\n{asm}");
    let bytes = estimate(Cpu::Mos6502, &asm).unwrap_or_else(|e| panic!("{e}\n{asm}")).bytes;
    assert!(bytes <= 26, "{bytes} B\n{asm}");
}

/// A select whose result is read from a cell (`sbc rc`): a def kept in a cell
/// cost nothing, so the join stored both literals there (`lda #0; sta rc2` in
/// the head and `lda #1; sta rc2` in the arm, twice for a word) where one
/// store after the join does: phi_slot_unknown_arm +2 B, memcpy_libcall +12 B
/// (2026-11-10).
#[test]
fn a_select_read_from_a_cell_is_stored_once_after_the_join() {
    let ir = "define i16 @k_cell(i16 %a, i16 %b, i16 %c) {
  %cmp = icmp slt i16 %a, %b
  %s = select i1 %cmp, i16 1, i16 0
  %t = sub i16 %c, %s
  ret i16 %t
}";
    let asm = function(ir, "k_cell", Level::O2);
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let from = lines.iter().position(|l| l.ends_with("t:") && l.contains("_cm")).unwrap_or_else(|| panic!("no arm in\n{asm}"));
    let to = from + lines[from..].iter().position(|l| l.contains("_cm") && !l.ends_with("t:")).unwrap();
    assert!(lines[from..to].iter().all(|l| !l.starts_with("sta ")), "the arm stores its literal:\n{asm}");
}

/// A block the select made inherits the loop depth of the block it was cut
/// from (`timing` reads a depth off the last `_b<n>` label): the arm of a
/// select in a loop is weighed as the loop.
#[test]
fn the_blocks_of_a_select_weigh_as_the_block_they_were_cut_from() {
    let text = "f_b1:\n  lda #0\nf_cm1:\n  lda #1\n  rts\n";
    let flat = estimate(Cpu::Mos6502, text).unwrap().cycles;
    let weighed = estimate_calling(Cpu::Mos6502, text, &[], Some(&|n| u32::from(n == 1))).unwrap().cycles;
    // the block is run ten times and so is the arm cut from it
    assert_eq!((flat, weighed), (10, 100), "the arm's label reset the loop's weight");
}

/// `ispunct`: the select's result cell is written on both paths. The shared
/// sink forwarded the head's copy into the join and left the arm's write
/// dead: a 1 for every letter (A = 0, want 1, 2026-11-09).
#[test]
fn a_select_is_right_for_every_byte_on_both_cpus() {
    let ir = "define i16 @k_ispunct(i16 %c) {
  %n = icmp ne i16 %c, 127
  %ge = icmp sge i16 %c, 65
  %le = icmp sle i16 %c, 90
  %up = and i1 %ge, %le
  %not = xor i1 %up, true
  %r = and i1 %n, %not
  %z = zext i1 %r to i16
  ret i16 %z
}";
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let compiled = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let functions: String =
        compiled.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
    // the reference, byte by byte: 1 unless the byte is 127 or 'A'..='Z'; `bad`
    // is set by any mismatch
    let main = "  lda #0\n  sta bad\n  sta ia\nloop:\n  lda #1\n  sta want\n  lda ia\n  cmp #127\n  bne n1\n  lda #0\n  sta want\n\
                n1:\n  lda ia\n  cmp #65\n  bcc n2\n  cmp #91\n  bcs n2\n  lda #0\n  sta want\nn2:\n  lda ia\n  sta rc2\n  lda #0\n  sta rc3\n  \
                jsr k_ispunct\n  cmp want\n  beq ok\n  lda #1\n  sta bad\nok:\n  inc ia\n  bne loop\n  lda bad\n";
    let extra = format!("{functions}.segment \"BSS\"\nia: .res 1\nwant: .res 1\nbad: .res 1\n.segment \"CODE\"\n");
    for target in ["sim6502", "sim65c02"] {
        assert_eq!(common::run_on(target, &format!("sel-{target}"), main, &extra).0, 0, "{target}");
    }
    let cycles = common::cycles_on("sim6502", "sel-cycles", main, &extra);
    assert!(cycles > 10_000, "the driver ran {cycles} cycles for 256 calls");
}

/// A word select on each kind of flag, run for pairs that differ in the high
/// byte, in the low byte only, and not at all: the carry (`ult`), the sign
/// (`slt`) and the zero flag (`eq`) each reach the branch over the arm.
#[test]
fn a_word_select_is_right_on_the_carry_the_sign_and_the_zero_flag() {
    let pairs: [(u16, u16); 7] =
        [(0, 0), (1, 2), (2, 1), (0x0100, 0x00FF), (0x00FF, 0x0100), (0x8000, 0x7FFF), (0x7FFF, 0x8000)];
    let mut functions = String::new();
    let mut main = String::from("  lda #0\n  sta bad\n");
    type Pick = fn(u16, u16) -> bool;
    let kinds: [(&str, Pick); 3] =
        [("ult", |a, b| a < b), ("slt", |a, b| (a as i16) < (b as i16)), ("eq", |a, b| a == b)];
    for (n, (name, pick)) in kinds.into_iter().enumerate() {
        let ir = format!(
            "define i16 @k_{name}(i16 %a, i16 %b) {{\n  %c = icmp {name} i16 %a, %b\n  %r = select i1 %c, i16 %a, i16 %b\n  ret i16 %r\n}}"
        );
        let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
        let compiled = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
        let kept = compiled.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $"));
        functions.extend(kept.map(|l| format!("{l}\n")));
        for (k, (a, b)) in pairs.into_iter().enumerate() {
            let want = if pick(a, b) { a } else { b };
            main += &format!(
                "  lda #{}\n  sta rc2\n  lda #{}\n  sta rc3\n  lda #{}\n  sta rc4\n  lda #{}\n  sta rc5\n  jsr k_{name}\n  \
                 cmp #{}\n  bne bad{n}_{k}\n  cpx #{}\n  beq ok{n}_{k}\nbad{n}_{k}:\n  lda #1\n  sta bad\nok{n}_{k}:\n",
                a & 0xFF,
                a >> 8,
                b & 0xFF,
                b >> 8,
                want & 0xFF,
                want >> 8
            );
        }
    }
    main += "  lda bad\n";
    let extra = format!("{functions}.segment \"BSS\"\nbad: .res 1\n.segment \"CODE\"\n");
    for target in ["sim6502", "sim65c02"] {
        assert_eq!(common::run_on(target, &format!("selw-{target}"), &main, &extra).0, 0, "{target}");
    }
}
