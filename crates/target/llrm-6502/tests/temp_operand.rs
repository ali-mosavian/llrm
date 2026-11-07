//! The first of two memory operands goes to the instruction from where it was
//! loaded, not through `a` and a temporary.

use llrm_6502::asm::Asm;
use llrm_6502::peephole::run;

fn after(code: &str) -> String {
    let mut asm = Asm::parse(code);
    asm.no_a_result = true;
    run(&mut asm);
    asm.text()
}

/// `dotx`: `a[i] ^ b[i]` was `lda (a),y; sta t; lda (b),y; eor t`, 8 B for the
/// 4 of `lda (b),y; eor (a),y`.
#[test]
fn two_indirect_operands_need_no_temporary() {
    let before = "  ldy #0\nl:\n  lda (rc2),y\n  sta rc16\n  lda (rc4),y\n  eor rc16\n  sta out\n  iny\n  bne l\n  rts\n";
    let want = "  ldy #0\nl:\n  lda (rc4),y\n  eor (rc2),y\n  sta out\n  iny\n  bne l\n  rts\n";
    assert_eq!(after(before), want);
}

/// A carry op between the load and the instruction stays; a subtract keeps its
/// order (`y - x`).
#[test]
fn a_carry_op_and_a_subtract_keep_their_order() {
    let before = "  lda rc2\n  sta rc16\n  lda (rc4),y\n  sec\n  sbc rc16\n  sta out\n  rts\n";
    let want = "  lda (rc4),y\n  sec\n  sbc rc2\n  sta out\n  rts\n";
    assert_eq!(after(before), want);
}

/// The temporary read again after: it stays.
#[test]
fn a_temporary_that_is_read_again_stays() {
    let code = "  lda (rc2),y\n  sta rc16\n  lda (rc4),y\n  eor rc16\n  sta out\n  lda rc16\n  sta out2\n  rts\n";
    assert_eq!(after(code), code);
}
