//! QuickrBASIC's use-before-definition warning: a forward "definitely
//! assigned" analysis over one function's finished blocks.

use std::collections::{BTreeMap, BTreeSet};

use super::{Block, Operand};

/// The first load of each `tracked` place that some path from the entry
/// reaches before any store to it, as `(place, instruction)`.
///
/// Anything else that names a place (an address for a runtime reader, a
/// BYREF argument) may write it, so it counts as an assignment.  A call in
/// `assigns_all` (a user procedure that may share module variables) assigns
/// every place.  Blocks entered from outside the CFG (`entries`: RESUME
/// targets, error handlers) start with everything assigned: they are
/// reached from any point of the function.
pub(super) fn unassigned_reads(
    blocks: &[Block],
    tracked: &BTreeSet<u32>,
    assigns_all: &BTreeSet<u32>,
    entries: &[u32],
) -> Vec<(u32, u32)> {
    let Some(entry) = blocks.first().map(|block| block.id) else {
        return Vec::new();
    };
    let mut predecessors: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for block in blocks {
        for target in block.terminator.iter().flat_map(|one| &one.targets) {
            predecessors.entry(*target).or_default().push(block.id);
        }
    }
    // `None` is "everything assigned", the top of the intersection lattice.
    let mut exits: BTreeMap<u32, Option<BTreeSet<u32>>> =
        blocks.iter().map(|block| (block.id, None)).collect();
    let entry_state = |this: &BTreeMap<u32, Option<BTreeSet<u32>>>, id: u32| {
        if id == entry {
            return Some(BTreeSet::new());
        }
        if entries.contains(&id) {
            return None;
        }
        let mut state: Option<BTreeSet<u32>> = None;
        for predecessor in predecessors.get(&id).into_iter().flatten() {
            let Some(exit) = &this[predecessor] else {
                continue;
            };
            state = Some(match state {
                None => exit.clone(),
                Some(state) => state.intersection(exit).copied().collect(),
            });
        }
        state
    };
    let mut changed = true;
    while changed {
        changed = false;
        for block in blocks {
            let mut state = entry_state(&exits, block.id);
            if let Some(assigned) = &mut state {
                for instruction in &block.instructions {
                    transfer(instruction, tracked, assigns_all, assigned, &mut |_| {});
                }
            }
            if exits[&block.id] != state {
                exits.insert(block.id, state);
                changed = true;
            }
        }
    }
    let mut reported = BTreeSet::new();
    let mut reads = Vec::new();
    for block in blocks {
        let Some(mut assigned) = entry_state(&exits, block.id) else {
            continue;
        };
        for instruction in &block.instructions {
            transfer(instruction, tracked, assigns_all, &mut assigned, &mut |place| {
                if reported.insert(place) {
                    reads.push((place, instruction.id));
                }
            });
        }
    }
    reads
}

fn transfer(
    instruction: &super::Instruction,
    tracked: &BTreeSet<u32>,
    assigns_all: &BTreeSet<u32>,
    assigned: &mut BTreeSet<u32>,
    unassigned_read: &mut dyn FnMut(u32),
) {
    if assigns_all.contains(&instruction.id) {
        assigned.extend(tracked);
    }
    match (instruction.op, instruction.operands.as_slice()) {
        ("load", [Operand::Place(place)]) => {
            if tracked.contains(place) && !assigned.contains(place) {
                unassigned_read(*place);
            }
        }
        ("store", [Operand::Place(place), ..]) => {
            assigned.insert(*place);
        }
        _ => assigned.extend(instruction.operands.iter().filter_map(|operand| match operand {
            Operand::Place(place) => Some(*place),
            _ => None,
        })),
    }
}
