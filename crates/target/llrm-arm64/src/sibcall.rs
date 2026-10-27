//! Sibling calls: a call marked `tail` whose result is what the function
//! returns becomes the jump to the callee, after the epilogue
//! (`AArch64TargetLowering::LowerCall`'s `IsTailCall`, `TCRETURNdi`).
//!
//! Machine form, after selection and before allocation: the block ends
//! `Call { tail }`, copies that carry the result registers to the return's,
//! and the `Return`; the copies and the call become a `Return` that carries the
//! callee and the argument registers.
//!
//! An indirect callee is moved to `x9` first: the epilogue restores the
//! callee-saved registers and uses `x16`/`x17` itself, and `x9` is neither
//! (the pin uses `x16`/`x17` and keeps them free of its epilogue).
//!
//! DEVIATE 1: the call takes no stack arguments (it is only marked then), so
//! the caller's incoming argument area is never needed. Exit: stack arguments
//! in the caller's own area.

use crate::mc::inst::Gpr;
use crate::mc::visit::Reg;
use crate::mfunc::{Callee, MFunc, Op};

/// Where the value `register` holds before `ops[end]` was copied from, looking
/// back no further than `ops[from]`.
fn origin(
    ops: &[Op],
    from: usize,
    end: usize,
    register: Reg,
) -> Reg {
    ops[from..end]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(at, op)| match op {
            Op::Copy { dst, src } if *dst == register => Some(origin(ops, from, from + at, *src)),
            _ => None,
        })
        .unwrap_or(register)
}

/// Where an indirect callee waits for the epilogue.
const TARGET: Gpr = Gpr(9);

pub fn run(function: &mut MFunc) {
    for block in &mut function.blocks {
        let Some(Op::Return { uses, tail: None }) = block.ops.last() else { continue };
        let last = block.ops.len() - 1;
        let Some(call) = block.ops[..last].iter().rposition(|op| !matches!(op, Op::Copy { .. })) else { continue };
        let Op::Call { callee, uses: arguments, defs, tail: true } = &block.ops[call] else { continue };
        if uses.len() != defs.len() || uses.iter().zip(defs).any(|(u, def)| origin(&block.ops, call + 1, last, *u) != *def) {
            continue;
        }
        let (callee, arguments) = (callee.clone(), arguments.clone());
        block.ops.truncate(call);
        let target = match callee {
            Callee::Register(register) => {
                block.ops.push(Op::Copy { dst: Reg::Gpr(TARGET), src: Reg::Gpr(register) });
                Callee::Register(TARGET)
            }
            symbol => symbol,
        };
        block.ops.push(Op::Return { uses: arguments, tail: Some(target) });
    }
    function.calls = function.blocks.iter().flat_map(|block| &block.ops).any(|op| matches!(op, Op::Call { .. }));
}
