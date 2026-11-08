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

/// A routine that returns a byte in `a` only.
fn after_a_byte(code: &str) -> String {
    let mut asm = Asm::parse(code);
    (asm.no_a_result, asm.no_x_result, asm.no_y_result) = (true, true, true);
    run(&mut asm);
    asm.text()
}

/// `__divdf3` at -O2 read a spilled byte through `y` into a cell, `ldy s_6; sty
/// rc16; sbc rc16` (the -Os code is `sbc s_6`): 61 clocks a trip of its restoring
/// loop, +4457 clocks a division.
#[test]
fn an_operand_loaded_through_y_into_a_cell_is_the_operand() {
    let before = "  lda rc2\n  sec\n  ldy s_6\n  sty rc16\n  sbc rc16\n  sta out\n  rts\n";
    let want = "  lda rc2\n  sec\n  sbc s_6\n  sta out\n  rts\n";
    assert_eq!(after_a_byte(before), want);
}

/// The same through `x`, and with the carry op between the load and the user.
#[test]
fn an_operand_loaded_through_x_into_a_cell_is_the_operand() {
    let before = "  lda rc2\n  ldx s_6\n  stx rc16\n  clc\n  adc rc16\n  sta out\n  rts\n";
    let want = "  lda rc2\n  clc\n  adc s_6\n  sta out\n  rts\n";
    assert_eq!(after_a_byte(before), want);
}

/// `y` read after the user, or the cell read again: both stay.
#[test]
fn a_live_index_register_or_cell_keeps_the_temporary() {
    let y_live = "  lda rc2\n  ldy s_6\n  sty rc16\n  sbc rc16\n  sty out\n  rts\n";
    assert_eq!(after_a_byte(y_live), y_live);
    let cell_live = "  lda rc2\n  ldy s_6\n  sty rc16\n  sbc rc16\n  sta out\n  lda rc16\n  sta out2\n  rts\n";
    assert_eq!(after_a_byte(cell_live), cell_live);
}
