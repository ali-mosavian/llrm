//! The coalescer remakes a constant in place of a copy only where that is not
//! dearer.

use llrm_6502::alloc::Mos;
use llrm_6502::reg;
use llrm_codegen::model::ir::{Held, Imm, Loc, Operation, Semantics};
use llrm_codegen::model::lir::Insn;
use llrm_spike_mf::regalloc::Target;

fn zero() -> Insn {
    let what = Semantics {
        name: Some("mov".into()),
        dests: vec![Loc::Held(Held { value: 1, width: 1, offset: 0 })],
        sources: vec![Loc::Imm(Imm { value: 0, width: 1, address: None })],
        ..Semantics::new(Operation::Move)
    };
    Insn::new(0, None, Some(what), vec![1], vec![])
}

/// `-Os` made a zero again with `lda #0` (2 B) where `txa` (1 B) copied it:
/// tests/run/c/shift_int32_variable.c +3 B once the coalescer's size check was
/// gone, because the 6502 priced nothing and the remat always won.
#[test]
fn a_zero_in_a_register_is_not_made_again_where_a_transfer_copies_it() {
    let (zero, x, a) = (zero(), [reg::X], [reg::A]);
    assert!(!Mos::<false>::literal_move_wins(&zero, &x, &a, true), "size: 2 B against 1 B");
    assert!(
        !Mos::<false>::literal_move_wins(&zero, &x, &a, false),
        "speed: 2 clocks against 2 clocks is a tie, the copy keeps it"
    );
}

#[test]
fn a_zero_in_a_cell_is_made_again_only_against_a_cell_to_cell_copy() {
    let (zero, a, cell, other) = (zero(), [reg::A], [reg::RC2], [reg::RC3]);
    assert!(!Mos::<false>::literal_move_wins(&zero, &a, &cell, true), "sta rc is 2 B, lda #0 + sta rc is 4");
    assert!(
        Mos::<false>::literal_move_wins(&zero, &other, &cell, true),
        "lda rc + sta rc is 4 B, the same as the remat"
    );
    assert!(Mos::<false>::literal_move_wins(&zero, &other, &cell, false), "6 clocks against 5");
}

/// arm64 `mul_constants` -Os grew 124 B when the coalescer asked only the target's
/// price: a constant that stays read is kept as a copy at -Os (DEVIATE C10), which
/// the price of a cell-to-cell copy (it says remake) does not know.
#[test]
fn a_constant_that_stays_read_is_kept_as_a_copy_at_size_whatever_the_price_says() {
    use llrm_spike_mf::tolir::literal_remat_wins;
    let (cell, other) = ([reg::RC2], [reg::RC3]);
    let zero = zero();
    assert!(Mos::<false>::literal_move_wins(&zero, &other, &cell, true), "the price alone says remake");
    assert!(!literal_remat_wins::<Mos<false>>(&zero, &other, &cell, true, false), "it stays read");
    assert!(literal_remat_wins::<Mos<false>>(&zero, &other, &cell, true, true), "its own move goes");
    assert!(literal_remat_wins::<Mos<false>>(&zero, &other, &cell, false, false), "at speed the price decides");
}
