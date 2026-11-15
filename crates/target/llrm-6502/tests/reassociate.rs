//! k_stricmp folded `if (c >= 'a' && c <= 'z') c -= 32` as `adc #159; cmp #26;
//! bcs; clc; txa; adc #224`: the byte `a` already held (x - 97) was thrown
//! away and x - 32 made again from x, and at -O1 the compare went through a
//! copy of `a` in `y` (2026-11-15: 73 B at -O2 and 77 B at -O1, bar 70).

use std::process::Command;

use llrm_6502::asm::Asm;
use llrm_6502::peephole::run;
use llrm_6502::reassociate::reassociate_adds;
use llrm_6502::sink_compare::{compare_the_copy, sink_past_compare};

mod common;
use common::pipeline::llrm_cc;

fn reassociated(text: &str) -> String {
    let mut asm = Asm::parse(text);
    reassociate_adds(&mut asm);
    asm.text()
}

/// The `clc` after the `bcs` goes (the compare made the carry clear); the load
/// and the add stay.
fn without_clc(text: &str) -> String {
    text.replace("  bcs L\n  clc\n  txa\n", "  bcs L\n  txa\n")
}

const FOLD: &str = "  clc\n  txa\n  adc #159\n  cmp #26\n  bcs L\n  clc\n  txa\n  adc #224\n  tax\nL:\n  clc\n  rts\n";

#[test]
fn a_sum_of_a_byte_whose_other_sum_a_holds_adds_the_difference() {
    assert_eq!(
        reassociated(FOLD),
        "  clc\n  txa\n  adc #159\n  cmp #26\n  bcs L\n  adc #65\n  tax\nL:\n  clc\n  rts\n"
    );
}

#[test]
fn a_cell_source_too() {
    let text = FOLD.replace("txa", "lda rc6");
    assert_eq!(
        reassociated(&text),
        "  clc\n  lda rc6\n  adc #159\n  cmp #26\n  bcs L\n  adc #65\n  tax\nL:\n  clc\n  rts\n"
    );
}

/// The carry is not known: the `clc` stays, the load goes.
#[test]
fn an_unknown_carry_keeps_the_clc() {
    assert_eq!(
        reassociated("  clc\n  txa\n  adc #159\n  tay\n  clc\n  txa\n  adc #224\n  cpy #26\n  rts\n"),
        "  clc\n  txa\n  adc #159\n  tay\n  clc\n  adc #65\n  cpy #26\n  rts\n"
    );
}

#[test]
fn a_source_written_between_keeps_the_load() {
    let text = FOLD.replace("  cmp #26\n", "  inx\n  cmp #26\n");
    assert_eq!(reassociated(&text), without_clc(&text));
}

#[test]
fn a_changed_accumulator_keeps_the_load() {
    let text = FOLD.replace("  cmp #26\n", "  eor #1\n  cmp #26\n");
    assert_eq!(reassociated(&text), without_clc(&text));
}

/// The new add leaves another carry than the old: a branch on it reads wrong.
#[test]
fn a_carry_that_is_read_keeps_the_load() {
    let text = FOLD.replace("  tax\nL:", "  bcs L\n  tax\nL:");
    assert_eq!(reassociated(&text), without_clc(&text));
}

/// Another path reaches the second sum: `a` holds nothing known there.
#[test]
fn a_label_something_jumps_to_keeps_the_load() {
    let text = FOLD.replace("  bcs L\n", "  bcs L\nM:\n");
    let text = text.replace("  rts\n", "  bne M\n  rts\n");
    assert_eq!(reassociated(&text), text);
}

#[test]
fn a_clc_after_a_branch_that_settled_the_carry_goes() {
    assert_eq!(
        reassociated("  cmp #26\n  bcs L\n  clc\n  adc #1\n  tax\nL:\n  clc\n  rts\n"),
        "  cmp #26\n  bcs L\n  adc #1\n  tax\nL:\n  clc\n  rts\n"
    );
}

fn sunk(text: &str) -> String {
    let mut asm = Asm::parse(text);
    sink_past_compare(&mut asm);
    asm.text()
}

#[test]
fn work_on_a_that_only_the_fall_through_wants_follows_the_branch() {
    assert_eq!(
        sunk("  tay\n  clc\n  adc #65\n  cpy #26\n  bcs L\n  tax\nL:\n  lda #0\n  rts\n"),
        "  tay\n  cpy #26\n  bcs L\n  clc\n  adc #65\n  tax\nL:\n  lda #0\n  rts\n"
    );
}

#[test]
fn work_on_a_that_the_branch_target_reads_stays() {
    let text = "  tay\n  clc\n  adc #65\n  cpy #26\n  bcs L\n  tax\nL:\n  sta rc2\n  rts\n";
    assert_eq!(sunk(text), text);
}

#[test]
fn a_fall_through_label_others_reach_keeps_the_work() {
    let text = "  tay\n  clc\n  adc #65\n  cpy #26\n  bcs L\nM:\n  tax\nL:\n  lda #0\n  bne M\n  rts\n";
    assert_eq!(sunk(text), text);
}

#[test]
fn a_compare_of_a_copy_of_a_is_a_compare_of_a() {
    let mut asm = Asm::parse("  tay\n  cpy #26\n  bcs L\n  inx\nL:\n  ldy rc9\n  rts\n");
    compare_the_copy(&mut asm);
    assert_eq!(asm.text(), "  cmp #26\n  bcs L\n  inx\nL:\n  ldy rc9\n  rts\n");
}

#[test]
fn a_copy_that_is_read_later_stays() {
    let text = "  tay\n  cpy #26\n  bcs L\n  inx\nL:\n  sty rc2\n  rts\n";
    let mut asm = Asm::parse(text);
    compare_the_copy(&mut asm);
    assert_eq!(asm.text(), text);
}

/// The whole pipeline on the -O1 text: no `tay`, no `cpy`, one `clc` per site
/// at most.
#[test]
fn the_o1_range_fold_is_a_compare_of_a_and_one_add() {
    let mut asm = Asm::parse(
        "  clc\n  txa\n  adc #159\n  tay\n  clc\n  txa\n  adc #224\n  cpy #26\n  bcs L\n  tax\nL:\n  lda #0\n  ldy rc9\n  rts\n",
    );
    run(&mut asm);
    assert_eq!(
        asm.text(),
        "  clc\n  txa\n  adc #159\n  cmp #26\n  bcs L\n  adc #65\n  tax\nL:\n  lda #0\n  ldy rc9\n  rts\n"
    );
}

fn code_bytes(level: &str) -> u32 {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("reassoc-stricmp{level}"));
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bench/hand/stricmp.c");
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", level, "-S", "-w", "-o", "o.s"])
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let built = Command::new("ca65").current_dir(&dir).args(["-U", "-o", "o.o", "o.s"]).output().unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let dump = Command::new("od65").current_dir(&dir).args(["--dump-segsize", "o.o"]).output().unwrap();
    let text = String::from_utf8_lossy(&dump.stdout);
    text.lines()
        .find_map(|l| l.trim().strip_prefix("CODE:").and_then(|n| n.trim().parse().ok()))
        .unwrap_or_else(|| panic!("no CODE size in {text}"))
}

/// llvm-mos is 70 B at -O1 and -O2 (73 and 77 B before).
#[test]
fn k_stricmp_is_at_the_bar_at_o1_and_o2() {
    for level in ["-O1", "-O2"] {
        let code = code_bytes(level);
        assert!(code <= 70, "k_stricmp {level}: {code} B, bar 70");
    }
}
