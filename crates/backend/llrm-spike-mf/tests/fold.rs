//! `branch_fold` over small layouts.

mod common;

use common::{Two, branch, op};
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, branch_fold};

fn terminator(
    name: &'static str,
    barrier: bool,
    to: Option<u32>,
) -> Inst {
    let mut inst = op(name, to.map(Operand::Block).into_iter().collect());
    inst.terminator = true;
    inst.barrier = barrier;
    inst
}

fn jump(to: u32) -> Inst {
    terminator("jump", true, Some(to))
}

fn ret() -> Inst {
    terminator("ret", true, None)
}

fn layout(blocks: Vec<Vec<Inst>>) -> MFunction<()> {
    let mut function = MFunction::new("f");
    for (id, insts) in blocks.into_iter().enumerate() {
        function.blocks.push(Block { id: id as u32, insts, phis: vec![] });
    }
    function
}

fn shape(function: &MFunction<()>) -> Vec<(u32, Vec<String>)> {
    function
        .blocks
        .iter()
        .map(|block| {
            let insts = block
                .insts
                .iter()
                .map(|inst| {
                    let Kind::Op(name) = inst.kind else { return "copy".to_owned() };
                    match inst.operands.first() {
                        Some(Operand::Block(to)) => format!("{name} {to}"),
                        _ => name.to_owned(),
                    }
                })
                .collect();
            (block.id, insts)
        })
        .collect()
}

fn names(items: &[&str]) -> Vec<String> {
    items.iter().map(|one| (*one).to_owned()).collect()
}

/// A branch to a block that only jumps goes to where that jumps, and the
/// block is dropped: `bra L3` through `L2: bra L3` cost a jump per trip.
#[test]
fn a_branch_to_a_block_that_only_jumps_goes_past_it() {
    let mut function = layout(vec![
        vec![terminator("branch", false, Some(2))],
        vec![ret()],
        vec![jump(3)],
        vec![op("work", vec![]), ret()],
    ]);
    assert!(branch_fold::<Two>(&mut function));
    // the reordering then lays block 3 out after the branch, reversed
    let shape = shape(&function);
    assert!(shape.iter().all(|(id, insts)| *id != 2 && !insts.iter().any(|inst| inst.ends_with(" 2"))), "{shape:?}");
}

/// A jump to the block laid out next is dropped.
#[test]
fn a_jump_to_the_next_block_is_dropped() {
    let mut function = layout(vec![vec![terminator("branch", false, Some(2)), jump(1)], vec![ret()], vec![ret()]]);
    assert!(branch_fold::<Two>(&mut function));
    assert_eq!(shape(&function)[0], (0, names(&["branch 2"])));
}

/// A conditional branch over the next block's return is reversed so the
/// common path falls through: `bgt L1; bra L2` became `ble L2`.
#[test]
fn a_conditional_branch_to_the_next_block_is_reversed() {
    let mut function =
        layout(vec![vec![terminator("branch", false, Some(1)), jump(2)], vec![op("work", vec![]), ret()], vec![ret()]]);
    assert!(branch_fold::<Two>(&mut function));
    assert_eq!(shape(&function)[0], (0, names(&["branch_not 2"])));
}

/// A fixed layout is left alone.
#[test]
fn a_layout_with_nothing_to_fold_stays() {
    let mut function = layout(vec![vec![terminator("branch", false, Some(2))], vec![ret()], vec![ret()]]);
    let before = shape(&function);
    assert!(!branch_fold::<Two>(&mut function));
    assert_eq!(shape(&function), before);
    let _ = branch;
}
