//! LLVM's ShrinkWrap for the registers a procedure keeps for its caller (SI and DI): saved where they
//! are first needed instead of at every entry, and restored only on the returns that path reaches.
//!
//! A procedure that leaves early (`if row == 7 { return 1 }`) before any of its loop touches SI paid a
//! push and a pop for it on every call. The save goes to the block that dominates every use, and the
//! restore to each return that block dominates; where a return can be reached both through that block
//! and around it, or the save would sit in a loop, nothing is wrapped and the entry saves as before.

use std::collections::{BTreeMap, BTreeSet};

use iced_x86::Register;

use crate::model::ir::{self, Loc};
use crate::model::lir::{Insn, LirBody};

/// Where a procedure saves what it keeps: at the top of block `at`, and restored before the returns
/// of `restored` blocks.
pub struct Wrap {
    pub at: i64,
    pub restored: BTreeSet<i64>,
}

/// The registers `one` names.
pub fn named(one: &Insn) -> BTreeSet<Register> {
    let mut found = BTreeSet::new();
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

/// The wrap of `body` for the registers `kept`, where one is worth having.
pub fn wrapped(body: &LirBody, kept: &BTreeSet<Register>) -> Option<Wrap> {
    if body.noreturn || kept.is_empty() {
        return None;
    }
    let succ: BTreeMap<i64, &Vec<i64>> = body.blocks.iter().map(|block| (block.at, &block.succ)).collect();
    // Every block must be reached from the entry: a block only the runtime enters has no dominator.
    let order = reverse_postorder(body.entry, &succ);
    if order.len() != body.blocks.len() {
        return None;
    }
    let idom = dominators(body.entry, &order, &succ);
    let uses: Vec<i64> = body.blocks.iter().filter(|block| block.insns.iter().any(|one| named(one).iter().any(|register| kept.contains(register)))).map(|block| block.at).collect();
    let first = *uses.first()?;
    let mut home = uses.iter().copied().fold(first, |one, other| common(&idom, one, other));
    // Not in a loop: a save there would run each trip.
    while home != body.entry && reaches(home, home, &succ) {
        home = idom[&home];
    }
    if home == body.entry {
        return None;
    }
    let below: BTreeSet<i64> = reachable(home, &succ);
    let mut restored = BTreeSet::new();
    for block in &body.blocks {
        let returns = block.insns.iter().any(|one| one.what.as_ref().is_some_and(|what| what.op == ir::Operation::Return));
        if !returns || !below.contains(&block.at) {
            continue;
        }
        // Reached around the home as well: a restore there would pop what was never pushed.
        if !dominates(&idom, home, block.at) {
            return None;
        }
        restored.insert(block.at);
    }
    Some(Wrap { at: home, restored })
}

fn reverse_postorder(entry: i64, succ: &BTreeMap<i64, &Vec<i64>>) -> Vec<i64> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut stack = vec![(entry, 0_usize)];
    seen.insert(entry);
    while let Some((block, next)) = stack.pop() {
        let edges = succ.get(&block).map_or(&[][..], |one| &one[..]);
        if let Some(&to) = edges.get(next) {
            stack.push((block, next + 1));
            if succ.contains_key(&to) && seen.insert(to) {
                stack.push((to, 0));
            }
        } else {
            out.push(block);
        }
    }
    out.reverse();
    out
}

/// Each block's immediate dominator (the entry's is itself): Cooper, Harvey and Kennedy's.
fn dominators(entry: i64, order: &[i64], succ: &BTreeMap<i64, &Vec<i64>>) -> BTreeMap<i64, i64> {
    let number: BTreeMap<i64, usize> = order.iter().enumerate().map(|(at, &block)| (block, at)).collect();
    let mut preds: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    for &block in order {
        for &to in succ[&block] {
            if number.contains_key(&to) {
                preds.entry(to).or_default().push(block);
            }
        }
    }
    let mut idom: BTreeMap<i64, i64> = BTreeMap::from([(entry, entry)]);
    let meet = |idom: &BTreeMap<i64, i64>, mut one: i64, mut other: i64| {
        while one != other {
            while number[&one] > number[&other] {
                one = idom[&one];
            }
            while number[&other] > number[&one] {
                other = idom[&other];
            }
        }
        one
    };
    let mut changed = true;
    while changed {
        changed = false;
        for &block in order.iter().skip(1) {
            let mut found: Option<i64> = None;
            for &before in preds.get(&block).map_or(&[][..], |one| &one[..]) {
                if idom.contains_key(&before) {
                    found = Some(found.map_or(before, |one| meet(&idom, one, before)));
                }
            }
            if let Some(found) = found {
                if idom.get(&block) != Some(&found) {
                    idom.insert(block, found);
                    changed = true;
                }
            }
        }
    }
    idom
}

fn common(idom: &BTreeMap<i64, i64>, one: i64, other: i64) -> i64 {
    let ancestors = |mut block: i64| {
        let mut out = vec![block];
        while idom[&block] != block {
            block = idom[&block];
            out.push(block);
        }
        out
    };
    let mine: BTreeSet<i64> = ancestors(one).into_iter().collect();
    ancestors(other).into_iter().find(|block| mine.contains(block)).expect("the entry dominates every block")
}

fn dominates(idom: &BTreeMap<i64, i64>, above: i64, block: i64) -> bool {
    let mut at = block;
    loop {
        if at == above {
            return true;
        }
        if idom[&at] == at {
            return false;
        }
        at = idom[&at];
    }
}

fn reachable(from: i64, succ: &BTreeMap<i64, &Vec<i64>>) -> BTreeSet<i64> {
    let mut seen = BTreeSet::from([from]);
    let mut work = vec![from];
    while let Some(block) = work.pop() {
        for &to in succ.get(&block).map_or(&[][..], |one| &one[..]) {
            if seen.insert(to) {
                work.push(to);
            }
        }
    }
    seen
}

/// Whether a path leads from `from` back to `to`.
fn reaches(from: i64, to: i64, succ: &BTreeMap<i64, &Vec<i64>>) -> bool {
    succ.get(&from).map_or(&[][..], |one| &one[..]).iter().any(|&next| reachable(next, succ).contains(&to))
}
