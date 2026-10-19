//! Where a block local's bytes are live, from its lifetime markers, so that
//! locals never live together can share a frame slot.
//!
//! # How it works
//! A local is tracked only if it is an `alloca` with a marker; every use of it
//! but its markers (pointer arithmetic followed) must fall inside its
//! lifetime. Any other local is live throughout and shares with none.
//!
//! A forward fixed point finds, for each block, the tracked locals live on
//! entry: a local is live at a block start if it is live at the end of any
//! predecessor. Inside a block, `llvm.lifetime.start` adds the object and
//! `llvm.lifetime.end` removes it. A second walk numbers the positions where
//! each object is live (both markers count) and merges them into segments.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','textColor':'#64748b','titleColor':'#64748b','gridColor':'#94a3b8','sectionBkgColor':'transparent','sectionBkgColor2':'transparent','altSectionBkgColor':'transparent','taskTextColor':'#ffffff','taskTextOutsideColor':'#64748b','taskTextLightColor':'#ffffff','taskTextDarkColor':'#ffffff','doneTaskBkgColor':'#16a34a','doneTaskBorderColor':'#166534','activeTaskBkgColor':'#2563eb','activeTaskBorderColor':'#1e40af','critBkgColor':'#dc2626','critBorderColor':'#991b1b','taskBkgColor':'#475569','taskBorderColor':'#334155','todayLineColor':'transparent'},'gantt':{'barHeight':34,'barGap':12,'topPadding':48,'leftPadding':150,'fontSize':14,'sectionFontSize':14,'numberSectionStyles':2}}}%%
//! gantt
//!   dateFormat X
//!   axisFormat %s
//!   tickInterval 1second
//!   section %a
//!   S∶ lifetime.start :milestone, t10, 2, 2
//!   segment of %a, positions 1 to 4 :done, t14, 2, 5
//!   s∶ store :milestone, t11, 3, 3
//!   l∶ load :milestone, t12, 4, 4
//!   E∶ lifetime.end :milestone, t13, 5, 5
//! ```
//!
//! *Positions of one alloca's lifetime markers and accesses* Axis: 1 position
//! 0; 2 1: lifetime.start; 3 2: store; 4 3: load; 5 4: lifetime.end; 6 position
//! 5.
//!
//! An access at a position where the object is not live (a use before its
//! start, or after its end) disqualifies it: it is dropped from the result and
//! so shares nothing.
//!
//! # Example
//! No example: no dump prints the intervals, and no test compares them as text.
//! Read `slots.rs`, where they decide which locals share a cell.
//!
//! # Used by
//! `isel` takes the intervals in its own instruction numbering; `slots` then
//! lets the locals whose intervals do not overlap share a slot.

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

/// The instructions that use `object` other than its markers, pointer
/// arithmetic on it followed: a load, a store, a call it is passed to. The
/// markers are trusted for what the code does not show, as LLVM's
/// StackColoring trusts them (`-protect-from-escaped-allocas` off): a pointer
/// to the object is not used past its end.
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
                (Opcode::GetElementPtr { .. }, 0) => pending.push(instruction.result?),
                _ => found.push(one.user),
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
        for inst in function.iter_block(block) {
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
            for inst in function.iter_block(block) {
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
        for inst in function.iter_block(block) {
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
