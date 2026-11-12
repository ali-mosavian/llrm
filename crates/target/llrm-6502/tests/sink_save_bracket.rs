//! A literal made into a cell while `a` is live is stored where `a` is dead.

use llrm_6502::asm::Asm;
use llrm_6502::peephole::{run, sink_save_bracket};

fn sunk(text: &str) -> String {
    let mut asm = Asm::parse(text);
    sink_save_bracket(&mut asm);
    asm.text()
}

/// `idioms` -O2: `pha; lda #16; sta rc2; pla; sta L; lda #16`. Seventy-eight
/// brackets of 12 clocks, each next to a store of `a` and a load.
#[test]
fn the_bracket_follows_the_stores_of_a_to_the_next_load() {
    assert_eq!(
        sunk("  txa\n  pha\n  lda #16\n  sta rc2\n  pla\n  sta L\n  lda #7\n  sta L+1\n  rts\n"),
        "  txa\n  sta L\n  lda #16\n  sta rc2\n  lda #7\n  sta L+1\n  rts\n"
    );
}

/// `word_compares` -O2: the value `a` holds is made by `lda`, `sbc`, `and`;
/// the bracket goes above the load that makes it, past the carry chain.
#[test]
fn the_bracket_precedes_the_load_that_made_a_live() {
    assert_eq!(
        sunk("  lda rc3\n  sbc rc4\n  pha\n  lda #8\n  sta rc9\n  pla\n  and #1\n  sta rc6\n  rts\n"),
        "  lda #8\n  sta rc9\n  lda rc3\n  sbc rc4\n  and #1\n  sta rc6\n  rts\n"
    );
}

/// Code that reads the cell on both sides, or branches on both, keeps the
/// bracket: moved, it would be read before it is set, or skipped.
#[test]
fn the_bracket_stays_where_the_cell_is_used_or_control_leaves() {
    let read = "  lda rc5
  adc rc2
  pha
  lda #16
  sta rc2
  pla
  adc rc2
  rts
";
    assert_eq!(sunk(read), read);
    let branch = "  beq l
  pha
  lda #16
  sta rc2
  pla
  beq m
  lda #7
m:
  rts
";
    assert_eq!(sunk(branch), branch);
}

/// The whole pass list, so a regression in the order shows.
#[test]
fn the_pass_list_applies_it() {
    let mut asm = Asm::parse("  txa\n  pha\n  lda #16\n  sta rc2\n  pla\n  sta L\n  lda #7\n  sta L+1\n  rts\n");
    run(&mut asm);
    assert!(!asm.text().contains("pha"), "{}", asm.text());
}
