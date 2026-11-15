//! Block placement on the shared LIR placer: the blocks of a function after
//! register assignment seen as `llrm_codegen`'s `LirBody`, so
//! `MachineBlockPlacement` is the one translation x86 uses.
//!
//! Each instruction is one LIR instruction that keeps its place in `payload`
//! by the number its `what.name` carries (`k7`), so a copy a placement makes
//! (`TailDupPlacement`) maps back to the instruction it copies.

use std::sync::Arc;

use llrm_codegen::backend::jumps::placed_explicit;
use llrm_codegen::model::lir::{Block, BlockOdds, Insn, LirBlock, LirBody};
use llrm_lir::{Operation, Semantics};
use llrm_support::hash::HashMap;

use crate::frame::Lowered;
use crate::mc::inst::{Inst, Target};

/// Instruction numbers start here, apart from the block numbers.
const FIRST_INSN: i64 = 1 << 20;

/// What `inst` does to placement.
fn operation(inst: &Inst) -> (Operation, bool) {
    match inst {
        Inst::Branch { link: false, target: Target::Label(_) } => (Operation::Jump, false),
        Inst::BranchReg { link: false, .. } => (Operation::Jump, true),
        Inst::Branch { link: true, .. } | Inst::BranchReg { link: true, .. } => (Operation::Call, false),
        Inst::BranchCond { .. } | Inst::CompareBranch { .. } | Inst::TestBranch { .. } => (Operation::Branch, false),
        Inst::Ret { .. } => (Operation::Return, false),
        Inst::Brk { .. } => (Operation::Barrier, false),
        _ => (Operation::Binary, false),
    }
}

/// The blocks as placed: each one's position and the numbers of the
/// instructions it ends with (counted over all blocks in their order), which
/// differ from the block's own where a tail was copied into it.
pub(crate) struct Placed {
    pub blocks: Vec<(usize, Vec<usize>)>,
}

impl Placed {
    /// The positions in layout order.
    pub fn order(&self) -> Vec<usize> {
        self.blocks.iter().map(|(at, _)| *at).collect()
    }

    /// Whether any block holds other instructions than it began with.
    pub fn copied(
        &self,
        starts: &[usize],
        lens: &[usize],
    ) -> bool {
        self.blocks.iter().any(|(at, kept)| {
            kept.len() != lens[*at] || kept.iter().enumerate().any(|(place, &number)| number != starts[*at] + place)
        })
    }
}

/// The blocks `blocks` (the reachable ones, numbered by position) in the order
/// `MachineBlockPlacement` lays them out. `edges` is each block's
/// successors, `odds` the share of each edge in 2^31sts, and `size` is
/// `-Os`/`-Oz`.
pub(crate) fn order(
    blocks: &[&Lowered],
    edges: &[Vec<usize>],
    odds: &HashMap<(usize, usize), u32>,
    index: &HashMap<&str, usize>,
    size: bool,
) -> Placed {
    let mut next = FIRST_INSN;
    let lir: Vec<Block> = blocks
        .iter()
        .enumerate()
        .map(|(at, block)| {
            let insns: Vec<Arc<Insn>> = block
                .insts
                .iter()
                .map(|inst| {
                    let (op, indirect) = operation(inst);
                    let target = crate::layout::label_of(inst).and_then(|label| index.get(label)).map(|&to| to as i64);
                    let what = Semantics {
                        name: Some(format!("k{next}").into()),
                        target,
                        indirect,
                        ..Semantics::new(op)
                    };
                    let one = Insn::new(next, Some((next, next)), Some(what), Vec::new(), Vec::new());
                    next += 1;
                    Arc::new(one)
                })
                .collect();
            let succ: Vec<i64> = edges[at].iter().map(|&to| to as i64).collect();
            Block::from(LirBlock { succ: succ.into(), ..LirBlock::new(at as i64, insns) })
        })
        .collect();
    let mut body = LirBody::new("f", 0, lir.clone(), Default::default(), Default::default());
    body.odds = BlockOdds { taken: odds.iter().map(|(&(from, to), &share)| ((from as i64, to as i64), share)).collect() };
    let placed = placed_explicit(&body, lir, size, false, None, None, 0, false);
    let number = |one: &Insn| {
        one.what.as_ref().and_then(|what| what.name.as_deref()?.strip_prefix('k')?.parse::<i64>().ok()).map(|k| (k - FIRST_INSN) as usize)
    };
    Placed {
        blocks: placed
            .blocks
            .iter()
            .map(|block| (block.at as usize, block.insns.iter().filter_map(|one| number(one)).collect()))
            .collect(),
    }
}
