//! `WebAssemblyArgumentMove`: the `ARGUMENT` pseudos go first in the entry
//! block, in their order.
//!
//! # See also
//! `WebAssemblyArgumentMove.cpp`, pin d1106deb71cc.

use crate::mir::{Function, is_argument};

/// Whether anything moved.
pub fn argument_move(f: &mut Function) -> bool {
    let entry = &mut f.blocks[0];
    let Some(first_other) = entry.iter().position(|i| !is_argument(i.opcode)) else { return false };
    let mut at = first_other;
    let mut changed = false;
    for i in first_other..entry.len() {
        if is_argument(entry[i].opcode) {
            let moved = entry.remove(i);
            entry.insert(at, moved);
            at += 1;
            changed = true;
        }
    }
    changed
}
