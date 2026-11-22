//! inlined_tail_call_keeps_frame_stores -Os was 3 B over the bar: a 16-bit
//! load went `lda a; tay; lda b; tax; tya` (9 B, `ldx b; lda a` is 6), and a
//! zero stored through `(rc2),y` was made in `x` and moved back
//! (`ldx #0; ...; txa`, 3 B where `lda #0` is 2).

use llrm_6502::asm::Asm;
use llrm_6502::peephole::{load_pair_without_transfers, load_straight_into_a};

/// A function that returns a word in a:x and leaves `y` nothing to read.
fn pair(text: &str) -> String {
    let mut asm = Asm::parse(text);
    asm.no_y_result = true;
    load_pair_without_transfers(&mut asm);
    asm.text()
}

/// A function that returns nothing in `x`.
fn straight(text: &str) -> String {
    let mut asm = Asm::parse(text);
    asm.no_x_result = true;
    load_straight_into_a(&mut asm);
    asm.text()
}

#[test]
fn a_word_load_through_y_is_two_loads() {
    let text = "  lda g\n  tay\n  lda g+1\n  tax\n  tya\n  rts\n";
    assert_eq!(pair(text), "  ldx g+1\n  lda g\n  rts\n");
}

#[test]
fn a_word_load_through_x_is_two_loads() {
    let text = "  lda g\n  tax\n  lda g+1\n  tay\n  txa\n  rts\n";
    let mut asm = Asm::parse(text);
    asm.no_x_result = true;
    load_pair_without_transfers(&mut asm);
    assert_eq!(asm.text(), "  ldy g+1\n  lda g\n  rts\n");
}

/// `y` holds the low byte after the original: a later read of it keeps the
/// transfers.
#[test]
fn a_word_load_stays_where_the_spare_register_is_read_after() {
    let text = "  lda g\n  tay\n  lda g+1\n  tax\n  tya\n  sty rc4\n  rts\n";
    assert_eq!(pair(text), text);
}

/// The first load indexed by `x` would read the new `x`.
#[test]
fn a_word_load_stays_where_the_first_load_is_indexed_by_the_new_register() {
    let text = "  lda g,x\n  tay\n  lda g+1\n  tax\n  tya\n  rts\n";
    assert_eq!(pair(text), text);
}

#[test]
fn a_zero_made_in_x_for_a_store_through_a_pointer_is_made_in_a() {
    let text = "  lda #5\n  ldx #0\n  ldy #0\n  sta (rc2),y\n  txa\n  ldy #1\n  sta (rc2),y\n  rts\n";
    assert_eq!(straight(text), "  lda #5\n  ldy #0\n  sta (rc2),y\n  lda #0\n  ldy #1\n  sta (rc2),y\n  rts\n");
}

/// A load from memory may be changed by a store through a pointer: it stays.
#[test]
fn a_memory_load_stays_across_a_store_through_a_pointer() {
    let text = "  lda #5\n  ldx g\n  ldy #0\n  sta (rc2),y\n  txa\n  rts\n";
    assert_eq!(straight(text), text);
}
