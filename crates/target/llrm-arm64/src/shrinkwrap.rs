//! Shrink wrapping for the arm64 frame: where the prologue and the epilogue go.
//! The placement is the shared `llrm_mir::saverestore` (`ShrinkWrapImpl`); this
//! module says which blocks need the frame and gives the pass the machine
//! function's graph. The prologue goes at the top of the Save block, and the
//! epilogue before the return of the Restore block, or, where a return is
//! shared with a path that never made the frame (`tryToSplitRestore`), at the
//! end of each block that reaches it through the frame.

use std::collections::HashMap;

use llrm_analysis::saverestore::{self, Frame, Frequencies};
use llrm_mir::blocklist::BlockList;
use llrm_mir::cycleinfo::FlowGraph;
use llrm_mir::domtree::{DomGraph, DomTreeBase};
use llrm_mir::module::BlockId;

use crate::mc::inst::{Addr, Inst};
use crate::mfunc::{MFunc, Op, successors_of};

/// Where the frame is made and unmade.
#[derive(Debug)]
pub struct Wrapped {
    pub save: usize,
    /// The blocks whose end takes the frame down: the Restore block, or the
    /// blocks that reach it through the frame when it is shared.
    pub restores: Vec<usize>,
    /// Whether the epilogue goes at a block's end (split) and not before its
    /// return.
    pub at_end: bool,
}

struct Graph {
    nodes: Vec<BlockId>,
    successors: Vec<Vec<usize>>,
    predecessors: Vec<Vec<usize>>,
}

fn id(block: usize) -> BlockId {
    BlockId(block as u32)
}

fn list(blocks: &[usize]) -> BlockList {
    blocks.iter().map(|&one| id(one)).collect()
}

impl DomGraph for Graph {
    fn entry(&self) -> Option<BlockId> {
        self.nodes.first().copied()
    }

    fn max_number(&self) -> usize {
        self.nodes.len()
    }

    fn nodes(&self) -> &[BlockId] {
        &self.nodes
    }

    fn successors(
        &self,
        block: BlockId,
    ) -> BlockList {
        list(&self.successors[block.0 as usize])
    }

    fn predecessors(
        &self,
        block: BlockId,
    ) -> BlockList {
        list(&self.predecessors[block.0 as usize])
    }
}

impl FlowGraph for Graph {}

/// Whether `op` needs the frame: it calls, addresses a frame object or the
/// caller's arguments, or touches a callee-saved register.
fn needs_frame(op: &Op) -> bool {
    match op {
        Op::Call { .. } | Op::FrameAddress { .. } => true,
        Op::I(Inst::Load { addr: Addr::Slot { .. }, .. } | Inst::Store { addr: Addr::Slot { .. }, .. }) => true,
        _ => {
            let (defs, uses) = op.defs_and_uses();
            defs.iter().chain(&uses).any(|register| crate::regalloc::pool::callee_saved(*register))
        }
    }
}

/// The index where the branches that end `ops` begin: the epilogue goes before
/// them.
pub fn terminators_start(ops: &[Op]) -> usize {
    ops.iter()
        .rposition(|op| {
            !matches!(
                op,
                Op::I(
                    Inst::Branch { link: false, .. }
                        | Inst::BranchCond { .. }
                        | Inst::CompareBranch { .. }
                        | Inst::TestBranch { .. }
                )
            )
        })
        .map_or(0, |at| at + 1)
}

struct Uses<'a> {
    function: &'a MFunc,
}

impl Frame for Uses<'_> {
    fn touches(
        &self,
        block: BlockId,
        stack_address_used: bool,
    ) -> bool {
        self.function.blocks[block.0 as usize].ops.iter().any(|op| {
            needs_frame(op) || (stack_address_used && matches!(op, Op::I(Inst::Load { .. } | Inst::Store { .. })))
        })
    }

    fn terminators_touch(
        &self,
        block: BlockId,
    ) -> bool {
        let ops = &self.function.blocks[block.0 as usize].ops;
        ops[terminators_start(ops)..].iter().any(needs_frame)
    }

    fn is_return(
        &self,
        block: BlockId,
    ) -> bool {
        matches!(
            self.function.blocks[block.0 as usize].ops.last(),
            Some(Op::Return { .. })
        )
    }
}

/// `MachineBlockFrequencyInfo` by loop depth: a block in a loop is ten times
/// the depth below it.
struct Depth(Vec<u64>);

impl Frequencies for Depth {
    fn entry(&self) -> u64 {
        self.0[0]
    }

    fn block(
        &self,
        block: BlockId,
    ) -> u64 {
        self.0[block.0 as usize]
    }
}

/// The placement of the prologue and the epilogue of `function`, if it is not
/// the entry and the end of every return.
pub fn place(function: &MFunc) -> Option<Wrapped> {
    let count = function.blocks.len();
    if function.dynamic || count < 2 {
        return None;
    }
    let labels: HashMap<&str, usize> =
        function.blocks.iter().enumerate().map(|(at, one)| (one.label.as_str(), at)).collect();
    let successors: Vec<Vec<usize>> =
        function.blocks.iter().enumerate().map(|(at, block)| successors_of(block, at, count, &labels)).collect();
    let mut predecessors = vec![Vec::new(); count];
    for (from, targets) in successors.iter().enumerate() {
        for &to in targets {
            predecessors[to].push(from);
        }
    }
    let mut depth = vec![0u32; count];
    for (at, targets) in successors.iter().enumerate() {
        for &target in targets {
            if target <= at {
                for block in target..=at {
                    depth[block] += 1;
                }
            }
        }
    }
    let graph = Graph { nodes: (0..count).map(id).collect(), successors, predecessors };
    let frequencies = Depth(depth.iter().map(|&one| 10u64.pow(one.min(6))).collect());
    let placement = saverestore::run(&graph, &Uses { function }, &frequencies)?;
    let (save, restore) = (placement.save.0 as usize, placement.restore.0 as usize);
    let uses = Uses { function };
    let (restores, at_end) = match &placement.split {
        Some(split) => {
            // The epilogue goes at the end of a block whose only way on is the
            // shared return.
            if !split.iter().all(|one| graph.successors[one.0 as usize] == [restore]) {
                return None;
            }
            (split.iter().map(|one| one.0 as usize).collect::<Vec<_>>(), true)
        }
        // The Restore's return, or the end of a Restore that does not return:
        // what follows it needs no frame.
        None => (vec![restore], !uses.is_return(placement.restore)),
    };
    // A block the frame reaches is entered with the frame made only by the
    // Save: any other way in would find the stack as the caller left it.
    let mut dominators = DomTreeBase::<false>::new();
    dominators.recalculate(&graph);
    let mut seen = vec![false; count];
    let mut work = vec![save];
    while let Some(block) = work.pop() {
        if std::mem::replace(&mut seen[block], true) {
            continue;
        }
        if !dominators.dominates(id(save), id(block)) {
            return None;
        }
        if restores.contains(&block) && at_end || (block == restore && !at_end) {
            continue;
        }
        work.extend(graph.successors[block].iter().copied());
    }
    Some(Wrapped { save, restores, at_end })
}
