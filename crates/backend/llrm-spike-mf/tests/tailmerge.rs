//! `TailMergeBlocks` through `branch_fold`.

mod common;

use common::{Two, branch, op};
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, branch_fold};

fn jump(to: u32) -> Inst {
    let mut inst = op("jump", vec![Operand::Block(to)]);
    inst.terminator = true;
    inst.barrier = true;
    inst
}

fn ret() -> Inst {
    let mut inst = op("ret", vec![]);
    inst.terminator = true;
    inst.barrier = true;
    inst
}

fn work(name: &'static str) -> Inst {
    op(name, vec![])
}

fn layout(blocks: Vec<Vec<Inst>>) -> MFunction<()> {
    let mut function = MFunction::new("f");
    for (id, insts) in blocks.into_iter().enumerate() {
        function.blocks.push(Block { id: id as u32, insts, phis: vec![] });
    }
    function
}

/// How many instructions of the function are `name`.
fn count(
    function: &MFunction<()>,
    name: &str,
) -> usize {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .filter(|inst| matches!(inst.kind, Kind::Op(one) if one == name))
        .count()
}

/// Two returns ending in `y; z; ret` each kept their own copy: the tail was
/// emitted twice (the pin merges them; an 8-case switch was 6 B larger).
#[test]
fn blocks_with_no_successors_share_their_common_tail() {
    let mut function = layout(vec![
        vec![branch(2)],
        vec![work("x"), work("y"), work("z"), ret()],
        vec![work("p"), work("y"), work("z"), ret()],
    ]);
    assert!(branch_fold::<Two>(&mut function));
    assert_eq!((count(&function, "y"), count(&function, "z"), count(&function, "ret")), (1, 1, 1));
    assert_eq!((count(&function, "x"), count(&function, "p")), (1, 1));
}

/// Predecessors of a join that end in the same instructions share them; the
/// one that falls into the join keeps its copy as the shared one.
#[test]
fn predecessors_of_a_join_share_their_common_tail() {
    let mut function = layout(vec![
        vec![branch(2)],
        vec![work("x"), work("y"), work("z"), jump(3)],
        vec![work("p"), work("y"), work("z")],
        vec![ret()],
    ]);
    assert!(branch_fold::<Two>(&mut function));
    assert_eq!((count(&function, "y"), count(&function, "z")), (1, 1));
}

/// Two instructions in common are below the pin's minimum of three, and
/// neither block is all tail: nothing merges.
#[test]
fn a_two_instruction_tail_stays() {
    let mut function =
        layout(vec![vec![branch(2)], vec![work("x"), work("z"), ret()], vec![work("p"), work("z"), ret()]]);
    branch_fold::<Two>(&mut function);
    assert_eq!((count(&function, "z"), count(&function, "ret")), (2, 2));
}

/// Built for size, two instructions in common are enough when one block is
/// all tail: the merge cannot cost a branch.
#[test]
fn for_size_a_two_instruction_tail_that_is_a_whole_block_merges() {
    let blocks = || vec![vec![branch(2)], vec![work("p"), work("z"), ret()], vec![work("z"), ret()]];
    let mut speed = layout(blocks());
    branch_fold::<Two>(&mut speed);
    let mut size = layout(blocks());
    size.optimize_for_size = true;
    assert!(branch_fold::<Two>(&mut size));
    assert_eq!((count(&size, "z"), count(&size, "ret")), (1, 1));
}

fn cond(
    name: &'static str,
    to: u32,
    reads_flags: bool,
) -> Inst {
    let mut inst = op(name, vec![Operand::Block(to)]);
    inst.terminator = true;
    inst.reads_flags = reads_flags;
    inst
}

fn compare() -> Inst {
    let mut inst = op("compare", vec![]);
    inst.sets_flags = true;
    inst
}

/// The blocks control can go to from `at`: branch targets and the fall-through.
fn successors(
    function: &MFunction<()>,
    at: usize,
) -> Vec<usize> {
    let block = &function.blocks[at];
    let mut out: Vec<usize> = block
        .insts
        .iter()
        .filter(|inst| inst.terminator)
        .filter_map(|inst| match inst.operands.first() {
            Some(Operand::Block(id)) => function.blocks.iter().position(|one| one.id == *id),
            _ => None,
        })
        .collect();
    if at + 1 < function.blocks.len() && !block.insts.last().is_some_and(|last| last.barrier) {
        out.push(at + 1);
    }
    out
}

/// Whether the block holding `goal` is reachable from the first block without
/// passing through a block that holds an instruction named `through`.
fn reaches_without(
    function: &MFunction<()>,
    goal: &str,
    through: &str,
) -> bool {
    let has = |at: usize, name: &str| {
        function.blocks[at].insts.iter().any(|inst| matches!(inst.kind, Kind::Op(one) if one == name))
    };
    let mut seen = vec![false; function.blocks.len()];
    let mut stack = vec![0];
    while let Some(at) = stack.pop() {
        if std::mem::replace(&mut seen[at], true) {
            continue;
        }
        if has(at, goal) {
            return true;
        }
        if has(at, through) {
            continue;
        }
        stack.extend(successors(function, at));
    }
    false
}

/// gcd's shape: the entry tests `a == b` and leaves for the exit, the loop's
/// last block tests the same and loops back or falls into the exit. Reversed,
/// the two end in the same compare and branch. Splitting the entry for the
/// shared tail left the worklist naming its head, so a jump to the loop was put
/// after the head and the entry's test was gone: gcd(a, a) never returned.
#[test]
fn a_split_block_stays_the_common_tail_in_the_worklist() {
    let mut function = layout(vec![
        vec![work("load_a"), work("load_b"), compare(), cond("branch", 5, true)],
        vec![work("top"), cond("branch_not", 3, true)],
        vec![work("take_b"), jump(4)],
        vec![work("take_a"), work("shift")],
        vec![work("sub_x"), work("sub_y"), compare(), cond("branch_not", 1, true)],
        vec![work("restore"), ret()],
    ]);
    branch_fold::<Two>(&mut function);
    assert!(reaches_without(&function, "restore", "top"), "equal arguments cannot leave before the loop body");
}

/// The 65C02 selects `bra` for a jump; the pass takes a block's jump off and
/// puts one back, and put back `jmp`, which is a byte longer.
#[test]
fn a_jump_the_pass_takes_off_comes_back_as_the_opcode_it_was() {
    let short = |to| {
        let mut inst = op("short_jump", vec![Operand::Block(to)]);
        inst.terminator = true;
        inst.barrier = true;
        inst
    };
    let mut function =
        layout(vec![vec![compare(), cond("branch", 2, true)], vec![work("a"), short(3)], vec![work("b")], vec![ret()]]);
    branch_fold::<Two>(&mut function);
    let last = function.blocks[1].insts.last().expect("a block");
    assert_eq!(last.kind, Kind::Op("short_jump"), "the jump became {:?}", last.kind);
}
