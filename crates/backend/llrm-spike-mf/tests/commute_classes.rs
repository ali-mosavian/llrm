//! `commute_for_classes`: a commutable compare whose operand copies cannot join
//! their values in the order written, and can in the other, reads its sources
//! the other way round.

mod common;

use common::{op, v};
use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::Target;
use llrm_spike_mf::{Block, Inst, MFunction, Operand, commute_for_classes};

/// Classes are sets of places: 1 a register, 2 a memory cell; two classes meet
/// in what they share.
struct Masks;

impl Target for Masks {
    type Class = u8;

    fn meet(
        one: u8,
        two: u8,
    ) -> Option<u8> {
        Some(one & two).filter(|met| *met != 0)
    }

    fn any() -> u8 {
        3
    }

    fn candidates(_: u8) -> Vec<RegId> {
        vec![]
    }

    fn view(
        register: RegId,
        _: u8,
    ) -> RegId {
        register
    }

    fn root(register: RegId) -> RegId {
        register
    }

    fn reserved(_: RegId) -> bool {
        false
    }

    fn caller_saved() -> Vec<RegId> {
        vec![]
    }
}

const REGISTER: u8 = 1;
const CELL: u8 = 2;

fn compare(commutable: bool) -> Inst {
    let mut cmp = op("cmp", vec![Operand::Use(v(25)), Operand::Use(v(23))]);
    cmp.commutable = commutable;
    cmp
}

/// `x` (v8) is wanted in a register by one use, `y` (v9) in a cell by another,
/// and the compare reads `y` into its register slot (v25) and `x` into its cell
/// slot (v23): the `gcd` loop's closing `a != b` after its `a > b`. `wanted`
/// are the classes the other two uses ask of them.
fn gcd_like(
    commutable: bool,
    (x_wanted, y_wanted): (u8, u8),
) -> MFunction<u8> {
    let mut f = MFunction::new("f");
    f.classes = vec![3; 32];
    f.classes[30] = x_wanted;
    f.classes[31] = y_wanted;
    f.classes[25] = REGISTER;
    f.classes[23] = CELL;
    let insts = vec![
        Inst::copy(v(30), v(8)),
        Inst::copy(v(31), v(9)),
        Inst::copy(v(25), v(9)),
        Inst::copy(v(23), v(8)),
        compare(commutable),
    ];
    f.blocks.push(Block { id: 0, insts, phis: vec![] });
    f
}

/// The compare's operands and the classes of the two slot registers.
fn slots(f: &MFunction<u8>) -> (Vec<Operand>, (u8, u8)) {
    (f.blocks[0].insts[4].operands.clone(), (f.classes[25], f.classes[23]))
}

/// `x` wanted in a register and `y` in a cell, the compare asking the other
/// way: both slot copies were kept (`stx rc3; ldy rc2; cpy rc3` for `cpx rc2`).
#[test]
fn a_compare_reads_each_value_in_the_class_the_rest_of_the_function_gave_it() {
    let mut f = gcd_like(true, (REGISTER, CELL));
    assert_eq!(commute_for_classes::<Masks>(&mut f), 1);
    assert_eq!(slots(&f), (vec![Operand::Use(v(23)), Operand::Use(v(25))], (CELL, REGISTER)));
}

/// A compare something else reads the flags of (`bcs`) is not commutable: its
/// order is its meaning.
#[test]
fn a_compare_that_is_not_commutable_keeps_its_order() {
    let mut f = gcd_like(false, (REGISTER, CELL));
    assert_eq!(commute_for_classes::<Masks>(&mut f), 0);
    assert_eq!(slots(&f), (vec![Operand::Use(v(25)), Operand::Use(v(23))], (REGISTER, CELL)));
}

/// The order as written already fits: nothing moves.
#[test]
fn an_order_that_fits_stays() {
    let mut f = gcd_like(true, (CELL, REGISTER));
    assert_eq!(commute_for_classes::<Masks>(&mut f), 0);
}

/// Neither order fits (both values are wanted in a cell elsewhere): the order
/// as written stays, so no compare is made worse.
#[test]
fn an_order_that_neither_way_fits_stays() {
    let mut f = gcd_like(true, (CELL, CELL));
    assert_eq!(commute_for_classes::<Masks>(&mut f), 0);
    assert_eq!(slots(&f), (vec![Operand::Use(v(25)), Operand::Use(v(23))], (REGISTER, CELL)));
}
