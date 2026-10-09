//! Where a block local's bytes are live, from its lifetime markers: from a
//! `llvm.lifetime.start` along every path to where a `.end` or the return
//! comes. Two locals live never together may share a frame slot (`slots`).
//!
//! A local is tracked only if it has a marker and nothing but loads, stores and
//! pointer arithmetic on it and its markers touch it, and each access is where
//! it is live; any other local is live throughout and shares with none.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::module::{BlockId, Function, InstId, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;

use crate::analysis::intervals::{Interval, Segment};
use crate::support::hash::IndexMap;

/// What a lifetime marker says: its object, and whether it starts or ends.
pub struct Marker {
    pub object: ValueId,
    pub starts: bool,
}

/// `object`'s accesses, or none where it is used any other way: stored as a
/// value, passed on, compared. Pointer arithmetic on it is followed.
fn accesses(
    function: &Function,
    object: ValueId,
    marker: &impl Fn(InstId) -> Option<Marker>,
) -> Option<Vec<InstId>> {
    let mut found = Vec::new();
    let mut pending = vec![object];
    while let Some(value) = pending.pop() {
        for one in function.users(value) {
            let instruction = function.instruction(one.user);
            match (&instruction.opcode, one.index) {
                _ if marker(one.user).is_some() => {}
                (Opcode::Load { .. }, 0) | (Opcode::Store { .. }, 1) => found.push(one.user),
                (Opcode::GetElementPtr { .. }, 0) => pending.push(instruction.result?),
                _ => return None,
            }
        }
    }
    Some(found)
}

/// The tracked locals' live ranges in `positions`' numbering.
pub fn intervals(
    function: &Function,
    layout: &[BlockId],
    positions: &IndexMap<InstId, i64>,
    marker: impl Fn(InstId) -> Option<Marker>,
) -> IndexMap<ValueId, Interval> {
    let in_layout: BTreeSet<BlockId> = layout.iter().copied().collect();
    let mut tracked: BTreeMap<ValueId, Vec<InstId>> = BTreeMap::new();
    for &block in layout {
        for &inst in function.block(block).instructions() {
            if let Some(Marker { object, .. }) = marker(inst)
                && matches!(
                    function.value(object).def,
                    ValueDef::Instruction(def) if matches!(function.instruction(def).opcode, Opcode::Alloca { .. })
                )
                && let Some(touching) = accesses(function, object, &marker)
            {
                tracked.entry(object).or_insert(touching);
            }
        }
    }
    if tracked.is_empty() {
        return IndexMap::default();
    }
    // The tracked locals live where each block starts: from any predecessor.
    let step = |state: &mut BTreeSet<ValueId>, inst: InstId| {
        if let Some(Marker { object, starts }) = marker(inst).filter(|one| tracked.contains_key(&one.object)) {
            if starts {
                state.insert(object);
            } else {
                state.remove(&object);
            }
        }
    };
    let mut entering: BTreeMap<BlockId, BTreeSet<ValueId>> =
        layout.iter().map(|&block| (block, BTreeSet::new())).collect();
    loop {
        let mut changed = false;
        for &block in layout {
            let mut state = entering[&block].clone();
            for &inst in function.block(block).instructions() {
                step(&mut state, inst);
            }
            for successor in function.successors(block).into_iter().filter(|one| in_layout.contains(one)) {
                let into = entering.get_mut(&successor).expect("a block of the layout");
                for &value in &state {
                    changed |= into.insert(value);
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut segments: BTreeMap<ValueId, Vec<Segment>> = BTreeMap::new();
    let mut violated: BTreeSet<ValueId> = BTreeSet::new();
    for &block in layout {
        let mut state = entering[&block].clone();
        for &inst in function.block(block).instructions() {
            let at = positions[&inst];
            // Live at a marker that starts it, though it enters the state
            // after.
            let started =
                marker(inst).filter(|one| one.starts && tracked.contains_key(&one.object)).map(|one| one.object);
            for &object in state.iter().chain(started.iter()) {
                let runs = segments.entry(object).or_default();
                match runs.last_mut() {
                    Some(last) if last.end == at => last.end = at + 1,
                    Some(last) if last.end == at + 1 => {}
                    _ => runs.push(Segment { start: at, end: at + 1 }),
                }
            }
            for (&object, touching) in &tracked {
                if touching.contains(&inst) && !state.contains(&object) {
                    violated.insert(object);
                }
            }
            step(&mut state, inst);
        }
    }
    tracked
        .keys()
        .filter(|object| !violated.contains(object))
        .map(|&object| {
            let mut runs = segments.remove(&object).unwrap_or_default();
            runs.sort_by_key(|one| (one.start, one.end));
            let mut merged: Vec<Segment> = Vec::new();
            for run in runs {
                match merged.last_mut() {
                    Some(last) if run.start <= last.end => last.end = last.end.max(run.end),
                    _ => merged.push(run),
                }
            }
            (object, Interval::new(object.0, merged))
        })
        .collect()
}
