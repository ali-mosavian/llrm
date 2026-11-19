//! A move into a register that already holds the same zero: GCC's
//! `postreload-cse` (`reload_cse_regs`, `reload_cse_simplify_set`) for the one
//! constant every function makes the most.
//!
//! # How it works
//! A forward dataflow over the allocated body keeps the byte lanes known to be
//! zero before each instruction. `xor r, r`, `sub r, r` and `mov r, 0` set the
//! lanes of `r`; `mov d, s` sets the lanes of `d` that `s` holds zero in; any
//! other write clears what it writes, and an instruction of unknown effect
//! clears all. A block starts with the lanes every edge into it leaves zero
//! (the entry block with none). Then a `mov d, s` whose `d` and `s` are both
//! zero, and a `mov d, 0` whose `d` is, change nothing and are dropped. `xor r,
//! r` stays: it writes the flags.
//!
//! ```text
//!   xor edx, edx                 xor edx, edx
//!   test ax, ax                  test ax, ax
//!   ...                          ...
//!   xor eax, eax                 xor eax, eax
//!   rep stosb                    rep stosb
//!   mov dx, ax      =>           (dropped: dx and ax are zero)
//! ```
//!
//! `machinecp` (LLVM's `MachineCopyPropagation`) sees the copies of one block
//! and no values, so it cannot tell that `dx` already held the zero `ax` holds.
//! LLVM has no counterpart: its `MachineCSE` runs before the phis become copies
//! and does not share a constant a phi reads (heuristic 3).
//!
//! # Example
//! m16 `sieve` -Os: the count (`dx`) and the sum (`eax`) both start at zero,
//! the count's zero made at entry, the sum's after the early exit; `mov dx, ax`
//! is the second zero into `dx`: 112 B, one over Open Watcom's 111.
//!
//! # Runs
//! After the last `machinecp`, on x86 bodies with no phis.
//!
//! # See also
//! [`crate::backend::machinecp`]; GCC
//! [`postreload.cc`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/postreload.cc).

use std::sync::Arc;

use crate::backend::lanes::{self, Lane, Lanes};
use crate::backend::liveness;
use crate::backend::peephole::{_lanes, _register_effects};
use crate::model::ir::{Loc, Operation, Reg};
use crate::model::lir::{self, Block, Insn, LirBody};
use crate::support::hash::IndexMap;

/// What `one` is, as far as the lanes known zero go.
enum Kind {
    /// Sets these lanes to zero: `xor r, r`, `sub r, r`, `mov r, 0`.
    Zeroes {
        into: Lanes,
        movable: bool,
    },
    /// `mov d, s` between registers of one width: the lanes of `d` and of `s`,
    /// in order.
    Copy {
        into: Vec<Lane>,
        from: Vec<Lane>,
    },
    Other,
}

/// Whether `one` is a plain instruction a pass may drop or rewrite: it states
/// nothing beyond its operands.
fn plain(one: &Insn) -> bool {
    one.requires.is_empty()
        && one.delivers.is_empty()
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.spread.is_empty()
        && one.group.is_none()
        && one.symbol != Some(true)
}

fn kind(
    regs: llrm_lir::registers::Regs,
    bits: u32,
    one: &Insn,
) -> Kind {
    let Some(what) = &one.what else {
        return Kind::Other;
    };
    let [Loc::Reg(Reg { register, width })] = what.dests.as_slice() else {
        return Kind::Other;
    };
    let into: Vec<_> = _lanes(regs, *register).into_iter().collect();
    if into.len() != *width as usize {
        return Kind::Other;
    }
    // A 32-bit write in long mode also zeroes the upper half of its root, which
    // the lanes of the 32-bit register do not name: such a move is not dropped.
    if *width == 4 && bits == 64 {
        return Kind::Other;
    }
    let destination = &what.dests[0];
    match (what.op, what.name.as_deref(), what.sources.as_slice()) {
        (_, Some("xor" | "sub"), [left, right]) if left == destination && right == destination => {
            Kind::Zeroes { into: into.into_iter().collect(), movable: false }
        }
        (Operation::Move, Some("mov"), [Loc::Imm(constant)]) if constant.address.is_none() && constant.value == 0 => {
            Kind::Zeroes { into: into.into_iter().collect(), movable: plain(one) }
        }
        (Operation::Move, Some("mov"), [Loc::Reg(Reg { register: from, width: same })]) if same == width => {
            let from: Vec<_> = _lanes(regs, *from).into_iter().collect();
            let apart = into.iter().all(|lane| !from.contains(lane));
            if from.len() == into.len() && apart && plain(one) { Kind::Copy { into, from } } else { Kind::Other }
        }
        _ => Kind::Other,
    }
}

/// The lanes known zero after `one`, given those before it; none where its
/// effect is unknown.
fn after(
    regs: llrm_lir::registers::Regs,
    bits: u32,
    one: &Insn,
    zero: Lanes,
) -> Lanes {
    let Some(effect) = liveness::effect(regs, bits, one) else {
        return Lanes::new();
    };
    // A repeated string store writes its pointers only when the count is not
    // zero: a write all the same.
    let maybe = _register_effects(regs, bits, one, true, false).map_or(Lanes::new(), |(_, writes)| writes);
    let kept = zero.minus(&effect.writes.or(&maybe));
    match kind(regs, bits, one) {
        Kind::Zeroes { into, .. } => kept.or(&into),
        Kind::Copy { into, from } => {
            let mut made = kept;
            for (to, source) in into.iter().zip(&from) {
                if zero.contains(source) {
                    made.insert(*to);
                }
            }
            made
        }
        Kind::Other => kept,
    }
}

/// The lanes known zero on entry to each block: the lanes every way in leaves
/// zero, none for the entry block and for a block nothing leads to.
fn entering(
    body: &LirBody,
    regs: llrm_lir::registers::Regs,
) -> IndexMap<i64, Lanes> {
    let mut preds: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &body.blocks {
        for &next in block.succ.iter() {
            preds.entry(next).or_default().push(block.at);
        }
    }
    // Optimistic: every block leaves every lane zero until a way in says not.
    let mut leaving: IndexMap<i64, Option<Lanes>> = body.blocks.iter().map(|block| (block.at, None)).collect();
    let mut into: IndexMap<i64, Lanes> = IndexMap::default();
    loop {
        let mut changed = false;
        for block in &body.blocks {
            let start = if block.at == body.entry {
                Lanes::new()
            } else {
                let mut ways =
                    preds.get(&block.at).into_iter().flatten().filter_map(|at| leaving.get(at).copied().flatten());
                match ways.next() {
                    Some(first) => ways.fold(first, |all, next| all.and(&next)),
                    // Nothing known of any way in yet (or none at all).
                    None if preds.contains_key(&block.at) => continue,
                    None => Lanes::new(),
                }
            };
            into.insert(block.at, start);
            let end = block.insns.iter().fold(start, |zero, one| after(regs, body.bits, one, zero));
            if leaving.get(&block.at).copied().flatten() != Some(end) {
                leaving.insert(block.at, Some(end));
                changed = true;
            }
        }
        if !changed {
            return into;
        }
    }
}

/// `body` without the moves that put into a register the zero it holds.
pub fn removed(body: &LirBody) -> LirBody {
    let regs = body.regs();
    if !lanes::is_x86(regs) || body.blocks.iter().any(|block| !block.phis.is_empty()) {
        return body.clone();
    }
    // A body with no zeroing and no move to drop has nothing to find.
    let candidate = |one: &Arc<Insn>| matches!(
        kind(regs, body.bits, one),
        Kind::Zeroes { .. } | Kind::Copy { .. }
    );
    if !body.blocks.iter().any(|block| block.insns.iter().any(candidate)) {
        return body.clone();
    }
    let into = entering(body, regs);
    let mut changed = false;
    let blocks: Vec<Block> = body
        .blocks
        .iter()
        .map(|block| {
            let Some(&start) = into.get(&block.at) else {
                return block.clone();
            };
            let mut zero = start;
            let mut insns: Vec<Arc<Insn>> = Vec::with_capacity(block.insns.len());
            let mut dropped = false;
            for one in block.insns.iter() {
                let redundant = match kind(regs, body.bits, one) {
                    Kind::Zeroes { into, movable } => movable && into.is_subset(&zero),
                    Kind::Copy { into, from } => into.iter().chain(&from).all(|lane| zero.contains(lane)),
                    Kind::Other => false,
                };
                if redundant {
                    insns.push(lir::anchor(Arc::clone(one)));
                    dropped = true;
                } else {
                    zero = after(regs, body.bits, one, zero);
                    insns.push(Arc::clone(one));
                }
            }
            if dropped {
                changed = true;
                block.with_insns(insns)
            } else {
                block.clone()
            }
        })
        .collect();
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// The phase: [`removed`] over each body.
pub struct KnownZero;

impl crate::model::passes::LIRTransform for KnownZero {
    fn class_name(&self) -> &'static str {
        "KnownZero"
    }

    fn name(&self) -> &str {
        "knownzero"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(removed(&body))
    }
}

#[cfg(test)]
#[path = "knownzero_tests.rs"]
mod tests;
