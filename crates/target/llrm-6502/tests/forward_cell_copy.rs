//! A value copied to a cell and loaded again reads the cell it came from.
//! `k_mulax3` was 24 B: `lda rc2; sta rc4; lda rc3; sta rc5; lda rc4; asl a`,
//! the copy of the low byte stored and loaded back for nothing (2026-11-08).

use llrm_6502::asm::Asm;
use llrm_6502::peephole::drop_dead_writes;
use llrm_6502::peephole_cells::forward_cell_copy;

fn run(text: &str) -> String {
    let mut asm = Asm::parse(text);
    forward_cell_copy(&mut asm);
    drop_dead_writes(&mut asm);
    asm.text()
}

#[test]
fn a_copy_loaded_back_after_other_work_reads_its_source() {
    assert_eq!(
        run("  lda rc2\n  sta rc4\n  lda rc3\n  sta rc5\n  lda rc4\n  asl a\n  rol rc5\n  clc\n  adc rc2\n  rts\n"),
        "  lda rc3\n  sta rc5\n  lda rc2\n  asl a\n  rol rc5\n  clc\n  adc rc2\n  rts\n"
    );
}

/// The source changes between the store and the load: the copy is the old
/// value.
#[test]
fn a_copy_stays_when_its_source_is_written_before_the_load() {
    let text = "  lda rc2\n  sta rc4\n  lda #1\n  sta rc2\n  lda rc4\n  rts\n";
    assert_eq!(run(text), text);
}

/// The copy is read after a branch target: not known dead.
#[test]
fn a_copy_stays_when_it_is_read_on_another_path() {
    let text = "  lda rc2\n  sta rc4\n  lda rc3\n  bne L\n  lda rc4\n  rts\nL:\n  lda rc4\n  rts\n";
    assert_eq!(run(text), text);
}

/// A load through a pointer in other cells does not stop the forwarding.
#[test]
fn a_load_through_another_pointer_does_not_stop_it() {
    assert_eq!(
        run("  lda rc2\n  sta rc4\n  ldy #0\n  lda (rc6),y\n  adc rc4\n  rts\n"),
        "  ldy #0\n  lda (rc6),y\n  adc rc2\n  rts\n"
    );
}

/// The pointer is the copy's own pair: the load reads the copy.
#[test]
fn a_pointer_made_of_the_copy_stops_it() {
    let text = "  lda rc2\n  sta rc4\n  ldy #0\n  lda (rc4),y\n  rts\n";
    assert_eq!(run(text), text);
}
