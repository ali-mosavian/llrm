//! Dead cell stores at a return, and a word shift that ends in `a`.

use llrm_6502::asm::Asm;
use llrm_6502::peephole_cells::{chain_ends_in_a, drop_dead_stores, store_then_index_load};

fn asm(
    text: &str,
    results: &[&str],
) -> Asm {
    let mut asm = Asm::parse(text);
    asm.result_cells = results.iter().map(|c| (*c).to_owned()).collect();
    asm
}

/// The `sta rc2` after a shift run left `a` as the result: 2 bytes.
#[test]
fn a_store_to_a_cell_the_return_does_not_read_is_dropped() {
    let mut a = asm("  sta rc2\n  ldx rc3\n  rts\n", &[]);
    drop_dead_stores(&mut a);
    assert_eq!(a.text(), "  ldx rc3\n  rts\n");
}

/// `rc2` is a long result's byte; `rc8` is the caller's; neither is dead.
#[test]
fn a_store_to_a_returned_or_callee_saved_cell_stays() {
    for (text, results) in [("  sta rc2\n  rts\n", &["rc2"][..]), ("  sta rc8\n  rts\n", &[])] {
        let mut a = asm(text, results);
        drop_dead_stores(&mut a);
        assert_eq!(a.text(), text);
    }
}

/// `sta rc3; lda rc2; ldx rc3` went through memory to reach `x`.
#[test]
fn a_byte_stored_and_loaded_into_x_is_transferred() {
    let mut a = asm("  sta rc3\n  lda rc2\n  ldx rc3\n  rts\n", &[]);
    store_then_index_load(&mut a);
    assert_eq!(a.text(), "  tax\n  lda rc2\n  rts\n");
}

#[test]
fn a_shift_run_whose_low_byte_is_loaded_next_shifts_in_a() {
    let mut a = asm("  lsr rc3\n  ror rc2\n  lsr rc3\n  ror rc2\n  lda rc2\n  ldx rc3\n  rts\n", &[]);
    chain_ends_in_a(&mut a);
    assert_eq!(a.text(), "  lda rc2\n  lsr rc3\n  ror a\n  lsr rc3\n  ror a\n  ldx rc3\n  rts\n");
}
