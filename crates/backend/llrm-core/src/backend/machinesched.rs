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
//! # Runs
//! `MachineScheduler`, a machine phase after `pressuresink` and before
//! `floatfold` (`machinesched@5`): each region longer than half the allocatable
//! registers is ordered by `bottom_up` (pieces 2 and 3 of
//! docs/working/machine-scheduler-plan.md). DEVIATE MS3: values that pass
//! through a block are not counted in its pressure, and one limit stands for
//! every register set.

use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::Regs;

use crate::backend::allocate::{LiveAt, live_rows};
use crate::backend::classes::RegisterClasses;
use crate::backend::liveness;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;

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

/// CF, PF, AF, ZF, SF, OF of rflags.
const ARITHMETIC_FLAGS: u32 = 0x8d5;

/// A move of the constant zero into a value.
fn zero_move(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| what.op == Operation::Move
                && matches!(
                    (what.dests.as_slice(), what.sources.as_slice()),
                    ([Loc::Held(_)], [Loc::Imm(crate::model::ir::Imm { value: 0, address: None, .. })])
                ),
        )
}

/// An x87 compare, whose answer the stackifier moves into EFLAGS.
fn float_compare(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| {
        what.op == Operation::Compare
            && what
                .name
                .as_deref()
                .is_some_and(|name| name.starts_with("fcom") || name.starts_with("ficom") || name.starts_with("fucom"))
    })
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
    let mut effects: Vec<_> = insns.iter().map(|one| liveness::effect(regs, bits, one)).collect();
    // `MOV32r0` writes EFLAGS (it becomes `xor`): a zero made between a flag
    // writer and its reader would break `add; adc` (the pin's scheduler
    // sees the def). An `fcom` writes them too: the stackifier follows it
    // with `fnstsw ax; sahf`, and a `seta` hoisted above it read stale flags.
    for (one, effect) in insns.iter().zip(&mut effects) {
        if let Some(effect) = effect {
            if zero_move(one) || float_compare(one) {
                effect.writes = effect.writes.or(&crate::backend::peephole::Lanes::flags(ARITHMETIC_FLAGS));
            }
        }
    }
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
    counted: &impl Fn(u32) -> bool,
) -> usize {
    let defined = one.defines.iter().filter(|value| live.contains(value)).count();
    let mut opened: Vec<u32> =
        one.uses.iter().copied().filter(|&value| counted(value) && !live.contains(&value)).collect();
    opened.sort_unstable();
    opened.dedup();
    live.len() - defined + opened.len()
}

/// The most values live at once while `order` (program order, as indexes into
/// `insns`) runs above `live_out`.
fn peak(
    insns: &[Arc<Insn>],
    order: &[usize],
    live_out: &HashSet<u32>,
    counted: &impl Fn(u32) -> bool,
) -> usize {
    let mut live = live_out.clone();
    let mut most = live.len();
    for &node in order.iter().rev() {
        for value in &insns[node].defines {
            live.remove(value);
        }
        live.extend(insns[node].uses.iter().copied().filter(|&value| counted(value)));
        most = most.max(live.len());
    }
    most
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
    counted: impl Fn(u32) -> bool,
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
            let (cand_p, held_p) =
                (pressure_after(&insns[cand], &live, &counted), pressure_after(&insns[held], &live, &counted));
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
        live.extend(insns[node].uses.iter().copied().filter(|&value| counted(value)));
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

/// The pre-RA machine scheduler: each region of a block that is longer than
/// half the allocatable registers is ordered by [`bottom_up`] (DEVIATE MS3:
/// values that pass through the block are not counted in its pressure; the
/// limit is the number of allocatable registers, not the pin's per-set limits).
pub struct MachineScheduler {
    pub classes: Rc<RegisterClasses>,
}

impl LIRTransform for MachineScheduler {
    fn class_name(&self) -> &'static str {
        "MachineScheduler"
    }

    fn name(&self) -> &str {
        "machinesched"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(scheduled(&body, self.classes.available.len()))
    }
}

/// `body` with each long region reordered; `limit` is the pressure it may
/// reach.
pub fn scheduled(
    body: &LirBody,
    limit: usize,
) -> LirBody {
    let (regs, bits) = (body.regs(), body.bits);
    // The x87 stack is its own register set with its own limit (DEVIATE MS3):
    // its values (those of the float operations) are not counted against
    // the general registers.
    let stack: HashSet<u32> = body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|one| {
            one.what
                .as_ref()
                .is_some_and(
                    |what| matches!(
                        what.op,
                        Operation::FloatLoad
                            | Operation::FloatStore
                            | Operation::FloatArith
                            | Operation::FloatArithPop
                            | Operation::FloatUnary
                            | Operation::FloatMultiplyAdd
                    ),
                )
        })
        .flat_map(|one| one.defines.iter().chain(&one.uses))
        .copied()
        .collect();
    let counted = |value: u32| !stack.contains(&value);
    let mut live = None;
    let mut changed = false;
    let blocks: Vec<_> = body
        .blocks
        .iter()
        .map(|block| {
            let insns = block.insns.to_vec();
            let regions = regions(&insns, regs, bits);
            if regions.iter().all(|region| region.len() <= limit / 2) {
                return block.clone();
            }
            let live = live.get_or_insert_with(|| live_rows(body));
            let mut values: Vec<u32> =
                insns.iter().flat_map(|one| one.defines.iter().chain(&one.uses)).copied().collect();
            values.sort_unstable();
            values.dedup();
            let mut now: HashSet<u32> =
                values.into_iter().filter(|&value| counted(value) && live.live_out(block.at, value)).collect();
            let step = |now: &mut HashSet<u32>, one: &Insn| {
                for value in &one.defines {
                    now.remove(value);
                }
                now.extend(one.uses.iter().copied().filter(|&value| counted(value)));
            };
            let mut out = insns.clone();
            let mut end = insns.len();
            for region in regions.iter().rev() {
                for one in insns[region.end..end].iter().rev() {
                    step(&mut now, one);
                }
                let slice = &insns[region.clone()];
                if slice.len() > limit / 2 {
                    let order = bottom_up(slice, &graph(slice, regs, bits), &now, limit, &counted);
                    // DEVIATE MS3: an order that does not lower the peak
                    // pressure is not taken (the pin
                    // keeps source order on ties and tracks per-set limits that
                    // are not modelled here).
                    let source: Vec<usize> = (0..slice.len()).collect();
                    if order != source && peak(slice, &order, &now, &counted) < peak(slice, &source, &now, &counted) {
                        let mut ats: Vec<i64> = slice.iter().map(|one| one.at).collect();
                        ats.sort_unstable();
                        for (place, &node) in order.iter().enumerate() {
                            let mut moved = (*slice[node]).clone();
                            moved.at = ats[place];
                            out[region.start + place] = Arc::new(moved);
                        }
                        changed = true;
                    }
                }
                for one in slice.iter().rev() {
                    step(&mut now, one);
                }
                end = region.start;
            }
            block.with_insns(out)
        })
        .collect();
    if changed { body.with_blocks(blocks) } else { body.clone() }
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
        assert_eq!(bottom_up(&insns, &found, &live_out, 2, |_| true), [0, 1, 3, 2, 4, 5]);
        // A limit that is never reached leaves the order alone.
        assert_eq!(bottom_up(&insns, &found, &live_out, 16, |_| true), [0, 1, 2, 3, 4, 5]);
    }

    /// The long region of a block was kept in source order whatever the
    /// pressure: three loads were all live at once (3 values) with a limit
    /// of 2. Through the pass the third load sits just above its reader.
    #[test]
    fn test_the_pass_orders_a_long_region_by_pressure() {
        use super::scheduled;
        use crate::model::lir::{LirBlock, LirBody};
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let mut insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![cell(0)]), vec![1], vec![]),
            insn(1, semantics(Operation::Move, "mov", vec![held(2)], vec![cell(8)]), vec![2], vec![]),
            insn(2, semantics(Operation::Move, "mov", vec![held(3)], vec![cell(16)]), vec![3], vec![]),
            insn(3, semantics(Operation::Binary, "add", vec![held(4)], vec![held(1), held(2)]), vec![4], vec![1, 2]),
            insn(4, semantics(Operation::Binary, "add", vec![held(5)], vec![held(4), held(3)]), vec![5], vec![4, 3]),
            insn(5, semantics(Operation::Binary, "add", vec![held(6)], vec![held(5), imm(1)]), vec![6], vec![5]),
        ];
        insns.push(insn(6, semantics(Operation::Return, "ret", vec![], vec![held(6)]), vec![], vec![6]));
        let mut body = LirBody::new(
            "pressure",
            0,
            vec![LirBlock::new(0, insns).into()],
            crate::support::hash::IndexMap::default(),
            crate::support::hash::IndexMap::default(),
        );
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let defined: Vec<u32> =
            scheduled(&body, 2).blocks[0].insns.iter().flat_map(|one| one.defines.clone()).collect();
        assert_eq!(defined, [1, 2, 4, 3, 5, 6]);
        let kept: Vec<u32> = scheduled(&body, 16).blocks[0].insns.iter().flat_map(|one| one.defines.clone()).collect();
        assert_eq!(kept, [1, 2, 3, 4, 5, 6]);
    }

    /// A zero made before `add` was scheduled between `add` and `adc` (i64
    /// divided by a power of two): the zero cannot be an `xor` there, and
    /// `mov ecx, 0` replaced a two-byte `xor`. The pin's `MOV32r0` writes
    /// the flags; so does ours here.
    #[test]
    fn test_a_zero_stays_out_of_a_flag_writer_and_its_reader() {
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![imm(0)]), vec![1], vec![]),
            insn(1, semantics(Operation::Binary, "add", vec![held(3)], vec![held(2), held(2)]), vec![3], vec![2]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        assert_eq!(graph(&insns, regs, 16).preds[1], [0], "the zero stays before the add");
    }

    /// A `seta` was scheduled above the `fcom` whose flags it reads
    /// (`a*100 + (x > 2.5 ? 30 : 20) + b` added 20 for x > 2.5): the x87
    /// compare writes the flags, as the `sahf` the stackifier adds does.
    #[test]
    fn test_a_set_stays_after_the_float_compare_it_reads() {
        let float = |value: u32| Loc::Held(Held { value, width: 10 });
        let byte = Loc::Held(Held { value: 3, width: 1 });
        let insns = vec![
            insn(0, semantics(Operation::Compare, "fcom", vec![], vec![float(1), float(2)]), vec![], vec![1, 2]),
            insn(1, semantics(Operation::Unary, "seta", vec![byte], vec![]), vec![3], vec![]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m32::REGISTER_INFO);
        assert_eq!(graph(&insns, regs, 32).preds[1], [0], "the set stays after the fcom");
    }
}
