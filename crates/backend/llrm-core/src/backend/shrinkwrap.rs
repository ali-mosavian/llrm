//! LLVM's ShrinkWrap: the registers a procedure keeps for its caller (SI and
//! DI), and its frame (the stack reserve and the frame register), set up where
//! they are first needed instead of at every entry, and taken back only on the
//! returns that path reaches.
//!
//! A procedure that leaves early (`if row == 7 { return 1 }`) before any of its
//! loop touches SI paid a push and a pop for it on every call. The save goes to
//! the block that dominates every use, and the restore to each return that
//! block dominates; where a return can be reached both through that block
//! and around it, or the save would sit in a loop, nothing is wrapped and the
//! entry saves as before.

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;

use crate::analysis::loops::{dominance, immediate_dominators, loops};
use crate::model::ir::{self, Loc};
use crate::model::lir::{Insn, LirBody};

/// Where a procedure saves what it keeps: at the top of block `at`, and
/// restored before the returns of `restored` blocks.
pub struct Wrap {
    pub at: i64,
    pub restored: BTreeSet<i64>,
    /// The frame's setup is at `at` too, and the returns outside `restored`
    /// take none back: nothing outside the blocks `at` dominates touches
    /// the frame or the stack.
    pub frame: bool,
}

/// The registers `one` names, and those the call it is disturbs by its
/// convention.
pub fn named(one: &Insn) -> BTreeSet<RegId> {
    let mut found: BTreeSet<RegId> =
        one.call.iter().flat_map(|call| call.disturbs.iter().copied().map(ir::root)).collect();
    let Some(what) = &one.what else { return found };
    for place in what.dests.iter().chain(&what.sources) {
        match place {
            Loc::Reg(ir::Reg { register, .. }) => {
                found.insert(ir::root(*register));
            }
            Loc::Mem(ir::Mem { through, index_through, .. }) => {
                found.extend([*through, *index_through].map(ir::root));
            }
            _ => {}
        }
    }
    found
}

/// Whether `one` reads or writes the frame or the stack pointer: a frame cell
/// or argument, a push or pop, or an instruction the printer cannot read.
fn touches_frame(one: &Insn) -> bool {
    let Some(what) = &one.what else { return false };
    if matches!(
        what.op,
        ir::Operation::Push | ir::Operation::Pop | ir::Operation::Leave | ir::Operation::Escape
    )
        || (what.op == ir::Operation::Nothing
            && what.name.as_deref().is_some_and(|name| name.starts_with("push") || name.starts_with("pop")))
    {
        return true;
    }
    what.dests.iter().chain(&what.sources).any(Loc::in_frame)
}

/// The wrap of `body` for the registers `kept`, and for the frame where `frame`
/// names the frame register and the stack pointer, if one is worth having. A
/// frame that cannot be wrapped leaves the registers to be.
pub fn wrapped(
    body: &LirBody,
    kept: &BTreeSet<RegId>,
    frame: Option<(RegId, RegId)>,
) -> Option<Wrap> {
    frame.and_then(|registers| placed(body, kept, Some(registers))).or_else(|| placed(body, kept, None))
}

fn placed(
    body: &LirBody,
    kept: &BTreeSet<RegId>,
    frame: Option<(RegId, RegId)>,
) -> Option<Wrap> {
    if body.noreturn || (kept.is_empty() && frame.is_none()) {
        return None;
    }
    let mut kept = kept.clone();
    kept.extend(frame.iter().flat_map(|(pointer, stack)| [ir::root(*pointer), ir::root(*stack)]));
    let entry = Some(body.entry);
    let from_entry = dominance(&body.blocks, entry);
    // Every block must be reached from the entry: a block only the runtime
    // enters has no dominator.
    if body.blocks.iter().any(|block| !from_entry.reachable(block.at)) {
        return None;
    }
    let idom = immediate_dominators(&body.blocks, entry);
    let uses: Vec<i64> = body
        .blocks
        .iter()
        .filter(|block| {
            block.insns.iter().any(|one| {
                (frame.is_some() && touches_frame(one)) || named(one).iter().any(|register| kept.contains(register))
            })
        })
        .map(|block| block.at)
        .collect();
    let first = *uses.first()?;
    // The nearest block above every use: the first of one's ancestors every
    // other use is below. The home only rises, so the climb is the depth of
    // the first use in all, not a chain of its ancestors for each use.
    let mut home = first;
    for &other in &uses[1..] {
        while !from_entry.dominates(home, other) {
            home = idom[&home].expect("the entry dominates every block");
        }
    }
    // Not in a loop: a save there would run each trip.
    let cycles = loops(&body.blocks, entry);
    while home != body.entry && cycles.iter().any(|one| one.body.contains(&home)) {
        home = idom[&home].expect("a block below the entry has a dominator");
    }
    if home == body.entry {
        return None;
    }
    let from_home = dominance(&body.blocks, Some(home));
    let mut restored = BTreeSet::new();
    // The frame is set up at `home`, so a block it reaches that something else
    // also reaches would be entered at two depths.
    if frame.is_some()
        && body.blocks.iter().any(|block| from_home.reachable(block.at) && !from_entry.dominates(home, block.at))
    {
        return None;
    }
    for block in &body.blocks {
        let returns =
            block.insns.iter().any(|one| one.what.as_ref().is_some_and(|what| what.op == ir::Operation::Return));
        if !returns || !from_home.reachable(block.at) {
            continue;
        }
        // Reached around the home as well: a restore there would pop what was
        // never pushed.
        if !from_entry.dominates(home, block.at) {
            return None;
        }
        restored.insert(block.at);
    }
    Some(Wrap { at: home, restored, frame: frame.is_some() })
}
