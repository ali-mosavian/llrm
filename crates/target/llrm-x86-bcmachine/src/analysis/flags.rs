//! Port of `qbopt/analysis/flags.py`: which flags are live.

use iced_x86::FlowControl;
pub use llrm_lir::flag::{ALL, Flag};

use crate::frontends::bc::blocks::Block;
use crate::frontends::bc::declen::Insn;
use crate::support::hash::IndexMap;

/// `CLOBBERS`: a call does not itself write a flag, but its callee does.
pub const CLOBBERS: [FlowControl; 3] = [FlowControl::Call, FlowControl::IndirectCall, FlowControl::Interrupt];

pub fn written_by(insn: &Insn) -> Flag {
    if CLOBBERS.contains(&insn.flow()) { ALL } else { Flag(insn.writes() & ALL.0) }
}

/// What the block reads before writing it.
pub fn reads(block: &Block) -> Flag {
    let (mut uses, mut written) = (Flag::NONE, Flag::NONE);
    for insn in &block.insns {
        uses |= Flag(insn.reads() & ALL.0) & !written;
        written |= written_by(insn);
    }
    uses
}

pub fn writes(block: &Block) -> Flag {
    let mut found = Flag::NONE;
    for insn in &block.insns {
        found |= written_by(insn);
    }
    found
}

/// The flags live on entry to each block, to a fixed point.
pub fn live_in(blocks: &[Block]) -> IndexMap<usize, Flag> {
    let known: std::collections::BTreeSet<usize> = blocks.iter().map(|block| block.at).collect();
    let mut live: IndexMap<usize, Flag> = blocks.iter().map(|block| (block.at, Flag::NONE)).collect();
    let uses: IndexMap<usize, Flag> = blocks.iter().map(|block| (block.at, reads(block))).collect();
    let defs: IndexMap<usize, Flag> = blocks.iter().map(|block| (block.at, writes(block))).collect();

    let mut changing = true;
    while changing {
        changing = false;
        for block in blocks.iter().rev() {
            // Everything this cannot see leaves every flag live.
            let mut out = if block.leaves() { ALL } else { Flag::NONE };
            for successor in &block.succ {
                out |= if known.contains(successor) { live[successor] } else { ALL };
            }
            let now = uses[&block.at] | (out & !defs[&block.at]);
            if now != live[&block.at] {
                live.insert(block.at, now);
                changing = true;
            }
        }
    }
    live
}
