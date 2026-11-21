//! `MachineCopyPropagation`: after allocation, a copy's source stands in for
//! its destination where the destination is read, and a copy nothing reads
//! goes.
//!
//! Translation of `forwardCopyPropagateBlock`, `forwardUses` and
//! `eraseIfRedundant`
//! ([MachineCopyPropagation.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineCopyPropagation.cpp))
//! at d1106deb71cc.
//!
//! # How it works
//! Going down a block, each copy `d = s` is remembered while neither register
//! is written. A later copy that repeats it, or puts the value back
//! (`s = d`), is a no-op and goes. A read of `d` in an instruction is
//! replaced by `s` (the source is read earlier, so the chain through the copy
//! shortens); a copy none of whose reads were kept, and whose destination no
//! successor reads, is removed at the block's end.
//!
//! ```text
//!   mov  w0, w1             mov  w0, w1
//!   sdiv w9, w8, w0   ->    sdiv w9, w8, w1     (w0 is live out: the copy stays)
//! ```
//!
//! DEVIATE MCP1 (owner arm64, exit 2026-12-18): only general registers
//! `x0`..`x28` are tracked (the pin also forwards a copy of the zero register
//! and tracks vector registers: a vector copy here may move 64 bits of a
//! 128-bit value); a call forgets every copy (the pin keeps those the call's
//! mask preserves); only `Role::Use` operands of an `Inst` or a copy are
//! forwarded into (the pin's "renamable" operands: the arguments of a call or a
//! return are physical).
//!
//! # Runs
//! After allocation, before the frame is laid out.

use rustc_hash::FxHashMap as HashMap;

use crate::mc::inst::Gpr;
use crate::mc::visit::{Reg, RegRef, Role};
use crate::mfunc::{MFunc, Op, successors_of};
use crate::postrasink::{bit, live_in};

/// A copy `dst = src` of two general registers `isNeverRedundant` does not
/// rule out (the frame pointer, the link register, `sp`/`xzr`).
fn tracked(
    dst: Reg,
    src: Reg,
) -> bool {
    let plain = |register: Reg| matches!(register, Reg::Gpr(Gpr(number)) if number < Gpr::FP.0);
    plain(dst) && plain(src) && dst != src
}

/// A copy being tracked.
struct Tracked {
    at: usize,
    src: Reg,
    /// Whether `src` still holds what it did at the copy.
    available: bool,
}

pub fn run(function: &mut MFunc) {
    let count = function.blocks.len();
    let successors: Vec<Vec<usize>> = {
        let labels: HashMap<&str, usize> =
            function.blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at)).collect();
        (0..count).map(|at| successors_of(&function.blocks[at], at, count, &labels)).collect()
    };
    let live = live_in(function, &successors);
    for (at, block) in function.blocks.iter_mut().enumerate() {
        let live_out = successors[at].iter().fold(0u64, |all, &next| all | live[next]);
        propagate_block(block, live_out);
    }
}

/// Whether `op` takes a forwarded register: an instruction or a copy, not
/// a call or a return (their registers are the convention's).
fn takes_forwarded(op: &Op) -> bool {
    matches!(op, Op::I(_) | Op::Copy { .. })
}

/// `forwardCopyPropagateBlock` of `block`, where `live_out` are the registers
/// its successors read.
fn propagate_block(
    block: &mut crate::mfunc::MBlock,
    live_out: u64,
) {
    // The copy now holding each destination (`Tracker.Copies`).
    let mut copies: HashMap<Reg, Tracked> = HashMap::default();
    // Copies whose destination nothing has read yet (`MaybeDeadCopies`).
    let mut maybe_dead: Vec<usize> = Vec::new();
    let mut at = 0;
    while at < block.ops.len() {
        if let Op::Copy { dst, src } = block.ops[at].clone()
            && tracked(dst, src)
        {
            // The two copies cancel out, or the same copy is made twice.
            let redundant =
                |holder: Reg, from: Reg| copies.get(&holder).is_some_and(|held| held.available && held.src == from);
            if redundant(dst, src) || redundant(src, dst) {
                block.ops.remove(at);
                // The copy kept now serves this one's reads.
                continue;
            }
        }
        // `forwardUses`.
        if takes_forwarded(&block.ops[at]) {
            block
                .ops
                .op_mut(at)
                .visit(
                    &mut |register, role| {
                        let (RegRef::Gpr(register), Role::Use) = (register, role) else { return };
                        if let Some(held) = copies.get(&Reg::Gpr(*register))
                            && held.available
                            && let Reg::Gpr(source) = held.src
                        {
                            *register = source;
                        }
                    },
                );
        }
        let (defs, uses) = block.ops[at].defs_and_uses();
        // A read of a copy's destination keeps the copy.
        for register in &uses {
            if let Some(held) = copies.get(register) {
                maybe_dead.retain(|&one| one != held.at);
            }
        }
        if let Op::Copy { dst, src } = block.ops[at]
            && dst != src
        {
            maybe_dead.push(at);
        }
        let calls = matches!(block.ops[at], Op::Call { .. });
        // A write ends what the register held, and what was copied from it.
        for register in &defs {
            copies.remove(register);
            for held in copies.values_mut().filter(|held| held.src == *register) {
                held.available = false;
            }
        }
        if calls {
            // The call's mask is not tracked, so no source stands after it; a
            // destination the call keeps (what IPRA lets a caller rely on)
            // is still read later, and that read still keeps its copy.
            for held in copies.values_mut() {
                held.available = false;
            }
        }
        if let Op::Copy { dst, src } = block.ops[at]
            && tracked(dst, src)
        {
            copies.insert(dst, Tracked { at, src, available: true });
        }
        at += 1;
    }
    // A copy nothing in the block read, whose result no successor reads.
    maybe_dead.sort_unstable();
    for &copy in maybe_dead.iter().rev() {
        let Op::Copy { dst, src } = block.ops[copy] else { continue };
        if tracked(dst, src) && bit(dst).is_some_and(|one| live_out & one == 0) {
            retarget_debug_values(block, copy, dst, src);
            block.ops.remove(copy);
        }
    }
}

/// The debug values that name a removed copy's destination name its source
/// (`updateDbgUsersToReg`).
fn retarget_debug_values(
    block: &mut crate::mfunc::MBlock,
    copy: usize,
    dst: Reg,
    src: Reg,
) {
    for at in copy + 1..block.ops.len() {
        if let Op::DbgValue { reg, .. } = block.ops.op_mut(at)
            && *reg == Some(dst)
        {
            *reg = Some(src);
        }
        if block.ops[at].defs_and_uses().0.as_slice().contains(&dst) {
            break;
        }
    }
}
