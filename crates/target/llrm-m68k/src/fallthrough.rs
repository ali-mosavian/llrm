//! A function whose last instruction jumps to the function laid out right after
//! it falls through instead.
//!
//! # How it works
//! [`drop_jumps_to_next`] removes a final `bra next` (a tail call, or the
//! `bench_fib: bra fib` wrapper) when `next` is the function emitted after it:
//! 4 bytes and 10 clocks a call. `BranchFolder::OptimizeBranches` does the same
//! for a block and its layout successor; a function and the next one are the
//! same case once both are in one text section.

use crate::compile::Item;
use crate::mc::Operand;

/// The functions with each final jump to the next function's entry removed.
pub fn drop_jumps_to_next(functions: &mut [(String, Vec<Item>)]) {
    for at in 1..functions.len() {
        let (before, after) = functions.split_at_mut(at);
        let next = after[0].0.as_str();
        let items = &mut before[at - 1].1;
        let Some(last) = items.iter().rposition(|item| !matches!(item, Item::Loc(_))) else { continue };
        let jumps = matches!(
            &items[last],
            Item::Inst(inst) if inst.opcode.starts_with("BRA") && matches!(&inst.operands[..], [Operand::Symbol(target, 0)] if target == next)
        );
        if jumps {
            items.truncate(last);
        }
    }
}
