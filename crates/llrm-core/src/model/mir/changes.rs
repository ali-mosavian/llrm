//! What a pass did, read off operation identities: the record the rewrite
//! ledger consumes. Identity is `Op.id`, which the pass manager assigns;
//! content is everything `Op` equality compares.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::support::hash::{HashMap, HashSet};

use super::{MirBody, Op, OpId, next_id, python_padded_hex};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposition {
    /// The same operation with different content.
    Rewritten,
    /// Copied: `replacements` are the copies, the original among them if it stays.
    Cloned,
    Deleted,
    /// New, from no operation: `old` is `None`.
    Inserted,
}

/// One operation's fate across one pass. An operation the pass left exactly
/// as it was has no record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransformChange {
    pub old: Option<u32>,
    pub replacements: Vec<u32>,
    pub disposition: Disposition,
}

/// One pass's changes, under the stage name dumps use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stage {
    pub name: String,
    pub changes: Vec<TransformChange>,
}

/// A pipeline's result: the body, and what each stage did to reach it.
#[derive(Clone, Debug)]
pub struct Transformed {
    pub body: Rc<MirBody>,
    pub stages: Vec<Stage>,
}

/// Operations without an id of their own: unassigned, or shared with another.
pub fn identity(body: &MirBody) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = BTreeMap::<u32, i64>::new();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        if op.id.0 == 0 {
            problems.push(format!("{} {} has no id", python_padded_hex(op.at), op.name));
        } else if let Some(first) = seen.insert(op.id.0, op.at) {
            problems.push(format!("{} {} repeats id {} of {}", python_padded_hex(op.at), op.name, op.id.0, python_padded_hex(first)));
        }
    }
    problems
}

/// `body` with every operation named by an id of its own. Identity is the
/// pass manager's to keep, not each pass's: an operation a pass made has none
/// yet, and one it copied repeats the original's. Both get a fresh id; of the
/// copies, the one owning source bytes keeps the old one, else the first.
pub fn identified(body: Rc<MirBody>) -> Rc<MirBody> {
    identify(body).0
}

/// `identified`, and each fresh id with the id it was copied from.
fn identify(body: Rc<MirBody>) -> (Rc<MirBody>, Vec<(u32, Option<u32>)>) {
    let mut keeper = HashMap::<u32, (usize, usize, bool)>::default();
    let mut changed = false;
    for (block_index, block) in body.blocks.iter().enumerate() {
        for (op_index, op) in block.ops.iter().enumerate() {
            if op.id.0 == 0 {
                changed = true;
                continue;
            }
            let owns = !op.absorbed.is_empty();
            match keeper.get_mut(&op.id.0) {
                None => {
                    keeper.insert(op.id.0, (block_index, op_index, owns));
                }
                Some(kept) => {
                    changed = true;
                    if owns && !kept.2 {
                        *kept = (block_index, op_index, owns);
                    }
                }
            }
        }
    }
    if !changed {
        return (body, Vec::new());
    }
    let mut fresh = Vec::new();
    let mut body = (*body).clone();
    for (block_index, block) in body.blocks.iter_mut().enumerate() {
        for (op_index, op) in block.ops.iter_mut().enumerate() {
            let kept = keeper.get(&op.id.0).is_some_and(|kept| (kept.0, kept.1) == (block_index, op_index));
            if !kept {
                let copied = (op.id.0 != 0).then_some(op.id.0);
                op.id = OpId(next_id());
                fresh.push((op.id.0, copied));
            }
        }
    }
    (Rc::new(body), fresh)
}

/// `after`, what a pass made of `before`, with its operations identified,
/// and what the pass did to each.
pub fn transformed(before: &MirBody, after: Rc<MirBody>) -> (Rc<MirBody>, Vec<TransformChange>) {
    let (after, fresh) = identify(after);
    let changes = changes(before, &after, &fresh);
    (after, changes)
}

fn changes(before: &MirBody, after: &MirBody, fresh: &[(u32, Option<u32>)]) -> Vec<TransformChange> {
    fn listed(body: &MirBody) -> Vec<&Op> {
        body.blocks.iter().flat_map(|block| &block.ops).collect()
    }
    let (was, now) = (listed(before), listed(after));
    let rewritten = |id: u32| TransformChange { old: Some(id), replacements: vec![id], disposition: Disposition::Rewritten };
    // Most passes keep every operation where it was: compare in step.
    if fresh.is_empty() && was.len() == now.len() && was.iter().zip(&now).all(|(old, new)| old.id.0 == new.id.0) {
        return was.iter().zip(&now).filter(|(old, new)| old != new).map(|(_, new)| rewritten(new.id.0)).collect();
    }
    let old: HashMap<u32, &Op> = was.iter().map(|op| (op.id.0, *op)).collect();
    let copied: HashMap<u32, Option<u32>> = fresh.iter().copied().collect();
    let mut changes = Vec::new();
    let mut clones = BTreeMap::<u32, Vec<u32>>::new();
    for op in &now {
        let id = op.id.0;
        match (copied.get(&id), old.get(&id)) {
            (Some(Some(from)), _) => clones.entry(*from).or_default().push(id),
            (Some(None), _) | (None, None) => {
                changes.push(TransformChange { old: None, replacements: vec![id], disposition: Disposition::Inserted });
            }
            (None, Some(previous)) if previous != op => changes.push(rewritten(id)),
            (None, Some(_)) => {}
        }
    }
    let present: HashSet<u32> = now.iter().map(|op| op.id.0).collect();
    for (from, copies) in clones {
        let mut replacements: Vec<u32> = present.contains(&from).then_some(from).into_iter().collect();
        replacements.extend(copies);
        changes.retain(|one| one.old != Some(from));
        changes.push(TransformChange { old: Some(from), replacements, disposition: Disposition::Cloned });
    }
    let recorded: HashSet<u32> = changes.iter().filter_map(|one| one.old).collect();
    for op in &was {
        if !present.contains(&op.id.0) && !recorded.contains(&op.id.0) {
            changes.push(TransformChange { old: Some(op.id.0), replacements: Vec::new(), disposition: Disposition::Deleted });
        }
    }
    changes
}
