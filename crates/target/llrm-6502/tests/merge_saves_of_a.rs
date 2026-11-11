//! Two brackets of `a` that follow one another are one.

use llrm_6502::asm::Asm;
use llrm_6502::peephole::{merge_saves_of_a, run};

fn merged(text: &str) -> String {
    let mut asm = Asm::parse(text);
    merge_saves_of_a(&mut asm);
    asm.text()
}

/// `known_loads_walks` -O1, two stores of 255 into cells while `a` is live: 24
/// clocks a trip, 576 trips. The restore and the save between them are dead.
#[test]
fn two_literal_stores_with_a_live_are_one_bracket() {
    assert_eq!(
        merged("  pha\n  lda #255\n  sta rc3\n  pla\n  pha\n  lda #255\n  sta rc4\n  pla\n  rts\n"),
        "  pha\n  lda #255\n  sta rc3\n  lda #255\n  sta rc4\n  pla\n  rts\n"
    );
}

/// The whole pass list: the second `lda #255` is then a known load.
#[test]
fn the_second_load_of_the_literal_goes() {
    let mut asm =
        Asm::parse("  pha\n  lda #255\n  sta rc3\n  pla\n  pha\n  lda #255\n  sta rc4\n  pla\n  jsr h\n  rts\n");
    run(&mut asm);
    assert_eq!(asm.text(), "  pha\n  lda #255\n  sta rc3\n  sta rc4\n  pla\n  jsr h\n  rts\n");
}

/// Three in a row are one.
#[test]
fn a_run_of_brackets_is_one() {
    assert_eq!(
        merged(
            "  pha\n  lda #1\n  sta rc3\n  pla\n  pha\n  lda #2\n  sta rc4\n  pla\n  pha\n  lda #3\n  sta rc5\n  pla\n  rts\n"
        ),
        "  pha\n  lda #1\n  sta rc3\n  lda #2\n  sta rc4\n  lda #3\n  sta rc5\n  pla\n  rts\n"
    );
}

/// The second body reads `a` before it writes it: it needs the restore.
#[test]
fn a_body_that_reads_a_first_keeps_the_restore() {
    let text = "  pha\n  lda #1\n  sta rc3\n  pla\n  pha\n  sta rc4\n  pla\n  rts\n";
    assert_eq!(merged(text), text);
}

/// The second body calls: the stack is not as the first left it.
#[test]
fn a_body_with_a_call_keeps_the_restore() {
    let text = "  pha\n  lda #1\n  sta rc3\n  pla\n  pha\n  lda #2\n  jsr f\n  pla\n  rts\n";
    assert_eq!(merged(text), text);
}

/// A label between the brackets is a way in with `a` restored.
#[test]
fn a_label_between_the_brackets_keeps_the_restore() {
    let text = "  pha\n  lda #1\n  sta rc3\n  pla\nL:\n  pha\n  lda #2\n  sta rc4\n  pla\n  rts\n";
    assert_eq!(merged(text), text);
}
