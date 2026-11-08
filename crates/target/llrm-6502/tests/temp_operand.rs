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

/// `__divdf3` at -O2 read a spilled byte through `y` into a cell, `ldy s_f_6; sty
/// rc16; sbc rc16` (the -Os code is `sbc s_f_6`): 61 clocks a trip of its restoring
/// loop, +4457 clocks a division.
#[test]
fn an_operand_loaded_through_y_into_a_cell_is_the_operand() {
    let before = "  lda rc2\n  sec\n  ldy s_f_6\n  sty rc16\n  sbc rc16\n  sta out\n  rts\n";
    let want = "  lda rc2\n  sec\n  sbc s_f_6\n  sta out\n  rts\n";
    assert_eq!(after_a_byte(before), want);
}

/// The same through `x`, and with the carry op between the load and the user.
#[test]
fn an_operand_loaded_through_x_into_a_cell_is_the_operand() {
    let before = "  lda rc2\n  ldx s_f_6\n  stx rc16\n  clc\n  adc rc16\n  sta out\n  rts\n";
    let want = "  lda rc2\n  clc\n  adc s_f_6\n  sta out\n  rts\n";
    assert_eq!(after_a_byte(before), want);
}

/// `y` still read after the user keeps its load, and the one read reads the
/// byte: the copy to the cell goes.
#[test]
fn a_live_index_register_keeps_its_load_and_loses_the_copy() {
    let before = "  lda rc2\n  ldy s_f_6\n  sty rc16\n  sbc rc16\n  sty out\n  rts\n";
    let want = "  lda rc2\n  ldy s_f_6\n  sbc s_f_6\n  sty out\n  rts\n";
    assert_eq!(after_a_byte(before), want);
}

/// Every read of the cell on every path reads the byte, in the next block too
/// (a compare chain and the subtract chain after it).
#[test]
fn the_reads_of_the_cell_in_a_later_block_read_the_byte() {
    let before = "  lda rc2\n  sec\n  ldy s_f_6\n  sty rc16\n  sbc rc16\n  bcc skip\n  lda rc3\n  sbc rc16\n  sta out\nskip:\n  rts\n";
    let want = "  lda rc2\n  sec\n  sbc s_f_6\n  bcc skip\n  lda rc3\n  sbc s_f_6\n  sta out\nskip:\n  rts\n";
    assert_eq!(after_a_byte(before), want);
}

/// Another path into a label where the cell is live brings another value: the
/// copy stays.
#[test]
fn a_label_another_path_reaches_with_the_cell_live_keeps_the_copy() {
    let code = "  lda rc2\n  bne other\n  ldy s_f_6\n  sty rc16\n  jmp join\nother:\n  ldy #3\n  sty rc16\njoin:\n  lda rc3\n  sbc rc16\n  sta out\n  rts\n";
    assert_eq!(after_a_byte(code), code);
}

/// The byte rewritten on the way: the cell is the old value, the copy stays.
#[test]
fn a_byte_stored_between_the_copy_and_the_read_keeps_the_copy() {
    let code = "  lda rc2\n  ldy s_f_6\n  sty rc16\n  sta s_f_6\n  lda rc3\n  sbc rc16\n  sta out\n  rts\n";
    assert_eq!(after_a_byte(code), code);
}

/// A call ends the cell's life above the argument cells (the `__divdf3` loop
/// exit), and reads it nowhere; at `rc4` it may be an argument and the copy
/// stays.
#[test]
fn a_call_ends_a_scratch_cell_and_reads_an_argument_cell() {
    let scratch = "  lda rc2\n  ldy s_f_6\n  sty rc16\n  sbc rc16\n  bcc out\n  jsr f\nout:\n  rts\n";
    assert_eq!(after_a_byte(scratch), "  lda rc2\n  sbc s_f_6\n  bcc out\n  jsr f\nout:\n  rts\n");
    let argument = "  lda rc2\n  ldy s_f_6\n  sty rc4\n  sbc rc4\n  bcc out\n  jsr f\nout:\n  rts\n";
    assert_eq!(after_a_byte(argument), argument);
}

/// A global byte is not a spill slot: `buf+2+1` and `buf+3` name one byte, and a
/// store spelled the second way changed what the first reads. memmove_overlap
/// -O2 read the new value (WRONG) after the copy to the cell was folded away.
#[test]
fn a_global_byte_stored_by_another_spelling_keeps_the_copy() {
    let code = "  lda buf+2+1\n  sta rc16\n  lda #9\n  sta buf+3\n  lda rc16\n  sta out\n  rts\n";
    assert_eq!(after_a_byte(code), code);
}
