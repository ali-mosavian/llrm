//! A store to an argument cell (`rc2`..`rc7`) before a call that is not passed
//! that cell is dead where nothing after reads it: `rep_count` -Os kept `sta
//! rc5; sta rc7` in a loop whose exit reaches `jsr report`, which takes
//! a, x, rc2 and rc3 only (4 bytes, 4 clocks a trip).

use llrm_6502::asm::Asm;
use llrm_6502::peephole_cells::drop_dead_stores;

fn after(
    text: &str,
    callee: &str,
    passed: &[u8],
) -> String {
    let mut asm = Asm::parse(text);
    std::sync::Arc::make_mut(&mut asm.cell_callees).insert(callee.to_owned(), passed.iter().copied().collect());
    drop_dead_stores(&mut asm);
    asm.text()
}

#[test]
fn a_cell_the_call_is_not_passed_is_dead_before_it() {
    let text = "  sta rc5\n  lda #1\n  jsr report\n  rts\n";
    assert_eq!(after(text, "report", &[2, 3]), "  lda #1\n  jsr report\n  rts\n");
}

#[test]
fn a_cell_the_call_is_passed_stays() {
    let text = "  sta rc5\n  lda #1\n  jsr report\n  rts\n";
    assert_eq!(after(text, "report", &[2, 3, 5]), text);
}

/// A call to a callee nobody listed is as it was: any argument cell may be
/// read.
#[test]
fn a_callee_without_facts_reads_every_argument_cell() {
    let text = "  sta rc5\n  lda #1\n  jsr report\n  rts\n";
    assert_eq!(after(text, "other", &[2, 3]), text);
}
