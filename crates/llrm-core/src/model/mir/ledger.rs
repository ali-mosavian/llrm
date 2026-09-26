//! The rewrite ledger: where deleted source bytes land. No operation stays
//! in a body to own bytes it no longer computes. Each pass boundary strips
//! such tombstones out and records where their bytes land; the backend gets
//! them back only when the pipeline hands the body over (`materialized`).

use std::collections::BTreeSet;

use crate::support::hash::{HashMap, HashSet};

use super::{Kind, MirBlock, MirBody, Op};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Landing {
    /// Just before this operation.
    Before(u32),
    /// At the end of the block labelled here.
    End(i64),
    /// A block of its own where a vanished block was.
    Block { at: i64, cold: bool },
}

/// An operation a pass deleted, where what landed before it goes, and the
/// tombstone owning its bytes if it owned any.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Retired {
    pub id: u32,
    pub landing: Landing,
    pub tombstone: Option<Op>,
}

/// Bytes waiting at one landing that go to another: its operation left the
/// block, or the block ended differently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Relanded {
    pub from: Landing,
    pub to: Landing,
}

/// An operation that now only owns bytes: what `cleared` and
/// `_empty_operation` leave, with no name and nothing it reads, writes or
/// defines. A source `nop` is an instruction, not a tombstone.
pub fn is_tombstone(op: &Op) -> bool {
    op.kind == Kind::Nothing
        && op.name.is_empty()
        && op.raised.is_none()
        && op.target.is_none()
        && op.args.is_empty()
        && op.results.is_empty()
        && !op.absorbed.is_empty()
        && op.defines.is_empty()
        && op.uses.is_empty()
        && op.loads.is_empty()
        && op.stores.is_empty()
        && op.exits.is_empty()
}

/// `body` without tombstones, each with where its bytes land. An unreachable
/// block they leave empty goes too, and its bytes keep its place.
pub(super) fn stripped(body: &MirBody) -> Option<(MirBody, Vec<Retired>)> {
    if !body.blocks.iter().flat_map(|block| &block.ops).any(is_tombstone) {
        return None;
    }
    let reached = reachable(body);
    let mut retired = Vec::new();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let kept: Vec<Op> = block.ops.iter().filter(|op| !is_tombstone(op)).cloned().collect();
        let vanishes = kept.is_empty() && block.phis.is_empty() && !reached.contains(&block.at);
        for (at, op) in block.ops.iter().enumerate() {
            if !is_tombstone(op) {
                continue;
            }
            let landing = if vanishes {
                Landing::Block { at: block.at, cold: block.cold }
            } else {
                block.ops[at + 1..].iter().find(|next| !is_tombstone(next)).map_or(Landing::End(block.at), |next| Landing::Before(next.id.0))
            };
            retired.push(Retired { id: op.id.0, landing, tombstone: Some(op.clone()) });
        }
        if !vanishes {
            blocks.push(block.with_ops(kept));
        }
    }
    Some((body.with_blocks(blocks), retired))
}

fn reachable(body: &MirBody) -> BTreeSet<i64> {
    let succ: HashMap<i64, &Vec<i64>> = body.blocks.iter().map(|block| (block.at, &block.succ)).collect();
    let (mut reached, mut pending) = (BTreeSet::new(), vec![body.entry]);
    while let Some(at) = pending.pop() {
        if succ.contains_key(&at) && reached.insert(at) {
            pending.extend(succ[&at].iter().copied());
        }
    }
    reached
}

/// Where operations that `before` had and `after` lacks land, and where
/// bytes waiting before an operation that left its block, or at the end of a
/// block that now ends differently, go instead.
pub(super) fn relanded(before: &MirBody, after: &MirBody, stripped: &HashSet<u32>) -> (Vec<Retired>, Vec<Relanded>) {
    let shape = |body: &MirBody| body.blocks.iter().map(|block| (block.at, block.ops.iter().map(|op| op.id.0).collect::<Vec<_>>())).collect::<Vec<_>>();
    if stripped.is_empty() && shape(before) == shape(after) {
        return (Vec::new(), Vec::new());
    }
    let placed: HashMap<u32, (i64, usize)> = after
        .blocks
        .iter()
        .flat_map(|block| block.ops.iter().enumerate().map(move |(index, op)| (op.id.0, (block.at, index))))
        .collect();
    let blocks: HashMap<i64, &MirBlock> = after.blocks.iter().map(|block| (block.at, block)).collect();
    let was: HashMap<i64, &MirBlock> = before.blocks.iter().map(|block| (block.at, block)).collect();
    // A vanished block's place is where control from it went: the start of
    // the first block along its single-successor chain that survives.
    let vanished = |block: &MirBlock| {
        let mut seen = BTreeSet::new();
        let mut at = block;
        while let [next] = at.succ[..] {
            if !seen.insert(next) {
                break;
            }
            if let Some(now) = blocks.get(&next) {
                return now.ops.first().map_or(Landing::End(next), |first| Landing::Before(first.id.0));
            }
            let Some(&then) = was.get(&next) else { break };
            at = then;
        }
        Landing::Block { at: block.at, cold: block.cold }
    };
    let mut retired = Vec::new();
    let mut relanded = Vec::new();
    // Just after operation `id` where it stands now.
    let following = |id: u32| {
        let (at, index) = placed[&id];
        blocks[&at].ops.get(index + 1).map_or(Landing::End(at), |next| Landing::Before(next.id.0))
    };
    for block in &before.blocks {
        let survives = blocks.contains_key(&block.at);
        // Where this block's end went: nowhere if it survives; else just after
        // its last operation that survives anywhere, as a merged block's
        // operations carry its bytes along.
        let end = || match block.ops.iter().rev().find(|op| placed.contains_key(&op.id.0)) {
            _ if survives => Landing::End(block.at),
            Some(last) => following(last.id.0),
            None => vanished(block),
        };
        // Where something leaving position `at` of this block lands: the next
        // operation still in the block, or, if the block went, the next one
        // that survives anywhere.
        let landing = |at: usize| {
            let next = block.ops[at + 1..].iter().find(|next| match placed.get(&next.id.0) {
                Some(&(now, _)) => !survives || now == block.at,
                None => false,
            });
            next.map_or_else(end, |next| Landing::Before(next.id.0))
        };
        for (at, op) in block.ops.iter().enumerate() {
            match placed.get(&op.id.0) {
                None if !stripped.contains(&op.id.0) => retired.push(Retired { id: op.id.0, landing: landing(at), tombstone: None }),
                Some(&(now, _)) if survives && now != block.at => {
                    relanded.push(Relanded { from: Landing::Before(op.id.0), to: landing(at) })
                }
                _ => {}
            }
        }
        let ended = match blocks.get(&block.at) {
            None => Some(end()),
            Some(now) => {
                let had: HashSet<u32> = block.ops.iter().map(|op| op.id.0).collect();
                let last = now.ops.iter().rposition(|op| had.contains(&op.id.0));
                let first_new = last.map_or(0, |last| last + 1);
                now.ops.get(first_new).map(|next| Landing::Before(next.id.0))
            }
        };
        if let Some(to) = ended {
            relanded.push(Relanded { from: Landing::End(block.at), to });
        }
    }
    (retired, relanded)
}

/// Bytes waiting at each landing, in the order their tombstones would stand.
#[derive(Clone, Debug, Default)]
pub struct Ledger {
    pending: HashMap<Landing, Vec<Op>>,
}

impl Ledger {
    pub fn from_stages<'a>(stages: impl IntoIterator<Item = &'a super::Stage>) -> Self {
        let mut ledger = Self::default();
        for stage in stages {
            ledger.apply(stage);
        }
        ledger
    }

    /// One stage's deletions and moves. What arrives at a landing goes in
    /// front of what already waits there: it stood before it.
    pub fn apply(&mut self, stage: &super::Stage) {
        // Every landing names the stage's output, where one group's new
        // landing can be another's old one: take all before placing any.
        let retired: Vec<(Landing, Vec<Op>)> = stage
            .retired
            .iter()
            .map(|one| {
                let mut group = self.pending.remove(&Landing::Before(one.id)).unwrap_or_default();
                group.extend(one.tombstone.clone());
                (one.landing, group)
            })
            .collect();
        let relanded: Vec<(Landing, Vec<Op>)> =
            stage.relanded.iter().filter_map(|one| Some((one.to, self.pending.remove(&one.from)?))).collect();
        for (landing, group) in retired.into_iter().rev().chain(relanded.into_iter().rev()) {
            self.arrive(landing, group);
        }
    }

    fn arrive(&mut self, landing: Landing, mut group: Vec<Op>) {
        if group.is_empty() {
            return;
        }
        let waiting = self.pending.entry(landing).or_default();
        group.append(waiting);
        *waiting = group;
    }

    /// `body` with every tombstone back where its bytes land, for the backend.
    pub fn materialized(mut self, body: &MirBody) -> Result<MirBody, String> {
        if self.pending.is_empty() {
            return Ok(body.clone());
        }
        let mut blocks: Vec<MirBlock> = body
            .blocks
            .iter()
            .map(|block| {
                let mut ops = Vec::with_capacity(block.ops.len());
                for op in &block.ops {
                    ops.extend(self.pending.remove(&Landing::Before(op.id.0)).unwrap_or_default());
                    ops.push(op.clone());
                }
                ops.extend(self.pending.remove(&Landing::End(block.at)).unwrap_or_default());
                block.with_ops(ops)
            })
            .collect();
        let mut vanished: Vec<(i64, bool, Vec<Op>)> = Vec::new();
        for (landing, ops) in std::mem::take(&mut self.pending) {
            match landing {
                Landing::Block { at, cold } => vanished.push((at, cold, ops)),
                other => return Err(format!("ledger: {} tombstones land at {other:?}, which the body no longer has", ops.len())),
            }
        }
        vanished.sort_by_key(|one| one.0);
        for (at, cold, ops) in vanished {
            let place = blocks.iter().position(|block| block.at > at).unwrap_or(blocks.len());
            blocks.insert(place, MirBlock { cold, ..MirBlock::new(at, Vec::new(), ops, Vec::new()) });
        }
        Ok(body.with_blocks(blocks))
    }
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
