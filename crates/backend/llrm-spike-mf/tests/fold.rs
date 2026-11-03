//! `branch_fold` over small layouts.

mod common;

use common::{Two, branch, op, v};
use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::Assignment;
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, branch_fold, branch_fold_hoisting};

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

fn cond(to: u32) -> Inst {
    terminator("branch", false, Some(to))
}

/// A compare chain, as a switch lowers: each block tests and branches to a
/// return, and the others jump to one shared default return. Folding it never
/// returned (blocks 2 and 5 both pulled the default next to themselves,
/// forever); the 6502 `switch` test hung on it.
#[test]
fn a_chain_of_tests_sharing_one_default_terminates() {
    let blocks = vec![
        vec![cond(3), jump(1)],
        vec![cond(10), jump(2)],
        vec![cond(6), jump(11)],
        vec![cond(7), jump(4)],
        vec![cond(8), jump(5)],
        vec![cond(9), jump(11)],
        vec![ret()],
        vec![ret()],
        vec![ret()],
        vec![ret()],
        vec![ret()],
        vec![ret()],
    ];
    let (done, wait) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut function = layout(blocks);
        branch_fold::<Two>(&mut function);
        let _ = done.send(function.blocks.len());
    });
    let blocks = wait.recv_timeout(std::time::Duration::from_secs(5)).expect("branch_fold does not return");
    assert!(blocks <= 12);
}

/// A branch on the flags `compare` sets, over blocks `1` and `2`.
fn guard(
    compare: Inst,
    ways: [Vec<Inst>; 2],
) -> MFunction<()> {
    let mut branch = cond(2);
    branch.reads_flags = true;
    let [one, two] = ways;
    let mut function = layout(vec![vec![compare, branch], one, two]);
    function.classes = vec![(); 8];
    function
}

fn compare(reg: u32) -> Inst {
    let mut inst = op("compare", vec![Operand::Use(v(reg))]);
    inst.sets_flags = true;
    inst
}

fn load_constant(
    reg: u32,
    value: i64,
) -> Inst {
    op("constant", vec![Operand::Def(v(reg)), Operand::Imm(value)])
}

/// `v1` and `v2` are the register `1`; `v0` is `0`, `v3` is `2`.
fn registers() -> Assignment {
    Assignment([0, 1, 1, 2, 3, 3].map(RegId::from_raw).to_vec())
}

fn first_block(function: &MFunction<()>) -> Vec<String> {
    shape(function).remove(0).1
}

/// A guard over a loop whose both ways begin with the same constant (crc16's
/// `moveq #0, %d0; not.w %d0`, once in the loop's entry and once in the early
/// return) loaded it twice; one copy runs ahead of the compare.
#[test]
fn the_constant_both_ways_begin_with_runs_before_the_compare() {
    let mut function = guard(
        compare(0),
        [
            vec![load_constant(1, 65535), op("loop", vec![Operand::Use(v(1))]), ret()],
            vec![load_constant(2, 65535), ret()],
        ],
    );
    assert!(branch_fold_hoisting::<Two>(&mut function, &registers()));
    assert_eq!(first_block(&function), names(&["constant", "compare", "branch 2"]));
    assert_eq!(shape(&function)[1].1, names(&["loop", "ret"]));
    assert_eq!(shape(&function)[2].1, names(&["ret"]));
    assert!(!branch_fold::<Two>(&mut guard(compare(0), [vec![ret()], vec![ret()]])));
}

/// A constant into the register the compare reads would change what it
/// compares.
#[test]
fn an_instruction_that_writes_what_the_compare_reads_stays() {
    let mut function = guard(compare(0), [vec![load_constant(0, 1), ret()], vec![load_constant(0, 1), ret()]]);
    branch_fold_hoisting::<Two>(&mut function, &registers());
    assert_eq!(first_block(&function), names(&["compare", "branch 2"]));
}

/// Different registers or different values are not one instruction.
#[test]
fn instructions_that_differ_stay() {
    let mut function = guard(compare(0), [vec![load_constant(1, 1), ret()], vec![load_constant(3, 1), ret()]]);
    branch_fold_hoisting::<Two>(&mut function, &registers());
    assert_eq!(first_block(&function), names(&["compare", "branch 2"]));
    let mut function = guard(compare(0), [vec![load_constant(1, 1), ret()], vec![load_constant(2, 2), ret()]]);
    branch_fold_hoisting::<Two>(&mut function, &registers());
    assert_eq!(first_block(&function), names(&["compare", "branch 2"]));
}

/// Where another block also enters a way, the instruction is not both ways'.
#[test]
fn a_way_with_another_entry_keeps_its_instructions() {
    let mut function =
        guard(compare(0), [vec![load_constant(1, 1), cond(2), jump(3)], vec![load_constant(2, 1), ret()]]);
    function.blocks.push(Block { id: 3, insts: vec![ret()], phis: vec![] });
    branch_fold_hoisting::<Two>(&mut function, &registers());
    assert_eq!(first_block(&function), names(&["compare", "branch 2"]));
}

/// An instruction that sets the flags may run ahead of the compare when
/// neither way reads them after it; one whose flags are read may not.
#[test]
fn an_instruction_that_sets_flags_hoists_only_where_they_are_dead() {
    let adds = |reg| {
        let mut inst = op("add", vec![Operand::Def(v(reg)), Operand::Imm(1)]);
        inst.sets_flags = true;
        inst
    };
    let mut reads = op("reads", vec![]);
    reads.reads_flags = true;
    let mut function = guard(compare(0), [vec![adds(1), ret()], vec![adds(2), ret()]]);
    branch_fold_hoisting::<Two>(&mut function, &registers());
    assert_eq!(first_block(&function), names(&["add", "compare", "branch 2"]));
    let mut function = guard(compare(0), [vec![adds(1), reads.clone(), ret()], vec![adds(2), ret()]]);
    branch_fold_hoisting::<Two>(&mut function, &registers());
    assert_eq!(first_block(&function), names(&["compare", "branch 2"]));
}

/// m68k seed 227 returned 56 for 85: both ways began with a copy that is a
/// `move` into a data register, which ran ahead of the branch, between the
/// `sne` and the `beq` that read one compare's codes, and the branch took the
/// `move`'s.
#[test]
fn a_copy_that_sets_flags_stays_below_the_branch_that_reads_them() {
    let copy = |to| Inst::new(Kind::Copy, vec![Operand::Def(v(to)), Operand::Use(v(0))]);
    let mut set = op("set", vec![Operand::Def(v(3))]);
    set.reads_flags = true;
    let mut function = guard(compare(0), [vec![copy(1), ret()], vec![copy(2), ret()]]);
    function.blocks[0].insts.insert(1, set);
    branch_fold_hoisting::<Two>(&mut function, &registers());
    assert_eq!(first_block(&function), names(&["compare", "set", "branch 2"]));
}
