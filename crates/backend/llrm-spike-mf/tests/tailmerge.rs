//! `TailMergeBlocks` through `branch_fold`.

mod common;

use common::{Two, branch, op};
use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::Assignment;
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, branch_fold, branch_fold_hoisting_across_hot_loops};
use llrm_support::hash::{HashMap, HashSet};

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

/// A loop's latch `compare; branch_not top` shared its `compare` with the
/// entry block's, which left the latch a `jump` to the header: one executed
/// jump per iteration for two bytes saved (m68k gcd, +20 clk at 68000). Blocks
/// of different loops never merge.
#[test]
fn a_loop_latch_is_not_merged_with_a_block_outside_the_loop() {
    let mut function = layout(vec![
        vec![work("load_a"), work("load_b"), compare(), cond("branch", 5, true)],
        vec![work("top"), cond("branch_not", 3, true)],
        vec![work("take_b"), jump(4)],
        vec![work("take_a"), work("shift")],
        vec![work("sub_x"), work("sub_y"), compare(), cond("branch_not", 1, true)],
        vec![work("restore"), ret()],
    ]);
    branch_fold::<Two>(&mut function);
    let latch = function
        .blocks
        .iter()
        .find(|block| block.insts.iter().any(|inst| matches!(inst.kind, Kind::Op("sub_y"))))
        .expect("the latch");
    let last = latch.insts.last().expect("a terminator");
    assert!(matches!(last.kind, Kind::Op("branch_not")), "the latch ends in {:?}", last.kind);
}

fn latch_shape() -> MFunction<()> {
    layout(vec![
        vec![work("load_a"), work("load_b"), compare(), cond("branch", 5, true)],
        vec![work("top"), cond("branch_not", 3, true)],
        vec![work("take_b"), jump(4)],
        vec![work("take_a"), work("shift")],
        vec![work("sub_x"), work("sub_y"), compare(), cond("branch_not", 1, true)],
        vec![work("restore"), ret()],
    ])
}

fn latch_ends_in(function: &MFunction<()>) -> Kind {
    let latch = function
        .blocks
        .iter()
        .find(|block| block.insts.iter().any(|inst| matches!(inst.kind, Kind::Op("sub_y"))))
        .expect("the latch");
    latch.insts.last().expect("a terminator").kind
}

/// pattern_fill -O1 kept a 64-trip loop's guard and latch apart (`dec b; jr z;
/// push bc; jp top`, +5 B and 3072 T over the merged form). A loop named as
/// repeating joins its latch with the guard around it; one not named does not.
#[test]
fn a_loop_that_repeats_merges_its_latch_with_its_guard() {
    let assignment = Assignment(vec![RegId::from_raw(0); 8]);
    let run = |hot: &[u32]| {
        let mut function = latch_shape();
        let mut frequencies = HashMap::default();
        branch_fold_hoisting_across_hot_loops::<Two>(
            &mut function,
            &assignment,
            &hot.iter().copied().collect::<HashSet<u32>>(),
            &mut frequencies,
        );
        latch_ends_in(&function)
    };
    assert_eq!(run(&[]), Kind::Op("branch_not"), "a loop not named keeps its latch");
    assert_ne!(run(&[1]), Kind::Op("branch_not"), "a named loop's latch joins the guard's tail");
}

/// A common tail runs as often as the blocks merged into it together: priced at
/// its old block's frequency the folded function looked cheaper than it ran
/// (switch_table_holes -O1: 373 B / 29931 T priced, 395 B / 270215 T measured).
#[test]
fn a_merged_tail_takes_the_frequencies_of_the_tails_merged_into_it() {
    let assignment = Assignment(vec![RegId::from_raw(0); 8]);
    let mut function = latch_shape();
    let mut frequencies: HashMap<u32, f64> = [(0, 1.0), (1, 10.0), (4, 10.0), (5, 1.0)].into_iter().collect();
    branch_fold_hoisting_across_hot_loops::<Two>(
        &mut function,
        &assignment,
        &[1].into_iter().collect(),
        &mut frequencies,
    );
    let tail = function
        .blocks
        .iter()
        .find(|block| block.insts.first().is_some_and(|inst| matches!(inst.kind, Kind::Op("compare"))))
        .expect("the shared compare");
    assert_eq!(frequencies.get(&tail.id), Some(&11.0), "{frequencies:?}");
}

/// Every question about a block's neighbours rescanned the function, so the
/// pass was cubic in the blocks: a 4341-line function (div_constants_long at
/// m68k) spent 45% of its 3 s in `successors`. 3000 blocks that share nothing
/// took minutes; they take milliseconds.
#[test]
fn the_pass_does_not_rescan_the_function_for_each_question() {
    let blocks: Vec<Vec<Inst>> =
        (0..3000).map(|n| vec![work(Box::leak(format!("w{n}").into_boxed_str())), ret()]).collect();
    let mut function = layout(blocks);
    let start = std::time::Instant::now();
    branch_fold::<Two>(&mut function);
    let took = start.elapsed();
    assert!(took.as_secs() < 5, "3000 unrelated blocks took {took:?}");
}
