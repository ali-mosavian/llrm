//! A byte argument arrives in `a` with no flags set, and its zero test was
//! `cmp #0` (two bytes). With `x` or `y` dead after it, `tax` or `tay` sets the
//! same flags in one (minmax, fletcher, find, schr: +2 B each over the
//! argument in a cell, 2026-11-08).

use llrm_6502::asm::Asm;
use llrm_6502::peephole::zero_test_through_an_index;

fn run(text: &str) -> String {
    let mut asm = Asm::parse(text);
    // the routine returns a byte in `a` only
    (asm.no_x_result, asm.no_y_result) = (true, true);
    zero_test_through_an_index(&mut asm);
    asm.text()
}

#[test]
fn a_zero_test_of_a_goes_through_a_dead_x() {
    assert_eq!(
        run("  sta rc4\n  cmp #0\n  bne L\n  lda #1\n  rts\nL:\n  ldx #0\n  txa\n  rts\n"),
        "  sta rc4\n  tax\n  bne L\n  lda #1\n  rts\nL:\n  ldx #0\n  txa\n  rts\n"
    );
}

/// `x` is read after the branch: `y` takes it.
#[test]
fn a_live_x_leaves_the_test_to_y() {
    assert_eq!(
        run("  cmp #0\n  bne L\n  txa\n  rts\nL:\n  ldy #0\n  txa\n  rts\n"),
        "  tay\n  bne L\n  txa\n  rts\nL:\n  ldy #0\n  txa\n  rts\n"
    );
}

#[test]
fn both_indexes_live_keep_the_compare() {
    let text = "  cmp #0\n  bne L\n  txa\n  rts\nL:\n  tya\n  rts\n";
    assert_eq!(run(text), text);
}

/// The carry is read after the branch: `cmp #0` leaves it set, `tax` does not.
#[test]
fn a_live_carry_keeps_the_compare() {
    let text = "  cmp #0\n  bne L\n  rts\nL:\n  adc #1\n  rts\n";
    assert_eq!(run(text), text);
}
