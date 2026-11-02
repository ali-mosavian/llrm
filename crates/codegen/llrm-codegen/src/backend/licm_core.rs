//! The target-neutral half of early `MachineLICM`: which loops have a
//! preheader, which values a loop reads from outside, which copies a loop
//! phi takes, and the walk that moves what the target's policy calls
//! hoistable to the end of the preheader. Split out of `machinelicm.rs`
//! (`HoistOutOfLoop` L827, `Hoist` L1615 of
//! [MachineLICM.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineLICM.cpp))
//! so the x86 LIR and the arm64 machine function run one pass.
//!
//! # How it works
//! The caller states its function as [`Block`]s of its own instruction type.
//! [`hoist`] takes the loops inner first. A loop with one block outside it
//! that enters the header, and that block leaving only for the header, has a
//! preheader. Each such loop is walked in layout order until nothing moves; the
//! [`Policy`] says for each instruction whether it moves, given [`Facts`] of
//! the loop. A moved instruction goes before the preheader's jump.
//!
//! # Example
//! ```text
//!   pre:  jmp loop                      pre:  v9 := adrp @g ; jmp loop
//!   loop: v9 := adrp @g            =>   loop: use v9
//!         use v9
//! ```
//!
//! # See also
//! `machinelicm` (the x86 policy), the arm64 policy in `llrm-arm64`.

use std::collections::BTreeSet;

use crate::analysis::loops::{self as loopy, Node};
use crate::support::hash::{HashMap, HashSet};

/// What the pass needs of an instruction.
pub trait Inst: Clone {
    /// The values it defines.
    fn defines(&self) -> &[u32];
    /// The values it reads.
    fn uses(&self) -> &[u32];
    /// A move of one register into another (`COPY`): its source is a value a
    /// phi may take (`HasLoopPHIUse`).
    fn is_copy(&self) -> bool;
    /// Any move, a load among them: the chains a phi's input is followed
    /// through.
    fn is_move(&self) -> bool;
    /// Not code: a marker the preheader's end skips.
    fn is_meta(&self) -> bool;
    /// An unconditional jump or a branch: where the preheader ends.
    fn is_jump(&self) -> bool;

    /// Computes what `other` computes, from the same values (`isIdenticalTo`
    /// with the result ignored): a hoisted copy of it is not made twice
    /// (`EliminateCSE`). x86 states none.
    fn identical(
        &self,
        _other: &Self,
    ) -> bool {
        false
    }

    /// Reads `to` where it read `from`.
    fn rename_use(
        &mut self,
        _from: u32,
        _to: u32,
    ) {
    }
}

/// A phi: the value it defines and the values it takes.
#[derive(Clone, Debug)]
pub struct Phi {
    pub result: u32,
    pub incoming: Vec<u32>,
}

/// A block of the caller's function. `at` is its identity (`succ` names it).
#[derive(Clone, Debug)]
pub struct Block<I> {
    pub at: i64,
    pub succ: Vec<i64>,
    pub phis: Vec<Phi>,
    pub insns: Vec<I>,
}

impl<I> Node for Block<I> {
    fn at(&self) -> i64 {
        self.at
    }

    fn succ(&self) -> &[i64] {
        &self.succ
    }
}

/// What the walk knows of the loop it is in.
pub struct Facts<'a, I> {
    /// The blocks of the loop, by `at`.
    pub inside: &'a BTreeSet<i64>,
    /// The loop's blocks, by `at`, to read their instructions.
    pub blocks: &'a [Block<I>],
    /// The values the loop reads from outside and its phis' results: the
    /// registers it holds across its trips (floating point values among them:
    /// the policy leaves out the ones it counts apart).
    pub named: &'a HashSet<u32>,
    /// `HasLoopPHIUse`: values a phi of the loop or of an exit block takes,
    /// directly or through copies in the loop.
    pub copied: &'a HashSet<u32>,
    /// How many instructions define each value in the whole function.
    pub defs: &'a HashMap<u32, usize>,
    /// Whether the block runs on every trip of the loop
    /// (`IsGuaranteedToExecute`).
    pub every_trip: &'a dyn Fn(i64) -> bool,
}

/// The target's half.
pub trait Policy<I: Inst> {
    /// A loop is about to be walked: the policy reads what it needs of it
    /// (calls, stores, the stack of floating point values).
    fn begin(
        &mut self,
        facts: &Facts<'_, I>,
    );

    /// Whether `insns[index]` of the block `at` moves to the preheader,
    /// given that every value in `hoisted` already has. A policy also states
    /// when it moves an instruction whose operands are not all outside (it
    /// never does: the walk checks `IsLoopInvariantInst` first).
    fn movable(
        &mut self,
        facts: &Facts<'_, I>,
        at: i64,
        insns: &[I],
        index: usize,
    ) -> bool;
}

/// `blocks` with what `policy` moved placed at the end of each loop's
/// preheader; `None` when nothing moved. `entry` is the function's first
/// block.
pub fn hoist<I: Inst, P: Policy<I>>(
    blocks: &[Block<I>],
    entry: i64,
    policy: &mut P,
) -> Option<Vec<Block<I>>> {
    let found = loopy::loops(blocks, Some(entry));
    if found.is_empty() {
        return None;
    }
    let dominance = loopy::dominance(blocks, Some(entry));
    let mut blocks: Vec<Block<I>> = blocks.to_vec();
    let place: HashMap<i64, usize> = blocks.iter().enumerate().map(|(number, block)| (block.at, number)).collect();
    // Where each value is defined, and how many times.
    let mut defined: HashMap<u32, i64> = HashMap::default();
    let mut defs: HashMap<u32, usize> = HashMap::default();
    for block in &blocks {
        for phi in &block.phis {
            defined.insert(phi.result, block.at);
        }
        for one in &block.insns {
            for &value in one.defines() {
                defined.insert(value, block.at);
                *defs.entry(value).or_default() += 1;
            }
        }
    }
    let mut order: Vec<&loopy::Loop> = found.iter().collect();
    order.sort_by_key(|one| one.body.to_set().len());
    let mut changed = false;
    for lp in order {
        let inside = lp.body.to_set();
        let outside_entries: Vec<i64> = blocks
            .iter()
            .filter(|block| !inside.contains(&block.at) && block.succ.contains(&lp.header))
            .map(|block| block.at)
            .collect();
        let [pre] = outside_entries[..] else { continue };
        let pre_at = place[&pre];
        if blocks[pre_at].succ.len() != 1 {
            continue;
        }
        let named = named_values(&blocks, &place, &inside, &defined);
        let copied = phi_values(&blocks, &place, &inside);
        let exiting: Vec<i64> =
            inside.iter().copied().filter(|at| blocks[place[at]].succ.iter().any(|to| !inside.contains(to))).collect();
        let every_trip = |at: i64| at == lp.header || exiting.iter().all(|&out| dominance.dominates(at, out));
        let mut hoisted_values: HashSet<u32> = HashSet::default();
        let mut moved: Vec<I> = Vec::new();
        policy.begin(&Facts {
            inside: &inside,
            blocks: &blocks,
            named: &named,
            copied: &copied,
            defs: &defs,
            every_trip: &every_trip,
        });
        loop {
            let mut any = false;
            let mut renames: Vec<(u32, u32)> = Vec::new();
            for at in blocks.iter().map(|block| block.at).filter(|at| inside.contains(at)).collect::<Vec<_>>() {
                let number = place[&at];
                let insns = blocks[number].insns.clone();
                let mut keep: Vec<I> = Vec::new();
                for (index, one) in insns.iter().enumerate() {
                    let invariant = one.uses().iter().all(|used| {
                        hoisted_values.contains(used) || defined.get(used).is_none_or(|home| !inside.contains(home))
                    });
                    let facts = Facts {
                        inside: &inside,
                        blocks: &blocks,
                        named: &named,
                        copied: &copied,
                        defs: &defs,
                        every_trip: &every_trip,
                    };
                    if invariant && policy.movable(&facts, at, &insns, index) {
                        hoisted_values.extend(one.defines().iter().copied());
                        any = true;
                        // `EliminateCSE`: the same computation hoisted before
                        // stands for this one.
                        match moved.iter().find(|before| before.identical(one)) {
                            Some(before) if before.defines().len() == one.defines().len() => {
                                renames.extend(one.defines().iter().copied().zip(before.defines().iter().copied()));
                            }
                            _ => moved.push(one.clone()),
                        }
                    } else {
                        keep.push(one.clone());
                    }
                }
                blocks[number].insns = keep;
            }
            for (from, to) in renames {
                for block in blocks.iter_mut() {
                    for one in block.insns.iter_mut() {
                        one.rename_use(from, to);
                    }
                    for phi in block.phis.iter_mut() {
                        phi.incoming.iter_mut().filter(|value| **value == from).for_each(|value| *value = to);
                    }
                }
                for one in moved.iter_mut() {
                    one.rename_use(from, to);
                }
            }
            if !any {
                break;
            }
        }
        if moved.is_empty() {
            continue;
        }
        changed = true;
        for one in &moved {
            for &value in one.defines() {
                defined.insert(value, pre);
            }
        }
        let insns = &mut blocks[pre_at].insns;
        let end = insns.iter().rposition(|one| !one.is_meta()).filter(|&last| insns[last].is_jump());
        let at = end.unwrap_or(insns.len());
        for (offset, one) in moved.into_iter().enumerate() {
            insns.insert(at + offset, one);
        }
    }
    changed.then_some(blocks)
}

/// The values the loop reads from outside it, and its phis' results.
fn named_values<I: Inst>(
    blocks: &[Block<I>],
    place: &HashMap<i64, usize>,
    inside: &BTreeSet<i64>,
    defined: &HashMap<u32, i64>,
) -> HashSet<u32> {
    inside
        .iter()
        .flat_map(|at| {
            let block = &blocks[place[at]];
            block
                .insns
                .iter()
                .flat_map(|one| one.uses().iter().copied())
                .filter(|value| defined.get(value).is_none_or(|home| !inside.contains(home)))
                .chain(block.phis.iter().map(|phi| phi.result))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// `HasLoopPHIUse`: the values a copy in the loop reads, a phi of the loop
/// (or of an exit block) takes, and what feeds a copy of those.
fn phi_values<I: Inst>(
    blocks: &[Block<I>],
    place: &HashMap<i64, usize>,
    inside: &BTreeSet<i64>,
) -> HashSet<u32> {
    let mut taken: HashSet<u32> = inside
        .iter()
        .flat_map(|at| blocks[place[at]].insns.iter())
        .filter(|one| one.is_copy())
        .flat_map(|one| one.uses().iter().copied())
        .collect();
    let exits = inside
        .iter()
        .flat_map(|at| blocks[place[at]].succ.iter().copied())
        .filter(|to| !inside.contains(to) && place.contains_key(to));
    for at in inside.iter().copied().chain(exits) {
        for phi in &blocks[place[&at]].phis {
            taken.extend(phi.incoming.iter().copied());
        }
    }
    let copies: Vec<(u32, Vec<u32>)> = inside
        .iter()
        .flat_map(|at| blocks[place[at]].insns.iter())
        .filter(|one| one.is_move())
        .filter_map(|one| Some((*one.defines().first()?, one.uses().to_vec())))
        .collect();
    while copies.iter().any(|(result, uses)| taken.contains(result) && uses.iter().any(|value| taken.insert(*value))) {}
    taken
}
