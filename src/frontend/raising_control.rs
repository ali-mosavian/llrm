//! Port of `qbopt/frontend/raising_control.py`: apply established runtime
//! control contracts before constructing value SSA.

use crate::abi::runtime::{Contract, Control};
use crate::frontend::blocks::{Block, Ends};
use crate::support::hash::IndexMap;

/// A terminal call at a block boundary has no normal-return successor.
///
/// Interior calls need a separate byte-owning block split; this changes no
/// instruction spans, and never treats an unestablished contract as proof.
pub fn terminal_edges(blocks: Vec<Block>, contracts: &IndexMap<i64, Contract>) -> Vec<Block> {
    let mut result = Vec::new();
    for mut block in blocks {
        let contract = block.insns.last().and_then(|last| contracts.get(&(last.at as i64)));
        if contract.is_some_and(|contract| contract.established && contract.control == Control::Never) {
            block = Block { ends: Ends::Leaves, succ: Vec::new(), ..block };
        }
        result.push(block);
    }
    result
}
