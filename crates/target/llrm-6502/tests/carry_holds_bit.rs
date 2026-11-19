//! A top bit extracted twice: `k_shrax7` (`x >> 7`) was 18 B because `lda rc3;
//! asl a` took bit 15 into the carry again after `rol rc4` of a copy of rc3
//! had left it there (2026-11-08).

use llrm_6502::asm::Asm;
use llrm_6502::peephole_cells::carry_holds_the_bit;

fn run(text: &str) -> String {
    let mut asm = Asm::parse(text);
    carry_holds_the_bit(&mut asm);
    asm.text()
}

#[test]
fn a_bit_the_carry_holds_from_a_copys_shift_is_not_taken_again() {
    assert_eq!(
        run(
            "  lda rc3\n  sta rc4\n  asl rc2\n  rol rc4\n  lda rc3\n  asl a\n  lda #0\n  rol a\n  tax\n  lda rc4\n  rts\n"
        ),
        "  lda rc3\n  sta rc4\n  asl rc2\n  rol rc4\n  lda #0\n  rol a\n  tax\n  lda rc4\n  rts\n"
    );
}

/// The bottom bit, by `lsr` of the copy.
#[test]
fn the_bottom_bit_too() {
    assert_eq!(
        run("  lda rc3\n  sta rc4\n  lsr rc4\n  lda rc3\n  lsr a\n  lda #0\n  rol a\n  rts\n"),
        "  lda rc3\n  sta rc4\n  lsr rc4\n  lda #0\n  rol a\n  rts\n"
    );
}

/// The cell changes after the copy: the carry holds the old value's bit.
#[test]
fn the_source_written_after_the_copy_keeps_the_extraction() {
    let text = "  lda rc3\n  sta rc4\n  inc rc3\n  rol rc4\n  lda rc3\n  asl a\n  lda #0\n  rol a\n  rts\n";
    assert_eq!(run(text), text);
}

/// Something between sets the carry again.
#[test]
fn a_carry_set_between_keeps_the_extraction() {
    let text = "  lda rc3\n  sta rc4\n  rol rc4\n  clc\n  lda rc3\n  asl a\n  lda #0\n  rol a\n  rts\n";
    assert_eq!(run(text), text);
}

/// The shifted value is read: only a load that replaces it lets it go.
#[test]
fn a_shifted_byte_that_is_read_stays() {
    let text = "  lda rc3\n  sta rc4\n  rol rc4\n  lda rc3\n  asl a\n  adc #1\n  rts\n";
    assert_eq!(run(text), text);
}

/// A label between: another path reaches the extraction.
#[test]
fn a_label_between_keeps_the_extraction() {
    let text = "  lda rc3\n  sta rc4\n  rol rc4\nL:\n  lda rc3\n  asl a\n  lda #0\n  rol a\n  rts\n";
    assert_eq!(run(text), text);
}
