//! A copy whose source is dead is the source: `k_shrax7` shifted a copy of rc3
//! (`lda rc3; sta rc4; asl rc2; rol rc4; ...; lda rc4`) though rc3 was never
//! read again, two bytes and a clock more than shifting rc3 (2026-11-08).

use llrm_6502::asm::Asm;
use llrm_6502::peephole::drop_dead_writes;
use llrm_6502::peephole_cells::coalesce_copy;

fn run(text: &str) -> String {
    let mut asm = Asm::parse(text);
    coalesce_copy(&mut asm);
    drop_dead_writes(&mut asm);
    asm.text()
}

#[test]
fn a_copy_shifted_in_place_is_the_dead_source_shifted() {
    assert_eq!(
        run("  lda rc3\n  sta rc4\n  asl rc2\n  rol rc4\n  lda #0\n  rol a\n  tax\n  lda rc4\n  rts\n"),
        "  asl rc2\n  rol rc3\n  lda #0\n  rol a\n  tax\n  lda rc3\n  rts\n"
    );
}

/// The source is read after the window: it is not dead.
#[test]
fn a_live_source_keeps_its_copy() {
    let text = "  lda rc3\n  sta rc4\n  rol rc4\n  lda rc4\n  adc rc3\n  rts\n";
    assert_eq!(run(text), text);
}

/// The copy is read on another path after the window.
#[test]
fn a_copy_read_after_a_branch_keeps_its_copy() {
    let text = "  lda rc3\n  sta rc4\n  rol rc4\n  bne L\n  rts\nL:\n  lda rc4\n  rts\n";
    assert_eq!(run(text), text);
}

/// A callee-saved cell is never taken as the copy's home.
#[test]
fn a_callee_saved_source_keeps_its_copy() {
    let text = "  lda rc8\n  sta rc4\n  rol rc4\n  lda rc4\n  rts\n";
    assert_eq!(run(text), text);
}

/// A pointer made of the copy reads two cells: the copy stays.
#[test]
fn a_pointer_through_the_copy_keeps_its_copy() {
    let text = "  lda rc2\n  sta rc4\n  ldy #0\n  lda (rc4),y\n  rts\n";
    assert_eq!(run(text), text);
}
