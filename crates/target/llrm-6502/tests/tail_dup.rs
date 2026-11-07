//! A return block short enough to stand where the jump to it stands is copied
//! there.

use llrm_6502::asm::Asm;
use llrm_6502::peephole::run;

fn after(code: &str) -> String {
    let mut asm = Asm::parse(code);
    run(&mut asm);
    asm.text()
}

/// `dotx`: the zero-trip path stored its result and jumped to the return that
/// loads it, `lda #0; sta rc7; jmp b3` (7 B) for `lda #0; rts`.
#[test]
fn a_zero_trip_path_returns_its_constant() {
    let before = "  ldx rc6\n  bne b21\nb23:\n  lda #0\n  sta rc7\n  jmp b3\nb21:\n  lda #0\n  sta rc7\n  tay\nb2:\n  lda (rc4),y\n  clc\n  adc rc7\n  sta rc7\n  iny\n  cpy rc6\n  bne b2\nb3:\n  lda rc7\n  rts\n";
    let want = "  ldx rc6\n  bne b21\nb23:\n  lda #0\n  rts\nb21:\n  lda #0\n  sta rc7\n  tay\nb2:\n  lda (rc4),y\n  clc\n  adc rc7\n  sta rc7\n  iny\n  cpy rc6\n  bne b2\nb3:\n  lda rc7\n  rts\n";
    assert_eq!(after(before), want);
}

/// A block longer than the jump stays a jump (size never grows), and a block
/// with a branch is not a return block.
#[test]
fn a_longer_or_branching_block_stays_a_jump() {
    let long = "  jmp b3\nb2:\n  lda #1\n  rts\nb3:\n  lda rc7\n  sta rc8\n  rts\n";
    assert!(after(long).starts_with("  jmp b3\n"), "{}", after(long));
    let branching = "  jmp b3\nb2:\n  lda #1\n  rts\nb3:\n  lda rc7\n  beq b2\n  rts\n";
    assert!(after(branching).starts_with("  jmp b3\n"), "{}", after(branching));
}
