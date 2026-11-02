//! `timing.rs`'s bytes and cycles against sim65, which counts a cycle for each
//! of an instruction's cycles: a row is run twenty times and priced from the
//! difference with a run of none. The table is from the W65C02S datasheet and
//! masswerk's tables; this is the check that the code reads it right.

mod common;

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::timing::instruction;

const TARGETS: [(&str, Cpu); 2] = [("sim6502", Cpu::Mos6502), ("sim65c02", Cpu::W65c02)];

/// `rc2:rc3` points at `slot`, `x` is 0 and `y` is 1.
const SETUP: &str = "  lda #<slot\n  sta rc2\n  lda #>slot\n  sta rc3\n  lda #0\n  sta rc5\n  ldx #0\n  ldy #1\n";
const DATA: &str = ".segment \"BSS\"\nslot: .res 4\ncnt: .res 1\n.segment \"CODE\"\n";

/// Cycles one trip of `body` takes in a counted loop (`dec cnt` and `bne`),
/// found from two runs of one binary that differ only in the count. sim65's
/// total moves by a cycle when the program's size puts later code across a
/// page, so two runs of equal layout are the only fair difference.
fn trip(
    target: &str,
    name: &str,
    body: &str,
) -> i64 {
    let run = |count: u32| {
        let main = format!("{SETUP}  lda #{count}\n  sta cnt\nagain:\n{body}  dec cnt\n  bne again\n");
        common::cycles_on(target, &format!("{name}-{count}"), &main, DATA)
    };
    let (many, one) = (run(101), run(1));
    assert_eq!((many - one) % 100, 0, "{target} {name}: {many} - {one}");
    (many - one) / 100
}

/// What `trip` charges an empty body: `dec cnt` (6) and a taken `bne` (3).
///
/// `test` is the calling test's own name: two tests building one work directory
/// at once corrupt it (`ld65: Read error`, `Unresolved external '_main'`, about
/// two runs in three).
fn loop_cycles(
    target: &str,
    test: &str,
) -> i64 {
    trip(target, &format!("loop-{test}"), "")
}

/// Instructions that can run in a row without harm.
const STRAIGHT: &[&str] = &[
    "lda #1",
    "lda rc5",
    "lda slot",
    "lda slot,x",
    "lda slot,y",
    "lda rc5,x",
    "lda (rc2,x)",
    "lda (rc2),y",
    "ldx #1",
    "ldx rc5",
    "ldx slot",
    "ldx slot,y",
    "ldy #1",
    "ldy rc5",
    "ldy slot",
    "ldy slot,x",
    "sta rc5",
    "sta slot",
    "sta rc5,x",
    "sta slot,x",
    "sta slot,y",
    "sta (rc2,x)",
    "sta (rc2),y",
    "stx rc5",
    "stx slot",
    "sty rc5",
    "sty slot",
    "adc #1",
    "adc rc5",
    "adc slot",
    "adc (rc2),y",
    "sbc #1",
    "sbc rc5",
    "and #1",
    "and rc5",
    "ora #1",
    "eor #1",
    "cmp #1",
    "cmp rc5",
    "cmp slot",
    "cpx #1",
    "cpx rc5",
    "cpy #1",
    "cpy slot",
    "inc rc5",
    "inc slot",
    "inc rc5,x",
    "dec rc5",
    "dec slot",
    "asl a",
    "asl rc5",
    "asl slot",
    "lsr a",
    "lsr rc5",
    "rol a",
    "rol rc5",
    "ror a",
    "ror rc5",
    "tax",
    "tay",
    "txa",
    "tya",
    "inx",
    "iny",
    "dex",
    "dey",
    "clc",
    "sec",
    "nop",
    "lda (rc2)",
    "sta (rc2)",
    "stz rc5",
    "stz slot",
    "inc a",
    "dec a",
];

/// The forms only the 65C02 has.
const CMOS_ONLY: [&str; 6] = ["lda (rc2)", "sta (rc2)", "stz rc5", "stz slot", "inc a", "dec a"];

#[test]
fn every_row_costs_what_sim65_counts() {
    for (target, cpu) in TARGETS {
        let overhead = loop_cycles(target, "rows");
        assert_eq!(overhead, 9, "{target}: dec abs (6) and a taken bne (3)");
        for (n, text) in STRAIGHT.iter().enumerate() {
            let (mnemonic, operand) = text.split_once(' ').unwrap_or((text, ""));
            let Some(cost) = instruction(cpu, mnemonic, operand) else {
                assert!(cpu == Cpu::Mos6502 && CMOS_ONLY.contains(text), "{target}: no price for `{text}`");
                continue;
            };
            let got = trip(target, &format!("row{n}"), &format!("  {text}\n")) - overhead;
            assert_eq!(got, i64::from(cost.cycles), "{target}: `{text}`");
        }
    }
}

/// The branches, jumps and calls, each with its target the next line or a
/// routine of one `rts`.
#[test]
fn flow_costs_what_sim65_counts() {
    for (target, cpu) in TARGETS {
        let overhead = loop_cycles(target, "flow");
        let mut cases: Vec<(&str, &str, i64)> = vec![
            ("not taken", "  lda #1\n  beq L\nL:\n", 2 + 2),
            ("taken", "  lda #0\n  beq L\nL:\n", 2 + 3),
            ("jmp", "  jmp L\nL:\n", 3),
            ("jsr and rts", "  jsr L\n  jmp M\nL:\n  rts\nM:\n", 6 + 6 + 3),
            ("pha and pla", "  pha\n  pla\n", 3 + 4),
            ("php and plp", "  php\n  plp\n", 3 + 4),
        ];
        if cpu == Cpu::W65c02 {
            cases.push(("bra", "  bra L\nL:\n", 3));
            cases.push(("phx and plx", "  phx\n  plx\n", 3 + 4));
            cases.push(("phy and ply", "  phy\n  ply\n", 3 + 4));
        }
        for (n, (name, body, want)) in cases.into_iter().enumerate() {
            assert_eq!(trip(target, &format!("flow{n}"), body) - overhead, want, "{target}: {name}");
        }
        assert_eq!(instruction(cpu, "jsr", "f").unwrap().cycles, 6);
        assert_eq!(instruction(cpu, "rts", "").unwrap().cycles, 6);
        assert_eq!(instruction(cpu, "jmp", "f").unwrap().cycles, 3);
        assert_eq!(instruction(cpu, "bne", "f").unwrap().cycles, 2);
    }
}

/// Bytes are what the assembler writes: the difference of two linked sizes.
#[test]
fn every_row_takes_the_bytes_the_assembler_writes() {
    for (target, cpu) in TARGETS {
        let size = |name: &str, body: &str| common::run_on(target, name, &format!("{SETUP}{body}"), DATA).1 as i64;
        let none = size("bytes-none", "");
        for (n, text) in STRAIGHT.iter().enumerate() {
            let (mnemonic, operand) = text.split_once(' ').unwrap_or((text, ""));
            let Some(cost) = instruction(cpu, mnemonic, operand) else { continue };
            let got = size(&format!("bytes{n}"), &format!("  {text}\n").repeat(20)) - none;
            assert_eq!(got, 20 * i64::from(cost.bytes), "{target}: `{text}`");
        }
    }
}

/// A macro definition in the text (the runtime keeps the ones its routines use)
/// was priced as code: `mul16`'s priced size was 98 bytes against 44 linked
/// once the runtime carried its macros.
#[test]
fn a_macro_body_is_not_priced_as_code() {
    let text = ".macro twice\n  asl a\n  asl a\n.endmacro\nf:\n  lda #1\n  rts\n";
    let priced = llrm_6502::timing::estimate(Cpu::Mos6502, text).expect("prices");
    assert_eq!(priced.bytes, 3, "lda #1 and rts only");
}

/// A macro use was a line the estimator could not price ("cannot price `neg16
/// rc2, rc3`"), so the signed word division and modulo kernels of the hand
/// corpus had no row. It costs what its body costs.
#[test]
fn a_macro_use_is_priced_as_its_body() {
    let macros = ".macro neg16 lo, hi\n  sec\n  lda #0\n  sbc lo\n  sta lo\n  lda #0\n  sbc hi\n  sta hi\n.endmacro\n";
    let used = format!("{macros}f:\n  neg16 rc2, rc3\n  rts\n");
    let written = "f:\n  sec\n  lda #0\n  sbc rc2\n  sta rc2\n  lda #0\n  sbc rc3\n  sta rc3\n  rts\n";

    let used = llrm_6502::timing::estimate(Cpu::Mos6502, &used).expect("prices the macro use");

    assert_eq!(used, llrm_6502::timing::estimate(Cpu::Mos6502, written).unwrap());
}

/// A lookup table in `RODATA` is bytes of the linked file: the estimator
/// skipped the data segments, so a function with a CRC table priced 84 bytes
/// against 596 linked and the bench's size check failed.
#[test]
fn data_bytes_in_the_file_are_counted() {
    let text = "f:\n  rts\n.segment \"RODATA\"\nt:\n  .byte 1,2,3\n  .word 5\n.segment \"BSS\"\nb: .res 9\n.segment \"CODE\"\n";

    let priced = llrm_6502::timing::estimate(Cpu::Mos6502, text).unwrap();

    assert_eq!(priced.bytes, 1 + 3 + 2);
}

/// A branch up to a block that ends in `rts` is an exit laid out above the
/// code, not a loop: it priced the lines between at ten trips each, 62871
/// clocks for a 96-byte `strcspn` against 3770 for its 103-byte twin, and the
/// emitter kept the twin.
#[test]
fn a_branch_up_to_an_exit_is_not_a_loop() {
    let text = "f:\n  jmp go\ndone:\n  rts\ngo:\n  lda #1\n  beq done\n  lda #2\n  rts\n";

    let priced = llrm_6502::timing::estimate(Cpu::Mos6502, text).unwrap();

    // jmp 3, rts 6, lda 2, beq 2 (not taken), lda 2, rts 6
    assert_eq!(priced.cycles, 21);
}
