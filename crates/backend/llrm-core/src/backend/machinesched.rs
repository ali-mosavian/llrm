//! The dependency graph of a scheduling region of LIR, before allocation:
//! `ScheduleDAGInstrs` for the pre-RA machine scheduler (row 65).
//!
//! # How it works
//! A block is cut into regions at the instructions the scheduler may not move
//! across or reorder: terminators and calls, barriers, anything with a fixed
//! register or a clobber, a group, a spill or frame marker, a volatile
//! access, an instruction of unknown effect, and an x87 operation that
//! `floatregions::boundary` calls one. Within a region an edge runs from an
//! earlier instruction to a later one that must stay after it:
//!
//! ```text
//!  value        def -> use (data), use -> def (anti), def -> def (output)
//!  lanes        the same on the physical lanes and flags `liveness::effect` reads and writes
//!  memory       a store -> every later access, a load -> every later store
//! ```
//!
//! This is the pin's `buildSchedGraph` (MachineScheduler.cpp,
//! ScheduleDAGInstrs.cpp) without alias analysis: two memory accesses where one
//! writes are ordered.
//!
//! # Example
//! `x = load [m]; y = x + 1; z = load [n]; store [m], y`: the first load feeds
//! the add (data), the add feeds the store (data), both loads precede the store
//! (a load before a store), and the load of `n` has no edge to the first
//! load or the add.
//!
//! # Used by
//! The bottom-up list scheduler (pieces 2 and 3 of
//! docs/working/machine-scheduler-plan.md).

use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;

use llrm_lir::registers::Regs;

use crate::backend::liveness;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::Insn;

/// Whether `one` stays where it is, and nothing moves across it.
fn boundary(
    regs: Regs,
    bits: u32,
    one: &Insn,
) -> bool {
    let Some(what) = &one.what else { return true };
    matches!(
        what.op,
        Operation::Call
            | Operation::Branch
            | Operation::Jump
            | Operation::Return
            | Operation::Barrier
            | Operation::Escape
            | Operation::Fill
            | Operation::Copy
            | Operation::Nothing
    ) || one.call.is_some()
        || one.volatile()
        || one.group.is_some()
        || !one.clobbers.is_empty()
        || !one.clobbers_high.is_empty()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
        || !one.spread.is_empty()
        || one.frame_adjust
        || one.spill_reload
        || one.spill_store
        || liveness::effect(regs, bits, one).is_none()
        || crate::backend::floatregions::boundary(regs, one)
}

/// The runs of `insns` between boundaries, as index ranges; the boundaries
/// themselves are in none.
pub fn regions(
    insns: &[Arc<Insn>],
    regs: Regs,
    bits: u32,
) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = None;
    for (index, one) in insns.iter().enumerate() {
        if boundary(regs, bits, one) {
            if let Some(first) = start.take() {
                out.push(first..index);
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(first) = start {
        out.push(first..insns.len());
    }
    out
}

/// Whether `one` reads memory and whether it writes it. A `lea` names a cell
/// and reads none.
fn memory(one: &Insn) -> (bool, bool) {
    let Some(what) = &one.what else { return (true, true) };
    let reads = what.op != Operation::Address && what.sources.iter().any(|place| matches!(place, Loc::Mem(_)));
    let writes = what.dests.iter().any(|place| matches!(place, Loc::Mem(_)));
    (reads, writes)
}

/// The edges of a region, by position in it.
#[derive(Debug, Eq, PartialEq)]
pub struct Graph {
    /// For each instruction, the earlier ones that must stay before it.
    pub preds: Vec<Vec<usize>>,
    /// For each instruction, the later ones that must stay after it.
    pub succs: Vec<Vec<usize>>,
}

/// The graph of `insns`, one region (every instruction schedulable).
pub fn graph(
    insns: &[Arc<Insn>],
    regs: Regs,
    bits: u32,
) -> Graph {
    let n = insns.len();
    let effects: Vec<_> = insns.iter().map(|one| liveness::effect(regs, bits, one)).collect();
    let mut preds = vec![Vec::new(); n];
    let mut succs = vec![Vec::new(); n];
    for later in 0..n {
        let (later_reads_memory, later_writes_memory) = memory(&insns[later]);
        for earlier in 0..later {
            let (reads_memory, writes_memory) = memory(&insns[earlier]);
            let by_value = insns[earlier]
                .defines
                .iter()
                .any(|v| insns[later].uses.contains(v) || insns[later].defines.contains(v))
                || insns[earlier].uses.iter().any(|v| insns[later].defines.contains(v));
            let by_lane = match (&effects[earlier], &effects[later]) {
                (Some(first), Some(second)) => {
                    !first.writes.is_disjoint(&second.all_reads())
                        || !first.writes.is_disjoint(&second.writes)
                        || !first.all_reads().is_disjoint(&second.writes)
                }
                _ => true,
            };
            let by_memory =
                (writes_memory && (later_reads_memory || later_writes_memory)) || (reads_memory && later_writes_memory);
            if by_value || by_lane || by_memory {
                preds[later].push(earlier);
                succs[earlier].push(later);
            }
        }
    }
    Graph { preds, succs }
}

/// What a candidate does to the pressure of the region at its bottom edge:
/// `delta` values more are live once it is placed above everything already
/// placed.
fn pressure_after(
    one: &Insn,
    live: &HashSet<u32>,
) -> usize {
    let defined = one.defines.iter().filter(|value| live.contains(value)).count();
    let mut opened: Vec<u32> = one.uses.iter().copied().filter(|value| !live.contains(value)).collect();
    opened.sort_unstable();
    opened.dedup();
    live.len() - defined + opened.len()
}

/// The order of a region scheduled bottom-up as `GenericScheduler` does with
/// its register-pressure heuristics (MachineScheduler.cpp `tryCandidate`,
/// #L3965): among the instructions whose readers are all placed, the one that
/// leaves the least excess over `limit`, then the lowest pressure,
/// then the latest in the original order (`NodeOrder`). `live_out` holds the
/// values live below the region. Regions of at most `limit / 2` instructions
/// keep their order (`initPolicy`, #L3688: pressure is tracked only above
/// that), as a graph with no choice does.
pub fn bottom_up(
    insns: &[Arc<Insn>],
    graph: &Graph,
    live_out: &HashSet<u32>,
    limit: usize,
) -> Vec<usize> {
    let n = insns.len();
    if n <= limit / 2 {
        return (0..n).collect();
    }
    let mut waiting: Vec<usize> = graph.succs.iter().map(Vec::len).collect();
    let mut ready: Vec<usize> = (0..n).filter(|&node| waiting[node] == 0).collect();
    let mut live = live_out.clone();
    let mut placed = Vec::with_capacity(n);
    while !ready.is_empty() {
        let mut best = 0;
        for candidate in 1..ready.len() {
            let (cand, held) = (ready[candidate], ready[best]);
            let (cand_p, held_p) = (pressure_after(&insns[cand], &live), pressure_after(&insns[held], &live));
            let (cand_excess, held_excess) = (cand_p.saturating_sub(limit), held_p.saturating_sub(limit));
            let better = if cand_excess != held_excess {
                cand_excess < held_excess
            } else if cand_p != held_p {
                cand_p < held_p
            } else {
                cand > held
            };
            if better {
                best = candidate;
            }
        }
        let node = ready.swap_remove(best);
        for value in &insns[node].defines {
            live.remove(value);
        }
        live.extend(insns[node].uses.iter().copied());
        placed.push(node);
        for &before in &graph.preds[node] {
            waiting[before] -= 1;
            if waiting[before] == 0 {
                ready.push(before);
            }
        }
    }
    placed.reverse();
    placed
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{graph, regions};
    use crate::model::ir::{Held, Loc, Mem, Operation, Semantics};
    use crate::model::lir::Insn;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
    }

    fn cell(offset: i64) -> Loc {
        Loc::Mem(Mem { offset, ..Mem::new(None, 4) })
    }

    fn semantics(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Semantics {
        Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }
    }

    fn insn(
        at: i64,
        what: Semantics,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
    }

    fn body() -> Vec<Arc<Insn>> {
        vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![cell(0)]), vec![1], vec![]),
            insn(
                1,
                semantics(
                    Operation::Binary,
                    "add",
                    vec![held(2)],
                    vec![held(1), Loc::Imm(crate::model::ir::Imm { value: 1, width: 4, address: None })],
                ),
                vec![2],
                vec![1],
            ),
            insn(2, semantics(Operation::Move, "mov", vec![held(3)], vec![cell(8)]), vec![3], vec![]),
            insn(3, semantics(Operation::Move, "mov", vec![cell(0)], vec![held(2)]), vec![], vec![2]),
        ]
    }

    /// `x = load [m]; y = x + 1; z = load [n]; store [m], y`: a load feeds the
    /// add, the add and both loads precede the store, and the load of `n`
    /// is ordered with neither the first load nor the add.
    #[test]
    fn test_the_graph_orders_data_and_a_load_before_a_store_and_nothing_else() {
        let insns = body();
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let found = graph(&insns, regs, 16);
        assert_eq!(found.preds[1], [0], "the add reads the first load");
        assert!(found.preds[2].is_empty(), "the second load is ordered with nothing before it: {:?}", found.preds);
        assert_eq!(found.preds[3], [0, 1, 2], "the store follows both loads and the add");
    }

    /// A call, a terminator and a volatile access are in no region; the runs
    /// between are.
    #[test]
    fn test_regions_are_the_runs_between_boundaries() {
        let mut insns = body();
        insns.insert(2, insn(9, semantics(Operation::Call, "call", vec![], vec![]), vec![], vec![]));
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        assert_eq!(regions(&insns, regs, 16), vec![0..2, 3..5]);
    }

    /// Three loads feed two adds and a store. In source order all three loads
    /// are live together (3 values); bottom-up with a limit of two places
    /// the third load just above the add that reads it, so at most two are
    /// live.
    #[test]
    fn test_bottom_up_places_a_load_by_its_reader_when_the_limit_is_two() {
        use super::bottom_up;
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![cell(0)]), vec![1], vec![]),
            insn(1, semantics(Operation::Move, "mov", vec![held(2)], vec![cell(8)]), vec![2], vec![]),
            insn(2, semantics(Operation::Move, "mov", vec![held(3)], vec![cell(16)]), vec![3], vec![]),
            insn(3, semantics(Operation::Binary, "add", vec![held(4)], vec![held(1), held(2)]), vec![4], vec![1, 2]),
            insn(4, semantics(Operation::Binary, "add", vec![held(5)], vec![held(4), held(3)]), vec![5], vec![4, 3]),
            insn(5, semantics(Operation::Binary, "add", vec![held(6)], vec![held(5), imm(1)]), vec![6], vec![5]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let found = graph(&insns, regs, 16);
        let live_out: std::collections::HashSet<u32> = [6].into_iter().collect();
        // The loads read memory and nothing writes it: no edge among them.
        assert_eq!(bottom_up(&insns, &found, &live_out, 2), [0, 1, 3, 2, 4, 5]);
        // A limit that is never reached leaves the order alone.
        assert_eq!(bottom_up(&insns, &found, &live_out, 16), [0, 1, 2, 3, 4, 5]);
    }
}
