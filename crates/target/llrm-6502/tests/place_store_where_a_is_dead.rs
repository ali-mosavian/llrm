//! A cell made from a literal or another cell while `a` is live costs a
//! `pha`/`pla` bracket; where `a` is dead earlier in the run, the store goes there.

use llrm_6502::asm::Asm;
use llrm_6502::peephole::place_store_where_a_is_dead;

fn placed(text: &str) -> String {
    let mut asm = Asm::parse(text);
    place_store_where_a_is_dead(&mut asm);
    asm.text()
}

/// `known_loads_walks` -O1: `pha; lda #255; sta rc3; pla` 576 times, 12 clocks
/// each; above the `lda` that makes `a` live it is 5.
#[test]
fn a_literal_store_goes_above_the_load_that_makes_a_live() {
    assert_eq!(
        placed("  lda rc9\n  ldx rc2\n  pha\n  lda #255\n  sta rc3\n  pla\n  sta rc4\n  rts\n"),
        "  lda #255\n  sta rc3\n  lda rc9\n  ldx rc2\n  sta rc4\n  rts\n"
    );
}

/// A cell copy moves with its source.
#[test]
fn a_cell_copy_moves_when_nothing_writes_its_source() {
    assert_eq!(
        placed("  lda rc9\n  pha\n  lda rc5\n  sta rc3\n  pla\n  sta rc4\n  rts\n"),
        "  lda rc5\n  sta rc3\n  lda rc9\n  sta rc4\n  rts\n"
    );
}

#[test]
fn a_cell_copy_stays_when_its_source_is_written_between() {
    let text = "  lda rc9\n  inc rc5\n  pha\n  lda rc5\n  sta rc3\n  pla\n  sta rc4\n  rts\n";
    assert_eq!(placed(text), text);
}

#[test]
fn a_store_stays_when_the_run_reads_its_cell() {
    let text = "  lda rc9\n  ldx rc3\n  pha\n  lda #255\n  sta rc3\n  pla\n  sta rc4\n  rts\n";
    assert_eq!(placed(text), text);
}

/// A pointer in `rc2`:`rc3` reads `rc3`.
#[test]
fn a_store_stays_when_the_run_reads_it_through_a_pointer() {
    let text = "  lda rc9\n  ldy #0\n  ora (rc2),y\n  pha\n  lda #255\n  sta rc3\n  pla\n  sta rc4\n  rts\n";
    assert_eq!(placed(text), text);
}

/// `a` holds an argument on entry: nothing earlier makes it dead.
#[test]
fn a_store_stays_when_a_is_live_from_the_start() {
    let text = "  pha\n  lda #255\n  sta rc3\n  pla\n  sta rc4\n  rts\n";
    assert_eq!(placed(text), text);
}

/// A label is a way in with `a` live.
#[test]
fn a_store_stays_below_a_label() {
    let text = "  lda rc9\nL:\n  pha\n  lda #255\n  sta rc3\n  pla\n  sta rc4\n  rts\n";
    assert_eq!(placed(text), text);
}

/// The restored `a` set the flags the branch reads.
#[test]
fn a_store_stays_when_a_branch_reads_the_flags_of_the_restore() {
    let text = "  lda rc9\n  ldx rc2\n  pha\n  lda #255\n  sta rc3\n  pla\n  beq L\nL:\n  rts\n";
    assert_eq!(placed(text), text);
}

#[test]
fn a_call_in_the_run_stops_the_move() {
    let text = "  lda rc9\n  jsr f\n  pha\n  lda #255\n  sta rc3\n  pla\n  sta rc4\n  rts\n";
    assert_eq!(placed(text), text);
}
