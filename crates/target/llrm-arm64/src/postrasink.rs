//! Moves a register copy into the one successor that reads its result, so
//! that a path that returns at once runs without it and shrink wrapping puts
//! the frame after the test (LLVM's `PostRAMachineSinking`).
//!
//! # How it works
//! After allocation, in a block that ends in branches, a `COPY` whose result
//! is live into exactly one successor, and that successor is entered from this
//! block alone, is dead on the other paths. Going up from the block's end, a
//! copy moves to the head of that successor when no later instruction of the
//! block writes or reads what it writes, or writes what it reads. A call stops
//! the walk: nothing moves across one.
//!
//! ```text
//!  entry:  mov w19, w1             entry:  cbz w0, ret
//!          cbz w0, ret      ->     body:   mov w19, w1
//!  ret:    ret                             ...
//!  body:   ... reads w19           ret:    ret
//! ```
//! Live-in sets of physical registers are solved once for the function and
//! kept current as copies move.
//!
//! # Example
//! `paths(n, m)` copies its second argument into the callee-saved `w19`, then
//! tests `n`. The copy sinks past the test, the frame is made after it, and a
//! leaf call (`n == 0`) is `mov w0, #1; ret`.
//!
//! # Runs
//! After allocation, before the frame is laid out.
//!
//! # See also
//! LLVM's `PostRAMachineSinking`
//! ([MachineSink.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineSink.cpp#L2243)).

use rustc_hash::FxHashMap as HashMap;

use crate::mc::inst::Gpr;
use crate::mc::visit::Reg;
use crate::mfunc::{MFunc, Op, successors_of};

/// Registers as bits: the 31 general ones, then the 32 vector ones. `sp`/`zr`
/// (31) is nothing that moves or is live.
fn bit(register: Reg) -> Option<u64> {
    match register {
        Reg::Gpr(Gpr(number)) if number < 31 => Some(1 << number),
        Reg::Fpr(vreg) if vreg.0 < 32 => Some(1 << (32 + vreg.0)),
        _ => None,
    }
}

fn mask(registers: &[Reg]) -> u64 {
    registers.iter().filter_map(|&register| bit(register)).fold(0, |all, one| all | one)
}

/// The registers live into each block.
fn live_in(
    function: &MFunc,
    successors: &[Vec<usize>],
) -> Vec<u64> {
    let count = function.blocks.len();
    let mut live = vec![0u64; count];
    let mut changed = true;
    while changed {
        changed = false;
        for at in (0..count).rev() {
            let mut now = successors[at].iter().fold(0, |all, &next| all | live[next]);
            for op in function.blocks[at].ops.iter().rev() {
                let (defs, uses) = op.defs_and_uses();
                now = (now & !mask(&defs)) | mask(&uses);
            }
            if now != live[at] {
                live[at] = now;
                changed = true;
            }
        }
    }
    live
}

/// Where a block's way on really leads: a block that is one jump (an edge
/// block, which the pin never makes where no copy needs the edge) is not a
/// place to put a copy, so its jump's target stands for it.
fn through(
    function: &MFunc,
    successors: &[Vec<usize>],
    mut block: usize,
) -> usize {
    for _ in 0..successors.len() {
        let ops = &function.blocks[block].ops;
        match (ops.len(), successors[block].as_slice()) {
            (1, [only]) if matches!(
                ops[0],
                Op::I(crate::mc::inst::Inst::Branch { link: false, .. })
            ) => block = *only,
            _ => break,
        }
    }
    block
}

pub fn run(function: &mut MFunc) {
    let count = function.blocks.len();
    let successors: Vec<Vec<usize>> = {
        let labels: HashMap<&str, usize> =
            function.blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at)).collect();
        (0..count).map(|at| successors_of(&function.blocks[at], at, count, &labels)).collect()
    };
    if successors.iter().all(|next| next.len() < 2) {
        return;
    }
    // The graph the copies move over: edge blocks seen through.
    let seen: Vec<Vec<usize>> =
        successors.iter().map(|next| next.iter().map(|&one| through(function, &successors, one)).collect()).collect();
    let mut predecessors = vec![0usize; count];
    for next in seen.iter().flatten() {
        predecessors[*next] += 1;
    }
    let mut live = live_in(function, &successors);
    for at in 0..count {
        sink_copies(function, at, &seen[at], &predecessors, &mut live);
    }
}

/// `tryToSinkCopy` of block `at`.
fn sink_copies(
    function: &mut MFunc,
    at: usize,
    successors: &[usize],
    predecessors: &[usize],
    live: &mut [u64],
) {
    // Only a successor entered from this block alone, with something live in.
    let sinkable: Vec<usize> =
        successors.iter().copied().filter(|&next| next != at && predecessors[next] == 1 && live[next] != 0).collect();
    if sinkable.is_empty() {
        return;
    }
    let (mut modified, mut used) = (0u64, 0u64);
    let mut index = function.blocks[at].ops.len();
    while index > 0 {
        index -= 1;
        let op = function.blocks[at].ops[index].clone();
        if matches!(op, Op::Call { .. }) {
            return;
        }
        let (defs, uses) = op.defs_and_uses();
        let Op::Copy { dst, src } = op else {
            modified |= mask(&defs);
            used |= mask(&uses);
            continue;
        };
        let (dst_bit, src_bit) = (bit(dst), bit(src));
        let blocked = dst_bit.is_some_and(|one| (modified | used) & one != 0)
            || src_bit.is_some_and(|one| modified & one != 0)
            || dst == src;
        let target = match dst_bit {
            Some(one) if !blocked => single_live_in(successors, &sinkable, live, one),
            _ => None,
        };
        let Some(target) = target else {
            modified |= mask(&defs);
            used |= mask(&uses);
            continue;
        };
        let (moved, loc) = function.blocks[at].ops.remove(index);
        function.blocks[target].ops.insert_at(0, moved, loc);
        live[target] = (live[target] & !dst_bit.unwrap_or(0)) | src_bit.unwrap_or(0);
    }
}

/// `getSingleLiveInSuccBB`: the one sinkable successor that `register` is live
/// into, when no other successor has it live.
fn single_live_in(
    successors: &[usize],
    sinkable: &[usize],
    live: &[u64],
    register: u64,
) -> Option<usize> {
    let mut found = None;
    for &next in sinkable {
        if live[next] & register != 0 {
            if found.is_some() {
                return None;
            }
            found = Some(next);
        }
    }
    let found = found?;
    successors
        .iter()
        .all(|&next| next == found || sinkable.contains(&next) || live[next] & register == 0)
        .then_some(found)
}
