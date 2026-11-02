//! `WebAssemblyArgumentMove`: the `ARGUMENT` pseudos go first in the entry
//! block, in their order.
//!
//! DEVIATE: the `ARGUMENT` of a later block moves too. A `MEMCPY` or `MEMSET`
//! with a variable length splits its block (`custom_inserter`), and an
//! `ARGUMENT` scheduled after it lands in the tail; the pin's output has it
//! in the entry block.
//! Reason: this selector's schedule is not the pin's. Exit: none, the result
//! is the pin's.
//!
//! # See also
//! `WebAssemblyArgumentMove.cpp`, pin d1106deb71cc.

use crate::mir::{Function, is_argument};

/// Whether anything moved.
pub fn argument_move(f: &mut Function) -> bool {
    let mut changed = false;
    let mut later = Vec::new();
    for block in f.blocks.iter_mut().skip(1) {
        let (arguments, others): (Vec<_>, Vec<_>) = block.drain(..).partition(|i| is_argument(i.opcode));
        *block = others;
        changed |= !arguments.is_empty();
        later.extend(arguments);
    }
    let entry = &mut f.blocks[0];
    let first_other = entry.iter().position(|i| !is_argument(i.opcode)).unwrap_or(entry.len());
    let mut at = first_other;
    for i in first_other..entry.len() {
        if is_argument(entry[i].opcode) {
            let moved = entry.remove(i);
            entry.insert(at, moved);
            at += 1;
            changed = true;
        }
    }
    entry.splice(at..at, later);
    changed
}
