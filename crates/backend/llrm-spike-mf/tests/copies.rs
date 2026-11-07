//! `remove_redundant_copies`: a copy back into the register another copy
//! filled from it.

mod common;

use common::{Two, op, v};
use llrm_lir::registers::RegId;
use llrm_spike_mf::{Assignment, Block, Inst, Kind, MFunction, Operand, remove_redundant_copies};

/// `v0` and `v2` in r0, `v1` in r1.
fn run(
    insts: Vec<Inst>,
    wide: bool,
) -> Vec<Kind> {
    let mut function = MFunction::new("f");
    function.classes = vec![(); 3];
    function.blocks.push(Block { id: 0, insts, phis: vec![] });
    let assignment = Assignment(vec![RegId::from_raw(0), RegId::from_raw(1), RegId::from_raw(0)]);
    remove_redundant_copies::<Two>(&mut function, &assignment, |_| wide);
    function.blocks[0].insts.iter().map(|inst| inst.kind).collect()
}

/// strlen's loop kept `move.l %d0, %d1; move.l %d1, %d0`: the second moves a
/// value into the register that already holds it.
#[test]
fn a_copy_back_into_the_source_is_dropped() {
    let kinds = run(vec![Inst::copy(v(1), v(0)), Inst::copy(v(0), v(1))], true);
    assert_eq!(kinds, [Kind::Copy]);
}

/// Once the source is written the copy back is a real one.
#[test]
fn a_copy_back_after_the_source_changed_stays() {
    let kinds = run(vec![Inst::copy(v(1), v(0)), op("w", vec![Operand::Def(v(2))]), Inst::copy(v(2), v(1))], true);
    assert_eq!(kinds, [Kind::Copy, Kind::Op("w"), Kind::Copy]);
}

/// A byte or word copy leaves the rest of the destination: not the same value.
#[test]
fn a_narrow_copy_back_stays() {
    let kinds = run(vec![Inst::copy(v(1), v(0)), Inst::copy(v(0), v(1))], false);
    assert_eq!(kinds, [Kind::Copy, Kind::Copy]);
}

/// `v0`, `v2` in r0; `v1` in r1. Block 0 branches to 1 (and 2, when `other`
/// also reaches it); the instructions of each are given.
fn blocks(
    first: Vec<Inst>,
    second: Vec<Inst>,
    other: bool,
) -> Vec<Vec<Kind>> {
    use llrm_spike_mf::Jump;
    let mut function = MFunction::new("f");
    function.classes = vec![(); 3];
    let mut a = first;
    a.push(<Two as Jump>::jump(1));
    function.blocks.push(Block { id: 0, insts: a, phis: vec![] });
    function.blocks.push(Block { id: 1, insts: second, phis: vec![] });
    if other {
        function.blocks.push(Block { id: 2, insts: vec![<Two as Jump>::jump(1)], phis: vec![] });
    }
    let assignment = Assignment(vec![RegId::from_raw(0), RegId::from_raw(1), RegId::from_raw(0)]);
    remove_redundant_copies::<Two>(&mut function, &assignment, |_| true);
    function.blocks.iter().map(|b| b.insts.iter().map(|inst| inst.kind).collect()).collect()
}

/// m68k x87_arguments -Os: the block `split_flag_edges` puts on the exit edge
/// repeated the pair copy its predecessor made for the header phi (`move.l
/// %d6, %a2; move.l %d7, %a3` twice, +4 B). The only block that reaches it has
/// made the copy and not written either register since.
#[test]
fn the_copy_a_sole_predecessor_made_is_not_repeated() {
    let got = blocks(vec![Inst::copy(v(1), v(0))], vec![Inst::copy(v(1), v(0))], false);
    assert_eq!(got[1], [], "the successor repeats the copy: {got:?}");
}

/// Another way in: the copy was not made on every path.
#[test]
fn a_copy_is_repeated_in_a_block_with_two_ways_in() {
    let got = blocks(vec![Inst::copy(v(1), v(0))], vec![Inst::copy(v(1), v(0))], true);
    assert_eq!(got[1], [Kind::Copy], "{got:?}");
}

/// The register was written between: the copy is a real one.
#[test]
fn a_copy_after_the_source_changed_in_the_predecessor_stays() {
    let got =
        blocks(vec![Inst::copy(v(1), v(0)), op("w", vec![Operand::Def(v(2))])], vec![Inst::copy(v(1), v(0))], false);
    assert_eq!(got[1], [Kind::Copy], "{got:?}");
}
