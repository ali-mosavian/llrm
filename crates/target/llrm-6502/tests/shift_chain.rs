//! `fuse_shift_chain`: a bit that crosses from one word to the next goes
//! through the carry of the shift of the word that gives it up.

use llrm_6502::asm::Asm;
use llrm_6502::peephole::fuse_shift_chain;

fn fused(text: &str) -> String {
    let mut asm = Asm::parse(text);
    fuse_shift_chain(&mut asm);
    asm.text()
}

/// `x >> 1` of a long in cells was `lda rc2; lsr a; ror rc5; ror rc4; lsr rc3;
/// ror rc2` (6 instructions, 11 B) beside llvm-mos's `lsr rc3; ror rc2; ror
/// rc5; ror rc4`: the load and the shift of the byte re-made the carry that the
/// shift of the word had just left.
#[test]
fn a_funnel_step_passes_the_carry_on() {
    assert_eq!(
        fused("  lda rc2\n  lsr a\n  ror rc5\n  ror rc4\n  lsr rc3\n  ror rc2\n  lda #0\n  rts\n"),
        "  lsr rc3\n  ror rc2\n  ror rc5\n  ror rc4\n  lda #0\n  rts\n"
    );
    assert_eq!(
        fused("  lda rc3\n  asl a\n  rol rc4\n  rol rc5\n  asl rc2\n  rol rc3\n  lda #0\n  rts\n"),
        "  asl rc2\n  rol rc3\n  rol rc4\n  rol rc5\n  lda #0\n  rts\n"
    );
}

/// The word's byte was in X, so the shift ended in `a`: `txa; asl a; rol rc2;
/// rol rc3; txa; asl rc4; rol a` is `txa; asl rc4; rol a; rol rc2; rol rc3`.
#[test]
fn a_funnel_step_on_a_byte_in_an_index_register() {
    assert_eq!(
        fused("  txa\n  asl a\n  rol rc2\n  rol rc3\n  txa\n  asl rc4\n  rol a\n  sta rc5\n  asl a\n  rts\n"),
        "  txa\n  asl rc4\n  rol a\n  rol rc2\n  rol rc3\n  sta rc5\n  asl a\n  rts\n"
    );
}

/// `a` held the byte already (`sta rc5` is its copy): the store stays.
#[test]
fn a_funnel_step_after_the_store_of_its_byte() {
    assert_eq!(
        fused("  sta rc5\n  asl a\n  rol rc2\n  rol rc3\n  asl rc4\n  rol rc5\n  lda #0\n  rts\n"),
        "  sta rc5\n  asl rc4\n  rol rc5\n  rol rc2\n  rol rc3\n  lda #0\n  rts\n"
    );
}

/// The chain ends on another byte, so what follows must not read `a`, the
/// carry or the flags of the old end.
#[test]
fn a_funnel_step_is_kept_where_its_end_is_read() {
    for after in ["  tax\n  rts\n", "  bcc l\n  rts\nl:\n  rts\n", "  adc #1\n  rts\n", "  bne l\n  rts\nl:\n  rts\n"] {
        let text = format!("  lda rc2\n  lsr a\n  ror rc5\n  ror rc4\n  lsr rc3\n  ror rc2\n{after}");
        assert_eq!(fused(&text), text, "{after}");
    }
}
