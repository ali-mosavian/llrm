//! `place` and `place_blocks` over small layouts.

mod common;

use common::{Two, op};
use llrm_mir::blockfrequency::{BlockFrequency, BranchProbability};
use llrm_spike_mf::placement::{Layout, Loop, Odds, place, place_blocks};
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand};

/// A hand-made layout: successors with odds, block frequencies, loops.
struct Hand {
    succs: Vec<Vec<(usize, u32)>>,
    freqs: Vec<u64>,
    loops: Vec<Loop>,
}

impl Hand {
    fn targets(&self) -> Vec<Vec<usize>> {
        self.succs.iter().map(|list| list.iter().map(|one| one.0).collect()).collect()
    }
}

struct Shaped {
    hand: Hand,
    succs: Vec<Vec<usize>>,
    preds: Vec<Vec<usize>>,
}

impl Shaped {
    fn new(hand: Hand) -> Shaped {
        let succs = hand.targets();
        let mut preds = vec![Vec::new(); succs.len()];
        for (from, list) in succs.iter().enumerate() {
            for &to in list {
                preds[to].push(from);
            }
        }
        Shaped { hand, succs, preds }
    }
}

impl Layout for Shaped {
    fn len(&self) -> usize {
        self.succs.len()
    }

    fn successors(
        &self,
        block: usize,
    ) -> &[usize] {
        &self.succs[block]
    }

    fn predecessors(
        &self,
        block: usize,
    ) -> &[usize] {
        &self.preds[block]
    }

    fn probability(
        &self,
        from: usize,
        to: usize,
    ) -> BranchProbability {
        let percent = self.hand.succs[from].iter().find(|one| one.0 == to).map_or(0, |one| one.1);
        BranchProbability::new(percent, 100)
    }

    fn frequency(
        &self,
        block: usize,
    ) -> BlockFrequency {
        BlockFrequency(self.hand.freqs[block])
    }

    fn keeps_fall_through(
        &self,
        _: usize,
    ) -> bool {
        false
    }

    fn loops(&self) -> Vec<Loop> {
        self.hand.loops.clone()
    }

    fn optimize_for_size(&self) -> bool {
        false
    }
}

/// A loop entered from a block that prefers another successor has no
/// fall-through into its header, so its exit test goes to the bottom: the body
/// then ends in the one conditional branch back, with the test once per trip
/// instead of a test and a jump. Left in source order (`0 1 2 3 4`) it costs
/// the jump each trip.
#[test]
fn a_loop_entered_by_a_jump_rotates_its_exit_to_the_bottom() {
    let layout = Shaped::new(Hand {
        succs: vec![vec![(1, 30), (4, 70)], vec![(2, 90), (3, 10)], vec![(1, 100)], vec![], vec![(3, 100)]],
        freqs: vec![10, 30, 27, 10, 7],
        loops: vec![Loop { header: 1, blocks: vec![1, 2], inner: vec![] }],
    });
    assert_eq!(place(&layout), vec![0, 4, 2, 1, 3]);
}

/// A loop its predecessor falls into keeps the header on top: rotating would
/// trade the entry fall-through for the exit one.
#[test]
fn a_loop_the_entry_falls_into_keeps_its_header_on_top() {
    let layout = Shaped::new(Hand {
        succs: vec![vec![(1, 100)], vec![(2, 90), (3, 10)], vec![(1, 100)], vec![]],
        freqs: vec![1, 11, 10, 1],
        loops: vec![Loop { header: 1, blocks: vec![1, 2], inner: vec![] }],
    });
    assert_eq!(place(&layout), vec![0, 1, 2, 3]);
}

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

fn function(blocks: Vec<Vec<Inst>>) -> MFunction<()> {
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

fn odds(edges: &[(u32, u32, u32)]) -> Odds {
    edges.iter().enumerate().map(|(rank, &(from, to, percent))| ((from, to), (BranchProbability::new(percent, 100), rank))).collect()
}

/// The cold block between a branch and its likely target goes to the end, and
/// the branch is reversed to fall into the likely one: the hot path then takes
/// no jump. In source order the likely edge was a taken branch every time.
#[test]
fn a_cold_block_between_a_branch_and_its_target_moves_to_the_end() {
    let mut f = function(vec![
        vec![terminator("branch", false, Some(2))],
        vec![op("cold", vec![]), terminator("jump", true, Some(3))],
        vec![op("hot", vec![])],
        vec![terminator("ret", true, None)],
    ]);
    assert!(place_blocks::<(), Two>(&mut f, &odds(&[(0, 2, 90), (0, 1, 10)]), false));
    assert_eq!(
        shape(&f),
        vec![
            (0, names(&["branch_not 1"])),
            (2, names(&["hot"])),
            (3, names(&["ret"])),
            (1, names(&["cold", "jump 3"])),
        ]
    );
}

/// Blocks already in the best order are left alone, branches and all.
#[test]
fn a_function_in_its_best_order_is_unchanged() {
    let mut f = function(vec![
        vec![terminator("branch", false, Some(2))],
        vec![op("work", vec![])],
        vec![terminator("ret", true, None)],
    ]);
    let before = shape(&f);
    assert!(!place_blocks::<(), Two>(&mut f, &odds(&[(0, 1, 90), (0, 2, 10)]), false));
    assert_eq!(shape(&f), before);
}
