//! Sibling calls: a call marked `tail` whose result is what the function
//! returns becomes the jump to the callee, after the epilogue
//! (`AArch64TargetLowering::LowerCall`'s `IsTailCall`, `TCRETURNdi`).
//!
//! Machine form, after selection and before allocation: the block ends
//! `Call { tail }`, copies that carry the result registers to the return's,
//! and the `Return`; the copies and the call become a `Return` that carries the
//! callee and the argument registers.
//!
//! DEVIATE 1: the callee is a symbol and takes no stack arguments (the call is
//! only marked then), so the caller's incoming argument area is never needed.
//! Exit: stack arguments in the caller's own area and an indirect callee in
//! `x16`/`x17`.

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

pub fn run(function: &mut MFunc) {
    for block in &mut function.blocks {
        let Some(Op::Return { uses, tail: None }) = block.ops.last() else { continue };
        let last = block.ops.len() - 1;
        let Some(call) = block.ops[..last].iter().rposition(|op| !matches!(op, Op::Copy { .. })) else { continue };
        let Op::Call { callee: Callee::Symbol(callee), uses: arguments, defs, tail: true } = &block.ops[call] else {
            continue;
        };
        if uses.len() != defs.len() || uses.iter().zip(defs).any(|(u, def)| origin(&block.ops, call + 1, last, *u) != *def) {
            continue;
        }
        let jump = Op::Return { uses: arguments.clone(), tail: Some(Callee::Symbol(callee.clone())) };
        block.ops.truncate(call);
        block.ops.push(jump);
    }
    function.calls = function.blocks.iter().flat_map(|block| &block.ops).any(|op| matches!(op, Op::Call { .. }));
}
