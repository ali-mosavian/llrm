//! A phi cycle that carries one value is that value, and one nothing reads is
//! gone: `OptimizePHIs` before MachineCSE. Translated from
//! [OptimizePHIs.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/OptimizePHIs.cpp)
//! (`IsSingleValuePHICycle`, `IsDeadPHICycle`, `OptimizeBB`).
//!
//! # How it works
//! For each phi, the phis it reads are followed (at most 16, through plain
//! register copies too). If every other input is one value, the phi is that
//! value: its result is renamed and it goes. If every reader of its result is
//! a phi that is itself read only by phis in the cycle, the cycle goes.
//!
//! LLVM's `InstCombine` folds `phi [a, self]` in IR. A machine phi of the kind
//! comes from DAG legalization, "e.g., when i64 values are split up for 32-bit
//! targets"; ours from isel splitting a far pointer's phi in an offset and a
//! selector that a constant step leaves as it was. Left to the lowering it is
//! three copies and a register of its own, and a selector that must sit in ES
//! is copied in and out of it each trip.
//!
//! # Example
//! `bench/huge/huge.c` at `-m16 -O2`, `ISEL_DUMP=1`, stage `MachineSink`: the
//! sum loop's selector `v26` is carried round the loop and never changed.
//!
//! ```lir
//! block 0x0026
//!   v25 := phi 0x001d:v73 0x0026:v79
//!   v26 := phi 0x001d:v72 0x0026:v26
//!   0x0029  v77 := mov [[es:r0+0x0] v25@unplaced]
//! ```
//!
//! After the pass the load reads `v72` and the phi is gone.
//!
//! DEVIATE O1 (no number): the pin's `constrainRegClass` of the single value to
//! the replaced register's class is not asked: a value's class here is what
//! its readers need, so the readers moved to it bring theirs. Exit: classes
//! are on LIR values.
//!
//! # Runs
//! After `FarIndirectCalls`, before [`crate::backend::phiconst`]. Every level.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBody, Phi};
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;

pub struct OptimizePhis;

/// How many phis the pin follows before it gives up.
const LIMIT: usize = 16;

impl LIRTransform for OptimizePhis {
    fn class_name(&self) -> &'static str {
        "OptimizePHIs"
    }

    fn name(&self) -> &str {
        "optimizephis"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        optimized(&body)
    }
}

/// `body` with its single-value and dead phi cycles gone.
pub fn optimized(body: &LirBody) -> Result<LirBody, String> {
    let mut body = body.clone();
    // Each change can open another (a cycle's other phis become trivial), so
    // go again until a round finds none. The pin walks its blocks once and
    // replaces in place, which reaches the same.
    while let Some(next) = round(&body)? {
        body = next;
    }
    Ok(body)
}

// The rounds `optimized` made, for a test that independent changes share one.
#[cfg(test)]
thread_local! {
    static ROUNDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// What a round learns about the body.
struct Facts {
    phis: IndexMap<u32, Vec<(i64, u32)>>,
    /// The value a plain register copy reads, for the copy's result.
    copies: IndexMap<u32, u32>,
    /// Who reads a value: a phi's result, or `None` for any other reader.
    readers: IndexMap<u32, Vec<Option<u32>>>,
}

impl Facts {
    fn of(body: &LirBody) -> Self {
        let mut phis = IndexMap::default();
        let mut copies = IndexMap::default();
        let mut readers: IndexMap<u32, Vec<Option<u32>>> = IndexMap::default();
        for block in &body.blocks {
            for phi in block.phis.iter() {
                phis.insert(phi.result, phi.incoming.clone());
                for &(_, value) in &phi.incoming {
                    readers.entry(value).or_default().push(Some(phi.result));
                }
            }
            for one in &block.insns {
                for &value in &one.uses {
                    readers.entry(value).or_default().push(None);
                }
                if let Some((into, from)) = plain_copy(one) {
                    copies.insert(into, from);
                }
            }
        }
        Self { phis, copies, readers }
    }

    /// `IsSingleValuePHICycle`.
    fn single(
        &self,
        result: u32,
        single: &mut Option<u32>,
        seen: &mut BTreeSet<u32>,
    ) -> bool {
        if !seen.insert(result) {
            return true;
        }
        if seen.len() == LIMIT {
            return false;
        }
        for &(_, mut source) in &self.phis[&result] {
            if source == result {
                continue;
            }
            // Over a register-to-register move.
            if let Some(&from) = self.copies.get(&source) {
                source = from;
            }
            if self.phis.contains_key(&source) {
                if !self.single(source, single, seen) {
                    return false;
                }
            } else {
                if single.is_some_and(|one| one != source) {
                    return false;
                }
                *single = Some(source);
            }
        }
        true
    }

    /// `IsDeadPHICycle`.
    fn dead(
        &self,
        result: u32,
        seen: &mut BTreeSet<u32>,
    ) -> bool {
        if !seen.insert(result) {
            return true;
        }
        if seen.len() == LIMIT {
            return false;
        }
        self.readers.get(&result).into_iter().flatten().all(|reader| reader.is_some_and(|phi| self.dead(phi, seen)))
    }
}

/// The body after the changes a round finds, or `None`. A change that touches
/// a phi an earlier one of the round touched waits for the next round: the pin
/// walks its blocks once and replaces in place, and each change here rebuilt
/// the facts and the body (a switch of N cases with N phis took N rounds).
fn round(body: &LirBody) -> Result<Option<LirBody>, String> {
    #[cfg(test)]
    ROUNDS.with(|n| n.set(n.get() + 1));
    let facts = Facts::of(body);
    let placed: BTreeSet<i64> = body.blocks.iter().map(|one| one.at).collect();
    let mut rename: IndexMap<u32, u32> = IndexMap::default();
    let mut gone: BTreeSet<u32> = BTreeSet::new();
    for block in &body.blocks {
        for phi in block.phis.iter() {
            if phi.incoming.iter().any(|(at, _)| !placed.contains(at)) || gone.contains(&phi.result) {
                continue;
            }
            let (mut single, mut seen) = (None, BTreeSet::new());
            if facts.single(phi.result, &mut single, &mut seen)
                && let Some(value) = single
            {
                rename.insert(phi.result, value);
                gone.insert(phi.result);
                continue;
            }
            let mut seen = BTreeSet::new();
            if facts.dead(phi.result, &mut seen)
                && seen.is_disjoint(&gone)
                && seen.iter().all(|one| !rename.contains_key(one))
            {
                gone.extend(seen);
            }
        }
    }
    if gone.is_empty() {
        return Ok(None);
    }
    renamed(body, &rename, &gone).map(Some)
}

/// `body` with `rename` applied everywhere and the phis `gone` removed.
fn renamed(
    body: &LirBody,
    rename: &IndexMap<u32, u32>,
    gone: &BTreeSet<u32>,
) -> Result<LirBody, String> {
    let name = |value: u32| -> Result<u32, String> { Ok(rename.get(&value).copied().unwrap_or(value)) };
    let mut out = body.clone();
    for block in &mut out.blocks {
        if !rename.is_empty() {
            block.insns = block.insns.iter().map(|one| one.try_renamed(&name)).collect::<Result<_, String>>()?;
        }
        block.phis = block
            .phis
            .iter()
            .filter(|phi| !gone.contains(&phi.result))
            .map(|phi| {
                Ok(Phi {
                    result: name(phi.result)?,
                    incoming: phi
                        .incoming
                        .iter()
                        .map(|&(at, value)| Ok((at, name(value)?)))
                        .collect::<Result<_, String>>()?,
                })
            })
            .collect::<Result<_, String>>()?;
    }
    Ok(out)
}

/// `into := mov from` between registers, nothing else about it.
pub(crate) fn plain_copy(one: &Arc<Insn>) -> Option<(u32, u32)> {
    let what = one.what.as_ref()?;
    let ([Loc::Held(into)], [Loc::Held(from)]) = (what.dests.as_slice(), what.sources.as_slice()) else { return None };
    (what.op == Operation::Move
        && what.target.is_none()
        && !what.indirect
        && into.width == from.width
        && one.defines == [into.value]
        && one.uses == [from.value]
        && one.call.is_none()
        && one.group.is_none()
        && !one.volatile()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.clobbers.is_empty())
    .then_some((into.value, from.value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Held, Imm, Semantics};
    use crate::model::lir::{Block, LirBlock};

    fn what(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        target: Option<i64>,
    ) -> Option<Semantics> {
        Some(Semantics { name: Some(name.into()), dests, sources, target, ..Semantics::new(op) })
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: Vec<i64>,
        phis: Vec<Phi>,
    ) -> LirBlock {
        LirBlock { at, insns: insns.into(), succ: succ.into(), phis: phis.into(), cold: false, align: 0, max_skip: 0 }
    }

    fn body(
        name: &str,
        blocks: Vec<Block>,
    ) -> LirBody {
        LirBody::new(name, 0, blocks, IndexMap::default(), IndexMap::default())
    }

    fn held(
        value: u32,
        width: u32,
    ) -> Loc {
        Loc::Held(Held { value, width })
    }

    fn imm(
        value: i64,
        width: u32,
    ) -> Loc {
        Loc::Imm(Imm { value, width, address: None })
    }

    fn mov(
        at: i64,
        value: u32,
        n: i64,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            Some((at, at + 1)),
            what(Operation::Move, "mov", vec![held(value, 2)], vec![imm(n, 2)], None),
            vec![value],
            vec![],
        ))
    }

    fn read(
        at: i64,
        into: u32,
        value: u32,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            Some((at, at + 1)),
            what(Operation::Binary, "add", vec![held(into, 2)], vec![held(value, 2), imm(1, 2)], None),
            vec![into],
            vec![value],
        ))
    }

    fn looped(phis: Vec<Phi>) -> LirBody {
        body(
            "looped",
            vec![
                block(0, vec![mov(0, 1, 7), mov(1, 2, 8)], vec![20], vec![]).into(),
                block(20, vec![read(20, 9, 3)], vec![20, 30], phis).into(),
                block(30, vec![], vec![], vec![]).into(),
            ],
        )
    }

    /// bench/huge -O2: a far pointer split in offset and selector left the
    /// selector's `v26 := phi [entry: v72, loop: v26]`; its copies, `ES := mov
    /// BX` and `BX := mov ES` each trip, cost two instructions an element.
    #[test]
    fn a_phi_that_feeds_itself_round_the_loop_is_its_entry_value() {
        let done = optimized(&looped(vec![Phi { result: 3, incoming: vec![(0, 1), (20, 3)] }])).unwrap();
        assert!(done.blocks.iter().all(|one| one.phis.is_empty()), "the phi is gone");
        assert!(done.blocks[1].insns.iter().any(|one| one.uses == [1]), "the loop reads the entry value");
    }

    /// Two phis that pass a value to each other are one value, whatever order
    /// the pin meets them in.
    #[test]
    fn a_cycle_of_two_phis_carrying_one_value_is_that_value() {
        let done = optimized(&looped(vec![
            Phi { result: 3, incoming: vec![(0, 1), (20, 4)] },
            Phi { result: 4, incoming: vec![(0, 1), (20, 3)] },
        ]))
        .unwrap();
        assert!(done.blocks.iter().all(|one| one.phis.is_empty()));
        assert!(done.blocks[1].insns.iter().any(|one| one.uses == [1]));
    }

    #[test]
    fn a_phi_with_two_different_values_stays() {
        let done = optimized(&looped(vec![Phi { result: 3, incoming: vec![(0, 1), (20, 2)] }])).unwrap();
        assert_eq!(done.blocks[1].phis.len(), 1);
    }

    /// A phi only another phi reads, which only it reads: nothing uses them.
    #[test]
    fn a_dead_phi_cycle_goes() {
        let done = optimized(&looped(vec![
            Phi { result: 3, incoming: vec![(0, 1), (20, 4)] },
            Phi { result: 4, incoming: vec![(0, 2), (20, 3)] },
            Phi { result: 5, incoming: vec![(0, 2), (20, 6)] },
            Phi { result: 6, incoming: vec![(0, 1), (20, 5)] },
        ]))
        .unwrap();
        // 3 and 4 differ, but 3 is read by the add: only 5 and 6 are dead.
        let left: Vec<u32> = done.blocks[1].phis.iter().map(|phi| phi.result).collect();
        assert_eq!(left, vec![3, 4]);
    }

    /// A switch of N cases left N phis that each carry one value: every change
    /// rebuilt the facts and the body (N rounds; `cases` at 256, x2.31 per
    /// doubling). Changes that touch different phis share a round.
    #[test]
    fn independent_phis_that_feed_themselves_are_removed_in_one_round() {
        let phis: Vec<Phi> = (0..300).map(|k| Phi { result: 100 + k, incoming: vec![(0, 1), (20, 100 + k)] }).collect();
        ROUNDS.with(|n| n.set(0));
        let done = optimized(&looped(phis)).unwrap();
        assert!(done.blocks.iter().all(|one| one.phis.is_empty()), "the phis are gone");
        let rounds = ROUNDS.with(std::cell::Cell::get);
        assert!(rounds <= 3, "{rounds} rounds for 300 independent phis");
    }
}
