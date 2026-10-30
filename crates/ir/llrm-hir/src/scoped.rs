//! Where a loop that states `NoFarAllocation` brackets its descriptor words.
//!
//! `llvm.invariant.start` before the loop and `llvm.invariant.end` after it
//! say the words hold for as long as it runs; the pin's LICM then lets their
//! loads leave it. The scope is made where the loop has a preheader that
//! nothing else enters the header by and exits only its own blocks reach, and
//! the loop does not write a descriptor word.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::domtree_address::AddressDominance;
use llrm_support::hash::HashMap;

use crate::facts::Region;
use crate::model::{Function, Op, Operand, TerminatorKind};

/// A descriptor word a loop loads through a value defined before it.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Word {
    pub base: i64,
    pub offset: i64,
    /// The HIR type of the word.
    pub ty: i64,
}

pub(crate) struct Bracket {
    pub preheader: i64,
    pub body: BTreeSet<i64>,
    pub exits: BTreeSet<i64>,
    pub words: BTreeSet<Word>,
}

/// The loops of `function` to bracket: the outermost of those whose back edge
/// is in `quiet`.
pub(crate) fn brackets(
    function: &Function,
    quiet: impl Fn(i64) -> bool,
    regions: &HashMap<(i64, i64), Region>,
) -> Vec<Bracket> {
    let successors = |block: &crate::model::Block| -> Vec<i64> {
        block.terminator.targets.iter().chain(block.terminator.cases.iter().map(|(_, target)| target)).copied().collect()
    };
    let dominance = AddressDominance::of(
        function.blocks.iter().map(|one| (one.id, one.terminator.targets.as_slice())),
        Some(function.entry),
    );
    let mut predecessors: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    for block in &function.blocks {
        for next in successors(block) {
            predecessors.entry(next).or_default().push(block.id);
        }
    }
    // The natural loop of each header, over the back edges that state the fact.
    let mut loops: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
    for latch in function.blocks.iter().filter(|one| quiet(one.id) && dominance.reachable(one.id)) {
        for header in successors(latch).into_iter().filter(|&next| dominance.dominates(next, latch.id)) {
            let body = loops.entry(header).or_default();
            body.insert(header);
            let mut work = vec![latch.id];
            while let Some(block) = work.pop() {
                if body.insert(block) {
                    work.extend(predecessors.get(&block).into_iter().flatten().copied());
                }
            }
        }
    }
    let mut candidates: Vec<(i64, BTreeSet<i64>)> = loops.into_iter().collect();
    candidates.sort_by_key(|(_, body)| std::cmp::Reverse(body.len()));
    let defined: HashMap<i64, i64> = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter().flat_map(move |one| one.results.iter().map(move |&r| (r, block.id))))
        .collect();
    let mut kept: Vec<Bracket> = Vec::new();
    for (header, body) in candidates {
        if kept.iter().any(|outer| body.is_subset(&outer.body)) {
            continue;
        }
        let outside: Vec<i64> =
            predecessors.get(&header).into_iter().flatten().copied().filter(|one| !body.contains(one)).collect();
        let [preheader] = outside.as_slice() else { continue };
        let single = function
            .blocks
            .iter()
            .find(|one| one.id == *preheader)
            .is_some_and(|one| one.terminator.kind == TerminatorKind::Jump && one.terminator.targets == [header]);
        if !single {
            continue;
        }
        let mut exits = BTreeSet::new();
        let mut shared = true;
        for block in function.blocks.iter().filter(|one| body.contains(&one.id)) {
            for next in successors(block).into_iter().filter(|next| !body.contains(next)) {
                shared &= predecessors[&next].iter().all(|one| body.contains(one));
                exits.insert(next);
            }
        }
        if !shared {
            continue;
        }
        let mut words = BTreeSet::new();
        let mut written = false;
        for block in function.blocks.iter().filter(|one| body.contains(&one.id)) {
            for one in &block.instructions {
                let described = regions.get(&(function.id, one.id)) == Some(&Region::Descriptor);
                match (one.op, one.operands.first()) {
                    (Op::Store, _) if described => written = true,
                    (Op::Load, Some(Operand::IndirectPlace(place))) if described => {
                        let before = match defined.get(&place.base) {
                            Some(&at) => !body.contains(&at) && dominance.dominates(at, *preheader),
                            None => function.parameters.contains(&place.base),
                        };
                        if before {
                            words.insert(Word { base: place.base, offset: place.offset, ty: place.r#type });
                        }
                    }
                    _ => {}
                }
            }
        }
        if !written && !words.is_empty() {
            kept.push(Bracket { preheader: *preheader, body, exits, words });
        }
    }
    kept
}
