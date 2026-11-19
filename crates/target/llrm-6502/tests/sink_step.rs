//! A step of the loop count moves down to the compare it makes redundant.

use llrm_6502::asm::Asm;
use llrm_6502::peephole::run;

fn after(code: &str) -> String {
    let mut asm = Asm::parse(code);
    asm.no_y_result = true;
    run(&mut asm);
    asm.text()
}

/// `xs8`: `dex` at the top of the rotated loop, `cpx #0; bne` at the bottom
/// after the work (37 B against 33 for llvm-mos, 2 B and 2 clocks a trip).
#[test]
fn a_count_taken_down_at_the_top_is_tested_at_the_bottom_without_a_compare() {
    let before = "  ldx rc3\nb2:\n  dex\n  lda rc2\n  asl a\n  eor rc2\n  sta rc2\n  cpx #0\n  bne b2\n  rts\n";
    let want = "  ldx rc3\nb2:\n  lda rc2\n  asl a\n  eor rc2\n  sta rc2\n  dex\n  bne b2\n  rts\n";
    assert_eq!(after(before), want);
}

/// Work that reads the register (`lda tab,x`) sees the stepped value if the
/// step moves: it stays.
#[test]
fn a_step_stays_above_work_that_reads_its_register() {
    let code = "  ldx rc3\nb2:\n  dex\n  lda tab,x\n  sta out\n  cpx #0\n  bne b2\n  rts\n";
    assert_eq!(after(code), code);
}

/// A label between them is another way in: the step stays.
#[test]
fn a_step_stays_above_a_label() {
    let code = "  ldx rc3\nb2:\n  dex\n  lda rc2\nb3:\n  sta out\n  cpx #0\n  bne b2\n  rts\n";
    assert_eq!(after(code), code);
}

/// `xs8`'s guard loaded the count into `x` for the loop and into `y` for the
/// zero test, two bytes for flags `ldx` had set.
#[test]
fn a_second_load_of_a_cell_only_to_branch_on_it_is_dropped() {
    let before = "  ldx rc3\n  ldy rc3\n  beq b3\nb2:\n  dex\n  bne b2\nb3:\n  lda rc2\n  rts\n";
    let want = "  ldx rc3\n  beq b3\nb2:\n  dex\n  bne b2\nb3:\n  lda rc2\n  rts\n";
    assert_eq!(after(before), want);
    let used = "  ldx rc3\n  ldy rc3\n  beq b3\nb3:\n  lda (rc4),y\n  rts\n";
    assert_eq!(after(used), used, "y is read after");
}
