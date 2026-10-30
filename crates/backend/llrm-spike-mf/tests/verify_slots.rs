//! `verify_sharing`: slot sharing may not change what any read of a slot sees.

use llrm_spike_mf::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_spike_mf::spill::verify_sharing;

fn store(
    register: u32,
    slot: i32,
) -> Inst {
    Inst::new(Kind::Op("STORE"), vec![Operand::Use(Reg::Virt(register)), Operand::Frame(slot)])
}

fn reload(
    register: u32,
    slot: i32,
) -> Inst {
    Inst::new(Kind::Op("RELOAD"), vec![Operand::Def(Reg::Virt(register)), Operand::Frame(slot)])
}

/// `register := register op slot`: a read of the slot inside an instruction.
fn folded(
    register: u32,
    slot: i32,
) -> Inst {
    Inst::new(
        Kind::Op("OP"),
        vec![Operand::Def(Reg::Virt(register)), Operand::Use(Reg::Virt(register)), Operand::Frame(slot)],
    )
}

fn function(insts: Vec<Inst>) -> MFunction<u8> {
    let mut function = MFunction::new("f");
    function.blocks.push(Block { id: 0, insts, phis: Vec::new() });
    function.classes = vec![0; 8];
    function
}

/// Every slot of `insts` renamed to cell 0.
fn one_cell(insts: &[Inst]) -> Vec<Inst> {
    insts
        .iter()
        .cloned()
        .map(|mut inst| {
            for operand in &mut inst.operands {
                if let Operand::Frame(slot) = operand {
                    *slot = 0;
                }
            }
            inst
        })
        .collect()
}

fn check(
    before: &[Inst],
    after: Vec<Inst>,
    kept: Vec<usize>,
) -> Result<(), String> {
    verify_sharing(&function(before.to_vec()), &[(0, 0), (1, 0)], &function(after), &[(0, 0)], &[kept])
}

/// Two values live at once in one cell: the first read sees the second store.
#[test]
fn two_values_live_at_once_may_not_share_a_cell() {
    let before = vec![store(0, 0), store(1, 1), reload(2, 0), reload(3, 1)];
    assert!(check(&before, one_cell(&before), vec![0, 1, 2, 3]).is_err());
}

#[test]
fn values_that_never_overlap_may_share_a_cell() {
    let before = vec![store(0, 0), reload(2, 0), store(1, 1), reload(3, 1)];
    assert_eq!(check(&before, one_cell(&before), vec![0, 1, 2, 3]), Ok(()));
}

/// A read folded into an instruction (`op r, slot`) followed by the store of
/// its result is not a copy of one slot into another: removing that store as a
/// self-copy once the two shared a cell left the next read with the old value
/// (184 of 240 differential programs).
#[test]
fn a_folded_read_and_the_store_of_its_result_are_not_a_copy() {
    let before = vec![store(0, 0), folded(1, 0), store(1, 1), reload(2, 1)];
    assert_eq!(check(&before, one_cell(&before), vec![0, 1, 2, 3]), Ok(()), "sharing alone is sound");
    let mut dropped = one_cell(&before);
    dropped.remove(2);
    assert!(check(&before, dropped, vec![0, 1, 3]).is_err(), "dropping the store is not");
}

/// A real copy (reload, store of the same register) may go when it lands in its
/// own cell.
#[test]
fn a_copy_into_its_own_cell_may_go() {
    let before = vec![store(0, 0), reload(1, 0), store(1, 1), reload(2, 1)];
    let mut after = one_cell(&before);
    after.drain(1..3);
    assert_eq!(check(&before, after, vec![0, 3]), Ok(()));
}
