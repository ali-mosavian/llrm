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

    /// A load that may read a different value later in the preheader than
    /// where the loop reads it (not invariant): `EliminateCSE` leaves it.
    fn is_plain_load(&self) -> bool {
        false
    }

    /// Every value `rename_use` may change.
    fn each_value(
        &self,
        mut each: impl FnMut(u32),
    ) {
        for &value in self.uses().iter().chain(self.defines()) {
            each(value);
        }
    }

    /// Reads `to` where it read `from`.
    fn rename_use(
        &mut self,
        _from: u32,
        _to: u32,
    ) {
    }
}

#[cfg(test)]
thread_local! {
    /// Blocks the policy read through `Facts::inside_blocks`.
    pub(crate) static READ: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
thread_local! {
    /// Instructions the rename sweep looked at.
    pub(crate) static SWEPT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `one` reading `to` where it read `from`, for each pair of `renamed`.
fn rename_uses<I: Inst>(
    one: &mut I,
    renamed: &llrm_support::hash::HashMap<u32, u32>,
) {
    #[cfg(test)]
    SWEPT.with(|swept| swept.set(swept.get() + 1));
    if !one.uses().iter().any(|used| renamed.contains_key(used)) {
        return;
    }
    for used in one.uses().to_vec() {
        if let Some(&to) = renamed.get(&used) {
            one.rename_use(used, to);
        }
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
    /// The function's blocks, to read their instructions.
    pub blocks: &'a [Block<I>],
    /// The positions in `blocks` of the loop's blocks, in order.
    pub members: &'a [usize],
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

impl<'a, I> Facts<'a, I> {
    /// The loop's blocks, in the function's order.
    pub fn inside_blocks(&self) -> impl Iterator<Item = &'a Block<I>> + use<'a, I> {
        let blocks = self.blocks;
        self.members
            .iter()
            .map(
                move |&number| {
                    #[cfg(test)]
                    READ.with(|read| read.set(read.get() + 1));
                    &blocks[number]
                },
            )
    }
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

    /// `ExtractHoistableLoad`: `one` as the load it makes, defining a new
    /// value, and what is left of it reading that value. The pin tries it for
    /// an instruction that does not move; the load is used only if it moves.
    fn unfold(
        &mut self,
        _one: &I,
    ) -> Option<(I, I)> {
        None
    }

    /// Whether a moved instruction goes ahead of the preheader's own, not at
    /// its end: an x87 value defined first lies deepest on the stack.
    fn leads(
        &self,
        _one: &I,
    ) -> bool {
        false
    }
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
    // The blocks that read each value, or did: a superset, since a moved
    // instruction leaves a block and a rename adds readers. The sweep after a
    // round of renames visits these blocks, not the function (the pin's
    // `replaceRegWith` follows the use list of each register).
    let mut readers: HashMap<u32, BTreeSet<usize>> = HashMap::default();
    for (number, block) in blocks.iter().enumerate() {
        for one in &block.insns {
            one.each_value(|used| {
                readers.entry(used).or_default().insert(number);
            });
        }
        for phi in &block.phis {
            for &used in &phi.incoming {
                readers.entry(used).or_default().insert(number);
            }
        }
    }
    // The blocks that branch to each block, in the function's order.
    let mut entries: HashMap<i64, Vec<i64>> = HashMap::default();
    for block in &blocks {
        for &to in &block.succ {
            let from = entries.entry(to).or_default();
            if from.last() != Some(&block.at) {
                from.push(block.at);
            }
        }
    }
    // How many loops each block is in: a header with 1 belongs to a loop no
    // other loop holds, the `CurLoop` of the pin's worklist.
    let mut nest: HashMap<i64, usize> = HashMap::default();
    for lp in &found {
        for at in &lp.body {
            *nest.entry(*at).or_default() += 1;
        }
    }
    let mut order: Vec<&loopy::Loop> = found.iter().collect();
    order.sort_by_key(|one| one.body.to_set().len());
    let mut changed = false;
    for lp in order {
        let inside = lp.body.to_set();
        let mut members: Vec<usize> = inside.iter().map(|at| place[at]).collect();
        members.sort_unstable();
        let outside_entries: Vec<i64> =
            entries.get(&lp.header).into_iter().flatten().copied().filter(|at| !inside.contains(at)).collect();
        let [pre] = outside_entries[..] else { continue };
        let pre_at = place[&pre];
        if blocks[pre_at].succ.len() != 1 {
            continue;
        }
        let top_level = nest.get(&lp.header) == Some(&1);
        let named = named_values(&blocks, &place, &inside, &defined);
        let copied = phi_values(&blocks, &place, &inside);
        let exiting: Vec<i64> =
            inside.iter().copied().filter(|at| blocks[place[at]].succ.iter().any(|to| !inside.contains(to))).collect();
        let every_trip = |at: i64| at == lp.header || exiting.iter().all(|&out| dominance.dominates(at, out));
        let mut hoisted_values: HashSet<u32> = HashSet::default();
        let mut moved: Vec<I> = Vec::new();
        // A hoist `EliminateCSE` folded into the preheader's copy moves nothing, and still edits the loop.
        let mut folded = false;
        policy.begin(&Facts {
            inside: &inside,
            blocks: &blocks,
            members: &members,
            named: &named,
            copied: &copied,
            defs: &defs,
            every_trip: &every_trip,
        });
        loop {
            let mut any = false;
            let mut renames: Vec<(u32, u32)> = Vec::new();
            for &number in &members {
                let at = blocks[number].at;
                let insns = blocks[number].insns.clone();
                let mut keep: Vec<I> = Vec::new();
                for (index, one) in insns.iter().enumerate() {
                    let invariant = one.uses().iter().all(|used| {
                        hoisted_values.contains(used) || defined.get(used).is_none_or(|home| !inside.contains(home))
                    });
                    let facts = Facts {
                        inside: &inside,
                        blocks: &blocks,
                        members: &members,
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
                        // `InitCSEMap(Preheader)`, on the first hoist out of a
                        // loop no other loop holds: what the preheader
                        // computes already stands for it too.
                        let same = |before: &&I| before.identical(one) && before.defines().len() == one.defines().len();
                        let there = if top_level {
                            blocks[pre_at].insns.iter().find(|before| !before.is_plain_load() && same(before))
                        } else {
                            None
                        };
                        match there.or_else(|| moved.iter().find(same)) {
                            Some(before) => {
                                folded = true;
                                renames.extend(one.defines().iter().copied().zip(before.defines().iter().copied()));
                            }
                            None => moved.push(one.clone()),
                        }
                    } else if let Some((load, rest)) = policy.unfold(one).filter(|(load, _)| {
                        load.uses().iter().all(|used| {
                            hoisted_values.contains(used) || defined.get(used).is_none_or(|home| !inside.contains(home))
                        }) && policy.movable(&facts, at, std::slice::from_ref(load), 0)
                    }) {
                        hoisted_values.extend(load.defines().iter().copied());
                        any = true;
                        moved.push(load);
                        // The unfolded half reads the load's new value: a later
                        // rename of that value finds it (`readers` was built
                        // before the unfold).
                        rest.each_value(|used| {
                            readers.entry(used).or_default().insert(number);
                        });
                        keep.push(rest);
                    } else {
                        keep.push(one.clone());
                    }
                }
                blocks[number].insns = keep;
            }
            // One sweep for all the renames: a `from` is a duplicate's
            // definition and a `to` a kept one's, so no rename feeds another
            // (the pin's `replaceRegWith` follows the use list of each).
            if !renames.is_empty() {
                let renamed: llrm_support::hash::HashMap<u32, u32> = renames.into_iter().collect();
                let mut touched: BTreeSet<usize> = BTreeSet::new();
                for (from, to) in &renamed {
                    if let Some(read) = readers.get(from).cloned() {
                        touched.extend(read.iter().copied());
                        readers.entry(*to).or_default().extend(read);
                    }
                }
                for number in touched {
                    let block = &mut blocks[number];
                    for one in block.insns.iter_mut() {
                        rename_uses(one, &renamed);
                    }
                    for phi in block.phis.iter_mut() {
                        for value in phi.incoming.iter_mut() {
                            if let Some(&to) = renamed.get(value) {
                                *value = to;
                            }
                        }
                    }
                }
                for one in moved.iter_mut() {
                    rename_uses(one, &renamed);
                }
            }
            if !any {
                break;
            }
        }
        if moved.is_empty() {
            changed |= folded;
            continue;
        }
        changed = true;
        for one in &moved {
            for &value in one.defines() {
                defined.insert(value, pre);
            }
        }
        for one in &moved {
            one.each_value(|used| {
                readers.entry(used).or_default().insert(pre_at);
            });
        }
        let insns = &mut blocks[pre_at].insns;
        let (leading, moved): (Vec<I>, Vec<I>) = moved.into_iter().partition(|one| policy.leads(one));
        let start = insns.iter().position(|one| !one.is_meta()).unwrap_or(insns.len());
        for (offset, one) in leading.into_iter().enumerate() {
            insns.insert(start + offset, one);
        }
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

/// `HasLoopPHIUse`: the values a phi of the loop (or of an exit block) takes,
/// and what feeds a copy of those. A copy that no phi takes is not one: the
/// pin looks past copies only for a phi (MachineLICM.cpp#L1166).
fn phi_values<I: Inst>(
    blocks: &[Block<I>],
    place: &HashMap<i64, usize>,
    inside: &BTreeSet<i64>,
) -> HashSet<u32> {
    let mut taken: HashSet<u32> = HashSet::default();
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
