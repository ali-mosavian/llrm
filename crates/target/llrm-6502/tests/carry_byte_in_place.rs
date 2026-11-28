//! `carry_stays_in_the_flag` and `arithmetic_chain_in_a` rewrite only where
//! nothing reads what the rewrite changes.

use llrm_6502::asm::Asm;
use llrm_6502::peephole_cells::{arithmetic_chain_in_a, carry_stays_in_the_flag};

fn carry(text: &str) -> String {
    let mut asm = Asm::parse(text);
    asm.no_x_result = true;
    asm.no_y_result = true;
    carry_stays_in_the_flag(&mut asm);
    asm.text()
}

/// The shape the pass is for: the carry waits in `x` while work that never
/// reads it runs.
#[test]
fn a_carry_byte_nobody_reads_goes() {
    assert_eq!(
        carry(
            "  lda #0\n  rol a\n  tax\n  lda rc4\n  sta rc5\n  txa\n  lsr a\n  lda rc6\n  adc rc7\n  sta rc2\n  rts\n"
        ),
        "  lda rc4\n  sta rc5\n  lda rc6\n  adc rc7\n  sta rc2\n  rts\n"
    );
}

/// `tay` before any load copied the carry byte; the rewrite copied the old `a`.
#[test]
fn a_copy_of_the_carry_byte_keeps_it() {
    let text = "  lda #0\n  rol a\n  tax\n  tay\n  sty rc10\n  txa\n  lsr a\n  lda rc6\n  adc rc7\n  sta rc2\n  rts\n";
    assert_eq!(carry(text), text);
}

/// `stx` stored the carry byte held in `x`; the rewrite stored the old `x`.
#[test]
fn a_store_of_the_carry_byte_in_x_keeps_it() {
    let text = "  lda #0\n  rol a\n  tax\n  stx rc10\n  txa\n  lsr a\n  lda rc6\n  adc rc7\n  sta rc2\n  rts\n";
    assert_eq!(carry(text), text);
}

/// `lsr a` left Z set and N clear; the rewrite left the work's flags to the
/// `beq`.
#[test]
fn a_zero_branch_after_the_shift_keeps_it() {
    let text = "  lda #0\n  rol a\n  tax\n  lda rc4\n  sta rc5\n  txa\n  lsr a\n  beq out\n  lda rc6\n  sta rc2\nout:\n  lda rc7\n  sta rc2\n  rts\n";
    assert_eq!(carry(text), text);
}

/// The chain left the old high byte in `a`; the rewrite leaves the shifted one.
#[test]
fn an_accumulator_read_after_the_arithmetic_shift_chain_keeps_it() {
    let text = "  lda rc3\n  cmp #$80\n  ror rc3\n  ror rc2\n  lda rc3\n  cmp #$80\n  ror rc3\n  ror rc2\n  sta rc4\n  jsr f\n  rts\n";
    let mut asm = Asm::parse(text);
    arithmetic_chain_in_a(&mut asm);
    assert_eq!(asm.text(), text);
}
