//! Drops a copy that puts a value back where it already is, and reads a copy's
//! source where its destination was read (LLVM's `MachineCopyPropagation`,
//! `ForwardCopyPropagateBlock`).
//!
//! # How it works
//! After allocation, going down a block, each copy `d = s` is remembered. A
//! later copy `s = d` while both still hold what the first moved is a no-op
//! and goes. A later read of `d` is a read of `s`, so the copy can sink out
//! of a block that only reads it (`PostRaSink`) and the frame it forced
//! (a callee-saved `d`) can wait (shrink wrapping). A write to either
//! register, or a call or return, forgets the copies that name it (a call,
//! all of them).
//!
//! ```text
//!   mov x19, x0        mov x19, x0
//!   mov x0, x19   ->   (gone)
//!   bl power           bl power
//!
//!   mov w20, w2        mov w20, w2
//!   cmp w1, w20   ->   cmp w1, w2
//! ```
//!
//! # Runs
//! After allocation, before shrink wrapping places the frame.
//!
//! # See also
//! LLVM's `MachineCopyPropagation`
//! ([MachineCopyPropagation.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineCopyPropagation.cpp)).

use crate::mc::visit::{Reg, RegRef, Role};
use crate::mfunc::{MFunc, Op};

/// `ForwardCopyPropagateBlock`: each register `op` reads that a live copy
/// filled is read from the copy's source. Not the stack pointer or the zero
/// register (register 31 means either), nor the registers of a call.
fn forward(
    op: &mut Op,
    copies: &[(Reg, Reg)],
) {
    if !matches!(op, Op::I(_) | Op::Copy { .. }) {
        return;
    }
    op.visit(&mut |register, role| {
        let RegRef::Gpr(used) = register else { return };
        if role != Role::Use || used.0 == 31 || used.is_virtual() {
            return;
        }
        if let Some(&(_, Reg::Gpr(source))) = copies.iter().find(|&&(dst, _)| dst == Reg::Gpr(*used))
            && source.0 != 31
            && !source.is_virtual()
        {
            *used = source;
        }
    });
}

pub fn run(function: &mut MFunc) {
    for block in &mut function.blocks {
        // Live copies as (dst, src).
        let mut copies: Vec<(Reg, Reg)> = Vec::new();
        let mut at = 0;
        while at < block.ops.len() {
            let mut op = block.ops[at].clone();
            forward(&mut op, &copies);
            if op != block.ops[at] {
                *block.ops.op_mut(at) = op.clone();
            }
            // A copy of a register onto itself (forwarding made it one) goes,
            // like the repeat of a copy still in force.
            if let Op::Copy { dst, src } = op
                && (dst == src || copies.iter().any(|&(d, s)| d == src && s == dst))
            {
                block.ops.remove(at);
                continue;
            }
            match &op {
                Op::Call { .. } | Op::Return { .. } | Op::JumpTable { .. } | Op::IndirectJump { .. } => copies.clear(),
                _ => {
                    let (defs, _) = op.defs_and_uses();
                    copies.retain(|(d, s)| !defs.contains(d) && !defs.contains(s));
                    if let Op::Copy { dst, src } = op
                        && dst != src
                    {
                        copies.push((dst, src));
                    }
                }
            }
            at += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mc::inst::{AddSub, Gpr, Inst, Shift, Width};
    use crate::mfunc::MBlock;

    fn compare(
        rn: u32,
        rm: u32,
    ) -> Op {
        Op::I(Inst::AddSubShifted {
            op: AddSub { subtract: true, set_flags: true },
            width: Width::W,
            rd: Gpr(31),
            rn: Gpr(rn),
            rm: Gpr(rm),
            shift: Shift::Lsl,
            amount: 0,
        })
    }

    fn run_on(ops: Vec<Op>) -> Vec<Op> {
        let mut function = MFunc::new("f");
        function.blocks.push(MBlock { label: "entry".into(), ops: ops.into() });
        run(&mut function);
        function.blocks[0].ops.iter().cloned().collect()
    }

    /// `mov x20, x2; cmp w1, w20` read the copy: a callee-saved register made
    /// before an early exit, which kept the frame before it too (quicksort
    /// -O2: 119,846 instructions against 117,375).
    #[test]
    fn a_read_of_a_copys_destination_reads_its_source() {
        let copy = Op::Copy { dst: Reg::Gpr(Gpr(20)), src: Reg::Gpr(Gpr(2)) };
        assert_eq!(run_on(vec![copy.clone(), compare(1, 20)]), vec![copy, compare(1, 2)]);
    }

    #[test]
    fn a_read_after_the_source_is_written_keeps_the_destination() {
        let copy = Op::Copy { dst: Reg::Gpr(Gpr(20)), src: Reg::Gpr(Gpr(2)) };
        let ops = vec![copy, compare(2, 2), Op::Imm { dst: Gpr(2), bits: 32, value: 7 }, compare(1, 20)];
        assert_eq!(run_on(ops.clone())[3], compare(1, 20));
    }

    /// `mov x19, x0; mov x0, x19` forwarded is `mov x0, x0`: a copy onto
    /// itself, which stayed (recpow -O1: 72,077 instructions against 70,306).
    #[test]
    fn a_copy_that_forwarding_made_onto_itself_is_dropped() {
        let save = Op::Copy { dst: Reg::Gpr(Gpr(19)), src: Reg::Gpr(Gpr(0)) };
        let back = Op::Copy { dst: Reg::Gpr(Gpr(0)), src: Reg::Gpr(Gpr(19)) };
        assert_eq!(run_on(vec![save.clone(), back]), vec![save]);
    }
}
