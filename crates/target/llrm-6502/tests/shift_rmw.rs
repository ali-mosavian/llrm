//! `shift_in_place`: a shift of a byte in memory is done in memory.

use llrm_6502::asm::Asm;

/// popcount's `x >>= 1` on a cell: `lda rc2; lsr a; sta rc2` (5 bytes, 8
/// clocks) beside llvm-mos's `lsr __rc2` (2 bytes, 5 clocks); nothing formed
/// the in-place shift.
#[test]
fn a_shift_of_a_cell_stored_back_is_done_in_the_cell() {
    use llrm_6502::peephole::shift_in_place;
    for op in ["asl", "lsr", "rol", "ror"] {
        let mut asm = Asm::parse(&format!("l:\n  lda rc2\n  {op} a\n  sta rc2\n  bne l\n  lda #0\n  rts\n"));
        shift_in_place(&mut asm);
        assert_eq!(asm.text(), format!("l:\n  {op} rc2\n  bne l\n  lda #0\n  rts\n"));
    }
    // `a` is read after the store: the shifted value is still needed there
    let live = "  lda rc2\n  lsr a\n  sta rc2\n  rts\n";
    let mut asm = Asm::parse(live);
    shift_in_place(&mut asm);
    assert_eq!(asm.text(), live);
}
