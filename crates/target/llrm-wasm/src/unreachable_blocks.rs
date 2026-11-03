//! `UnreachableMachineBlockElim`: the blocks no path from the entry reaches are
//! removed.
//!
//! # See also
//! `UnreachableBlockElim.cpp`, pin d1106deb71cc.

use crate::mir::Function;

/// Whether anything was removed.
pub fn remove_unreachable_blocks(f: &mut Function) -> bool {
    let mut reached = vec![false; f.blocks.len()];
    let mut work = vec![0];
    while let Some(block) = work.pop() {
        if !std::mem::replace(&mut reached[block], true) {
            work.extend(f.succs[block].iter().copied());
        }
    }
    let kept: Vec<usize> = (0..f.blocks.len()).filter(|&block| reached[block]).collect();
    if kept.len() == f.blocks.len() {
        return false;
    }
    f.apply_order(&kept);
    true
}
