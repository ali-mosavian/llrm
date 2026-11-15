//! A pair both paths of a branch copy is stored beside the register it was
//! loaded into, once.

use llrm_6502::asm::Asm;
use llrm_6502::peephole::fold_copy_into_store;

fn run(text: &str) -> String {
    let mut asm = Asm::parse(text);
    fold_copy_into_store(&mut asm);
    asm.text()
}

/// lz4's entry copied the destination to a saved pair through `x`, then both
/// successors copied the saved pair to the walking pointer (`lda rc6; sta
/// rc20; lda rc7; sta rc21`, 8 B each): the stores go beside the first copy.
#[test]
fn a_copy_at_the_start_of_both_paths_is_a_second_store() {
    let text = "  ldx rc4\n  stx rc6\n  ldx rc5\n  stx rc7\n  tax\n  bne L\n  lda rc6\n  sta rc20\n  lda rc7\n  sta rc21\n  lda #0\n  rts\nL:\n  lda rc6\n  sta rc20\n  lda rc7\n  sta rc21\n  lda #1\n  rts\n";
    assert_eq!(
        run(text),
        "  ldx rc4\n  stx rc6\n  stx rc20\n  ldx rc5\n  stx rc7\n  stx rc21\n  tax\n  bne L\n  lda #0\n  rts\nL:\n  lda #1\n  rts\n"
    );
}

/// The cell the copy writes is read between the store and the branch: the
/// early write would change what is read.
#[test]
fn a_copy_stays_when_its_target_is_read_before_the_branch() {
    let text =
        "  ldx rc4\n  stx rc6\n  lda rc20\n  bne L\n  lda rc6\n  sta rc20\n  rts\nL:\n  lda rc6\n  sta rc20\n  rts\n";
    assert_eq!(run(text), text);
}

/// One path reads `a` after the copy: the load stays.
#[test]
fn a_copy_stays_when_a_path_reads_a_after_it() {
    let text = "  ldx rc4\n  stx rc6\n  tax\n  bne L\n  lda rc6\n  sta rc20\n  rts\nL:\n  lda rc6\n  sta rc20\n  sta rc21\n  rts\n";
    assert_eq!(run(text), text);
}
