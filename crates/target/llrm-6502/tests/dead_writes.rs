//! A register-only instruction whose results nobody reads is not emitted.

use llrm_6502::asm::Asm;
use llrm_6502::peephole::drop_dead_writes;

fn run(text: &str) -> String {
    let mut asm = Asm::parse(text);
    drop_dead_writes(&mut asm);
    asm.text()
}

/// The borrow out of the last limb of a subtract chain, saved as a byte whose
/// store was dropped: `lda #0; rol a` before a load, 3 B and 4 clocks a trip
/// of the pair loop. Both go, the `rol` first.
#[test]
fn a_carry_byte_nobody_reads_is_dropped() {
    assert_eq!(
        run("  sbc rc20\n  sta rc17\n  lda #0\n  rol a\n  lda rc18\n  lsr a\n  rts\n"),
        "  sbc rc20\n  sta rc17\n  lda rc18\n  lsr a\n  rts\n"
    );
}

/// Not the shape: an index register loaded and loaded again.
#[test]
fn a_register_written_again_before_a_read_is_dropped() {
    assert_eq!(run("  ldx #5\n  ldx rc2\n  stx rc3\n  rts\n"), "  ldx rc2\n  stx rc3\n  rts\n");
}

/// The carry is read by the `adc` after: the byte is the carry's only copy.
#[test]
fn a_carry_byte_stays_when_the_carry_is_read_after() {
    let text = "  lda #0\n  rol a\n  lda rc18\n  adc rc19\n  rts\n";
    assert_eq!(run(text), text);
}

/// The zero flag is read by the branch.
#[test]
fn a_load_stays_when_a_branch_reads_its_flags() {
    let text = "  ldx #5\n  beq L\n  ldx rc2\n  stx rc3\nL:\n  ldx rc4\n  stx rc5\n  rts\n";
    assert_eq!(run(text), text);
}

/// The register is read.
#[test]
fn a_load_stays_when_the_register_is_read() {
    let text = "  ldx #5\n  stx rc3\n  ldx rc2\n  rts\n";
    assert_eq!(run(text), text);
}

/// The `cmp` feeds the `bne`; the `clc` between them, its carry written again
/// on both paths before any read, is the only one to go.
#[test]
fn a_dead_carry_flag_between_a_compare_and_its_branch_goes_and_the_compare_stays() {
    assert_eq!(
        run("  cmp rc3\n  clc\n  bne L\n  rts\nL:\n  clc\n  adc rc4\n  rts\n"),
        "  cmp rc3\n  bne L\n  rts\nL:\n  clc\n  adc rc4\n  rts\n"
    );
}

/// The carry the `cmp` leaves is read by the `bcs`.
#[test]
fn a_compare_stays_when_a_branch_reads_its_carry() {
    let text = "  cmp rc3\n  bcs L\n  rts\nL:\n  rts\n";
    assert_eq!(run(text), text);
}

/// The zero flag the `cmp` leaves is read by the `bne`.
#[test]
fn a_compare_stays_when_a_branch_reads_its_zero_flag() {
    let text = "  cmp rc3\n  bne L\n  rts\nL:\n  rts\n";
    assert_eq!(run(text), text);
}

/// `bit` ands `a` with its operand: a load before it was dropped as dead.
#[test]
fn an_accumulator_load_stays_before_a_bit_test() {
    let text = "  lda rc3\n  bit rc2\n  bne out\n  rts\nout:\n  rts\n";
    let mut asm = Asm::parse(text);
    asm.no_a_result = true;
    drop_dead_writes(&mut asm);
    assert_eq!(asm.text(), text);
}
