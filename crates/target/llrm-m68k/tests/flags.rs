//! `preserve_flags` on hand-built functions: a copy between the instruction
//! that set the condition codes and the branch that reads them is a `move`
//! into a data register, which sets them again.

use llrm_m68k::flags::preserve_flags;
use llrm_m68k::mfunc::MFunction;
use llrm_m68k::reg;
use llrm_m68k::regalloc::Assignment;
use llrm_spike_mf::{Block, Inst, Kind, Operand, Reg};

fn v(n: u32) -> Reg {
    Reg::Virt(n)
}

fn op(
    name: &'static str,
    operands: Vec<Operand>,
    sets: bool,
    reads: bool,
) -> Inst {
    let mut inst = Inst::new(Kind::Op(name), operands);
    inst.sets_flags = sets;
    inst.reads_flags = reads;
    inst
}

fn function(insts: Vec<Inst>) -> MFunction {
    let mut function = MFunction::new("f");
    function.blocks.push(Block { id: 0, insts, phis: Vec::new() });
    function
}

/// d0..d3 for v0..v3.
fn assignment() -> Assignment {
    Assignment(vec![reg::D0, reg::D1, reg::D2, reg::D3])
}

fn kinds(function: &MFunction) -> Vec<Kind> {
    function.blocks[0].insts.iter().map(|inst| inst.kind).collect()
}

/// `cmp; move d2,d3; bne` branched on the `move`'s codes, not the compare's.
#[test]
fn a_copy_that_does_not_depend_on_the_compare_moves_above_it() {
    let mut f = function(vec![
        op("CMP32dr", vec![Operand::Use(v(0)), Operand::Use(v(1))], true, false),
        Inst::copy(v(2), v(3)),
        op("Bne8", vec![], false, true),
    ]);
    preserve_flags(&mut f, &assignment()).unwrap();
    assert_eq!(kinds(&f), [Kind::Copy, Kind::Op("CMP32dr"), Kind::Op("Bne8")]);
}

/// `add d0; move d2,d0; bne` branched on the copy's codes; the result is
/// tested again right before the branch.
#[test]
fn a_copy_of_the_setters_result_is_followed_by_a_test_of_it() {
    let mut f = function(vec![
        op("ADD32dr", vec![Operand::Def(v(0)), Operand::Use(v(0)), Operand::Use(v(1))], true, false),
        Inst::copy(v(2), v(0)),
        op("Bne8", vec![], false, true),
    ]);
    preserve_flags(&mut f, &assignment()).unwrap();
    assert_eq!(kinds(&f), [Kind::Op("ADD32dr"), Kind::Copy, Kind::Op("TST32d"), Kind::Op("Bne8")]);
}

/// A copy that overwrites an operand of the compare leaves nothing to compare
/// again: an error, not a branch on the wrong codes.
#[test]
fn a_copy_over_the_compares_operand_is_an_error() {
    let mut f = function(vec![
        op("CMP32dr", vec![Operand::Use(v(0)), Operand::Use(v(1))], true, false),
        Inst::copy(v(0), v(3)),
        op("Bne8", vec![], false, true),
    ]);
    assert!(preserve_flags(&mut f, &assignment()).is_err());
}
