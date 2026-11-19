//! Drops a copy that puts a value back where it already is (the redundancy
//! half of LLVM's `MachineCopyPropagation`).
//!
//! # How it works
//! After allocation, going down a block, each copy `d = s` is remembered. A
//! later copy `s = d` while both still hold what the first moved is a no-op
//! and goes. A write to either register, or a call or return, forgets the
//! copies that name it (a call, all of them).
//!
//! ```text
//!   mov x19, x0        mov x19, x0
//!   mov x0, x19   ->   (gone)
//!   bl power           bl power
//! ```
//!
//! # Runs
//! After allocation, before shrink wrapping places the frame.
//!
//! # See also
//! LLVM's `MachineCopyPropagation`
//! ([MachineCopyPropagation.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineCopyPropagation.cpp)).

use crate::mc::visit::Reg;
use crate::mfunc::{MFunc, Op};

pub fn run(function: &mut MFunc) {
    for block in &mut function.blocks {
        // Live copies as (dst, src).
        let mut copies: Vec<(Reg, Reg)> = Vec::new();
        let mut at = 0;
        while at < block.ops.len() {
            let op = block.ops[at].clone();
            if let Op::Copy { dst, src } = op
                && copies.iter().any(|&(d, s)| d == src && s == dst)
            {
                block.ops.remove(at);
                continue;
            }
            match &op {
                Op::Call { .. } | Op::Return { .. } | Op::JumpTable { .. } => copies.clear(),
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
