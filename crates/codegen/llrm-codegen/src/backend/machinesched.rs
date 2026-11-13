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
//! `MachineScheduler`, a machine phase after `phielim` and before
//! `floatfold` (`machinesched@5`): each region longer than half the allocatable
//! registers is ordered by `bottom_up` (pieces 2 and 3 of
//! docs/working/machine-scheduler-plan.md). DEVIATE MS3: values that pass
//! through a block are not counted in its pressure, and one limit stands for
//! every register set.

use std::collections::BTreeSet;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::{RegId, Regs};

use crate::backend::classes::RegisterClasses;
use crate::backend::genericdriver::{RegionPolicy, Scheduler, ZonePressure};
use crate::backend::genericsched::Reason;
use crate::backend::liveness;
use crate::backend::liverows::{LiveAt, live_rows};
use crate::backend::regclass;
use crate::backend::regpressure::{Delta, Model, PressureChange, Region, TopTracker, Tracker};
use crate::backend::schedboundary::Unit;
use crate::backend::schedmodel::{MemBase, MemOp, SchedulingModel};
use crate::backend::target::Segments;
use crate::backend::twoaddr;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet, IndexMap};

/// `MI.modifiesRegister(StackPointerRegisterToSaveRestore)` of
/// `TargetInstrInfo::isSchedulingBoundary`: anything that writes the stack
/// pointer, whatever the operation (`add sp, n`, `sub esp, n`, `enter`; `push`,
/// `pop`, `leave`, `call` and `ret` write it implicitly and are listed by
/// operation).
fn modifies_stack_pointer(
    regs: Regs,
    one: &Insn,
) -> bool {
    // `_lanes` leaves the stack pointer out of the lanes it orders by, which is
    // why a write of it ordered nothing; the operands are asked instead.
    one.what
        .as_ref()
        .is_some_and(
            |what| what.dests.iter().any(|place| matches!(place, Loc::Reg(reg) if regs.is_stack(reg.register))),
        )
}

/// Whether `one` stays where it is, and nothing moves across it.
///
/// The pin's conditions (`MachineScheduler.cpp` `isSchedBoundary`,
/// `TargetInstrInfo::isSchedulingBoundary`,
/// `X86InstrInfo::isSchedulingBoundary`) against ours:
///
/// ```text
/// pin                                   ours
/// isCall                                Call, a `call` field
/// isTerminator                          Branch, Jump, Return
/// isPosition (labels, CFI)              an instruction with no semantics
/// INLINEASM_BR                          Escape
/// modifies the stack pointer            Push, Pop, Leave, and any write of SP's lanes
/// X86: FrameSetup / FrameDestroy        `frame_adjust`
/// X86: ENDBR32/64, PLDTILECFGV          none on this target
/// (no pin counterpart)                  fldcw, fnstcw, fnstsw and the other FPU word instructions
/// ```
///
/// Ours also ends a region at what the pin orders by dependences (fixed
/// registers, clobbers, groups, volatile accesses, barriers, spill markers,
/// unknown effects, x87 boundaries): DEVIATE MS4, a coarser graph that costs
/// scheduling room, never correctness.
fn boundary(
    regs: Regs,
    bits: u32,
    one: &Insn,
) -> bool {
    let Some(what) = &one.what else { return true };
    matches!(
        what.op,
        Operation::Call
            | Operation::Push
            | Operation::Pop
            | Operation::Leave
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
        || modifies_stack_pointer(regs, one)
        || fpu_word(one)
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

/// CF, PF, AF, ZF, SF, OF, in the lane numbering of `Lanes::flags`
/// (`llrm_lir::flag`, the decoder's bits: the mask 0x8d5 of rflags positions
/// named other lanes).
const ARITHMETIC_FLAGS: u32 = llrm_lir::flag::ALL.bits();

/// What an instruction does that its decoded effect leaves out, because a later
/// pass adds it: an x87 compare writes the arithmetic flags (the stackifier
/// adds `fnstsw; sahf` after it, and `seta` was hoisted above the `fcom`:
/// wc_mixed after_double), and `MOV32r0` becomes `xor`.
fn writes_flags_later(one: &Insn) -> bool {
    zero_move(one) || x87_compare(one)
}

/// `fcom`, `fcomp`, `fcompp`, `fucom*`, `ficom*`, `ftst`, `fxam`: they set the
/// FPU status word, whose condition bits `fnstsw; sahf` copy into the flags.
fn x87_compare(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| what.op == Operation::Compare
                && what.name.as_deref().is_some_and(|name| {
                    name.starts_with("fcom")
                        || name.starts_with("fucom")
                        || name.starts_with("ficom")
                        || matches!(name, "ftst" | "fxam")
                }),
        )
}

/// The FPU control and status words, which no lane orders: `fist` rounds by the
/// control word that `fldcw` set, and `fnstsw` reads the status word a compare
/// wrote.
fn fpu_word(one: &Insn) -> bool {
    one.what
        .as_ref()
        .and_then(|what| what.name.as_deref())
        .is_some_and(
            |name| matches!(
                name,
                "fldcw"
                    | "fnstcw"
                    | "fstcw"
                    | "fnstsw"
                    | "fstsw"
                    | "fnclex"
                    | "fclex"
                    | "fninit"
                    | "finit"
                    | "fldenv"
                    | "fnstenv"
                    | "fwait"
                    | "wait"
            ),
        )
}

/// A move of the constant zero into a value.
pub(crate) fn zero_move(one: &Insn) -> bool {
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

/// The edges of a region, by position in it.
#[derive(Debug, Eq, PartialEq)]
pub struct Graph {
    /// For each instruction, the earlier ones that must stay before it.
    pub preds: Vec<Vec<usize>>,
    /// For each instruction, the later ones that must stay after it.
    pub succs: Vec<Vec<usize>>,
    /// The latency of each edge in `preds`, in the same order: what the
    /// earlier instruction takes to produce what the later one reads
    /// ([`SchedulingModel::def_latency`]), one for an output dependence, none
    /// for an anti dependence or a memory order.
    pub latencies: Vec<Vec<u32>>,
    /// For each instruction, the ones weakly ordered before it
    /// ([`copy_constrain`]): they never hold a node back, `Weak` of
    /// `tryCandidate` only prefers the one with fewer left.
    pub weak_preds: Vec<Vec<usize>>,
    pub weak_succs: Vec<Vec<usize>>,
    /// For each instruction, the cluster of memory accesses it belongs to
    /// (`SUnit::ParentClusterIdx`, [`cluster_mem_ops`]).
    pub parent_cluster: Vec<Option<u32>>,
    /// For each instruction, the latency of its artificial edge to the region
    /// end (`ExitSU`): a def nothing in the region reads, held for its
    /// latency less one. Zero for the others.
    pub exit_latency: Vec<u32>,
    topo: Topo,
}

/// `ScheduleDAGTopologicalSort`: a topological order of the instructions,
/// kept as weak edges are added (Pearce and Kelly), so a reachability query
/// only walks the nodes between the two ends. It is a cache of the edges and
/// takes no part in comparing graphs.
#[derive(Debug, Default)]
struct Topo {
    /// Where each instruction stands in the order.
    index: Vec<usize>,
    /// The instruction at each place.
    at: Vec<usize>,
    /// Walk marks: `marks[node] == epoch` is seen in this walk.
    marks: std::cell::RefCell<(Vec<u32>, u32)>,
}

impl Topo {
    /// The program order, which every edge of a new graph follows.
    fn new(n: usize) -> Topo {
        Topo { index: (0..n).collect(), at: (0..n).collect(), marks: std::cell::RefCell::new((vec![0; n], 0)) }
    }
}

impl PartialEq for Topo {
    fn eq(
        &self,
        _: &Topo,
    ) -> bool {
        true
    }
}

impl Eq for Topo {}

impl Graph {
    /// A graph of `n` instructions with the given `(earlier, later, latency)`
    /// edges, for a scheduler test.
    #[cfg(test)]
    pub(crate) fn from_edges(
        n: usize,
        edges: &[(usize, usize, u32)],
    ) -> Graph {
        let mut graph = Graph {
            preds: vec![Vec::new(); n],
            succs: vec![Vec::new(); n],
            latencies: vec![Vec::new(); n],
            weak_preds: vec![Vec::new(); n],
            weak_succs: vec![Vec::new(); n],
            parent_cluster: vec![None; n],
            exit_latency: vec![0; n],
            topo: Topo::new(n),
        };
        for &(from, to, latency) in edges {
            graph.preds[to].push(from);
            graph.latencies[to].push(latency);
            graph.succs[from].push(to);
        }
        graph
    }

    /// Whether `to` follows `from` through the edges, weak ones too
    /// (`ScheduleDAGTopologicalSort::IsReachable`).
    fn reaches(
        &self,
        from: usize,
        to: usize,
    ) -> bool {
        if from == to {
            return true;
        }
        let limit = self.topo.index[to];
        if self.topo.index[from] > limit {
            return false;
        }
        let mut marks = self.topo.marks.borrow_mut();
        marks.1 += 1;
        let epoch = marks.1;
        let mut stack = vec![from];
        marks.0[from] = epoch;
        while let Some(node) = stack.pop() {
            for &next in self.succs[node].iter().chain(&self.weak_succs[node]) {
                if next == to {
                    return true;
                }
                if self.topo.index[next] < limit && marks.0[next] != epoch {
                    marks.0[next] = epoch;
                    stack.push(next);
                }
            }
        }
        false
    }

    /// `ScheduleDAGTopologicalSort::AddPred`'s reordering for the edge
    /// `before -> after`: the nodes between them that follow `after` move
    /// behind the ones that precede `before`.
    fn reorder(
        &mut self,
        before: usize,
        after: usize,
    ) {
        let (low, high) = (self.topo.index[after], self.topo.index[before]);
        if low > high {
            return;
        }
        let (mut forward, mut stack) = (vec![after], vec![after]);
        let mut seen = vec![after];
        while let Some(node) = stack.pop() {
            for &next in self.succs[node].iter().chain(&self.weak_succs[node]) {
                if self.topo.index[next] <= high && !seen.contains(&next) {
                    seen.push(next);
                    forward.push(next);
                    stack.push(next);
                }
            }
        }
        let (mut backward, mut stack) = (vec![before], vec![before]);
        let mut seen = vec![before];
        while let Some(node) = stack.pop() {
            for &prev in self.preds[node].iter().chain(&self.weak_preds[node]) {
                if self.topo.index[prev] >= low && !seen.contains(&prev) {
                    seen.push(prev);
                    backward.push(prev);
                    stack.push(prev);
                }
            }
        }
        forward.sort_unstable_by_key(|&node| self.topo.index[node]);
        backward.sort_unstable_by_key(|&node| self.topo.index[node]);
        let mut places: Vec<usize> = forward.iter().chain(&backward).map(|&node| self.topo.index[node]).collect();
        places.sort_unstable();
        for (&node, place) in backward.iter().chain(&forward).zip(places) {
            self.topo.index[node] = place;
            self.topo.at[place] = node;
        }
    }

    fn add_weak(
        &mut self,
        before: usize,
        after: usize,
    ) {
        if !self.weak_preds[after].contains(&before) {
            self.weak_preds[after].push(before);
            self.weak_succs[before].push(after);
            self.reorder(before, after);
        }
    }

    /// An `Artificial` edge `before -> after` (`ScheduleDAGMI::addEdge`): it
    /// holds `after` back like a data edge, with no latency. False, and no
    /// edge, where `before` already follows `after`.
    fn add_artificial(
        &mut self,
        before: usize,
        after: usize,
    ) -> bool {
        if self.reaches(after, before) {
            return false;
        }
        if let Some(at) = self.preds[after].iter().position(|&earlier| earlier == before)
            && self.latencies[after][at] == 0
        {
            return true;
        }
        self.preds[after].push(before);
        self.latencies[after].push(0);
        self.succs[before].push(after);
        self.reorder(before, after);
        true
    }
}

/// The latency of what `one` defines when the model states none:
/// `TargetInstrInfo::defaultDefLatency` is zero for a transient instruction
/// (a copy), the model's `LoadLatency` for a load, one for the rest.
pub fn default_def_latency(
    one: &Insn,
    load_latency: u32,
) -> u32 {
    let Some(what) = &one.what else { return 1 };
    let copy = what.op == Operation::Move
        && matches!(
            (what.dests.as_slice(), what.sources.as_slice()),
            ([Loc::Held(_)], [Loc::Held(_)])
        );
    if copy || (what.op == Operation::Nothing && what.name.as_deref().is_none_or(str::is_empty)) {
        0
    } else if memory(one).0 {
        load_latency
    } else {
        1
    }
}

/// The set bits of `mask`, as indexes.
fn bits_of(mut mask: u128) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        if mask == 0 {
            return None;
        }
        let bit = mask.trailing_zeros() as usize;
        mask &= mask - 1;
        Some(bit)
    })
}

/// What `graph` knows of one register lane: the defs standing (a live def
/// replaces them, a dead one joins them), the one live def among them, and
/// the readers since.
#[derive(Clone, Default)]
struct Lane {
    defs: Vec<usize>,
    live_def: Option<usize>,
    readers: Vec<usize>,
}

/// The graph of `insns`, one region (every instruction schedulable).
pub fn graph(
    insns: &[Arc<Insn>],
    regs: Regs,
    bits: u32,
    model: &dyn SchedulingModel,
    exit: Option<&Insn>,
) -> Graph {
    let n = insns.len();
    let mut effects: Vec<_> = insns.iter().map(|one| liveness::effect(regs, bits, one)).collect();
    let decoded = effects.clone();
    // `MOV32r0` writes EFLAGS (it becomes `xor`): a zero made between a flag
    // writer and its reader would break `add; adc` (the pin's scheduler
    // sees the def).
    for (one, effect) in insns.iter().zip(&mut effects) {
        if let Some(effect) = effect {
            if writes_flags_later(one) {
                effect.writes = effect.writes.or(&crate::backend::peephole::Lanes::flags(ARITHMETIC_FLAGS));
            }
        }
    }
    // ScheduleDAGInstrs' way: each instruction depends on the last definition
    // of what it reads (data), of what it writes (output), on what read it
    // since (anti), and memory and the unknown on their last writer and
    // readers; the rest follows by chains. One pass, tables by value and lane.
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut latencies: Vec<Vec<u32>> = vec![Vec::new(); n];
    // Whether a later instruction depends on it through a register (data,
    // output or anti): `NumSuccs` where the pin looks, before the chains.
    let mut register_succ = vec![false; n];
    let mut by_value: HashMap<u32, (Option<usize>, Vec<usize>)> = HashMap::default();
    // Per lane: the defs still standing (a live def replaces them, a dead one
    // joins them, as the pin's `Defs`) and the readers since.
    let mut by_lane: Vec<Lane> = vec![Lane::default(); 128];
    // The lanes each instruction writes that nothing reads before the next
    // write (`MO.isDead()`); what is live below the region is not known, so the
    // last write of a lane is live.
    let mut dead = vec![0u128; n];
    let mut live = u128::MAX;
    for later in (0..n).rev() {
        match &effects[later] {
            Some(effect) => {
                let writes = effect.writes.mask();
                dead[later] = writes & !live;
                // A write a later pass adds (a zero's `xor`) does not end the
                // life of the flags it sits between: `add; mov 0; adc` keeps
                // the add's carry for the `adc`.
                let kept = decoded[later].as_ref().map_or(writes, |decoded| decoded.writes.mask());
                live = (live & !kept) | effect.all_reads().mask();
            }
            None => live = u128::MAX,
        }
    }
    // The last store and the loads since it; the last instruction of unknown
    // effect and everything since it.
    let (mut store, mut loads): (Option<usize>, Vec<usize>) = (None, Vec::new());
    let (mut unknown, mut since): (Option<usize>, Vec<usize>) = (None, Vec::new());
    // Where an instruction states what its access reaches (`Insn::memory`), the
    // memory lane no longer orders it: each earlier store and load is kept and
    // an edge made where `Insn::may_alias` says they may overlap
    // (`addChainDependencies`). A body with none keeps the one chain above.
    let aliasing = insns.iter().any(|one| one.memory.is_some());
    let memory_lane = crate::backend::peephole::Lanes::flags(crate::backend::peephole::Lanes::MEMORY).mask();
    let (mut stores_before, mut loads_before): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
    for later in 0..n {
        let mut edges: Vec<(usize, u32)> = Vec::new();
        let one = &insns[later];
        let (reads_memory, writes_memory) = memory(one);
        let mut masks = effects[later].as_ref().map(|effect| (effect.all_reads().mask(), effect.writes.mask()));
        let (mut reads_memory, mut writes_memory) = (reads_memory, writes_memory);
        if aliasing {
            if let Some((reads, writes)) = &mut masks {
                reads_memory |= *reads & memory_lane != 0;
                writes_memory |= *writes & memory_lane != 0;
                *reads &= !memory_lane;
                *writes &= !memory_lane;
            }
        }
        // What every instruction after one of unknown effect keeps behind it.
        edges.extend(unknown.map(|earlier| (earlier, 0)));
        let register_from = edges.len();
        for value in &one.uses {
            if let Some((Some(def), _)) = by_value.get(value) {
                edges.push((*def, model.def_latency(&insns[*def])));
            }
        }
        for value in &one.defines {
            if let Some((def, readers)) = by_value.get(value) {
                edges.extend(def.map(|earlier| (earlier, 1)));
                edges.extend(readers.iter().map(|&earlier| (earlier, 0)));
            }
        }
        let value_end = edges.len();
        match masks {
            Some((reads, writes)) => {
                for bit in bits_of(reads) {
                    if let Some(&def) = by_lane[bit].defs.last() {
                        edges.push((def, model.def_latency(&insns[def])));
                    }
                }
                for bit in bits_of(writes) {
                    // No output edge between two dead defs (`addPhysRegDeps`).
                    let this_dead = dead[later] >> bit & 1 == 1;
                    // A dead write is ordered after the live def only, the dead
                    // defs standing before it being the ones it has no edge to.
                    if this_dead {
                        edges.extend(by_lane[bit].live_def.map(|earlier| (earlier, 1)));
                    } else {
                        edges.extend(by_lane[bit].defs.iter().map(|&earlier| (earlier, 1)));
                    }
                    edges.extend(by_lane[bit].readers.iter().map(|&earlier| (earlier, 0)));
                }
            }
            // An unknown effect is ordered with everything before it.
            None => edges.extend(since.iter().map(|&earlier| (earlier, 0))),
        }
        // The register edges are in: the unknown-effect chain and the memory
        // ones come next, and the pin counts neither.
        let register_to = if masks.is_some() { edges.len() } else { value_end };
        for &(earlier, _) in &edges[register_from..register_to] {
            register_succ[earlier] = true;
        }
        if aliasing {
            let apart = |earlier: usize| {
                !insns[earlier].may_alias(one) && one.memory.is_some() && insns[earlier].memory.is_some()
            };
            if reads_memory || writes_memory {
                edges.extend(stores_before.iter().filter(|&&earlier| !apart(earlier)).map(|&earlier| (earlier, 0)));
            }
            if writes_memory {
                edges.extend(loads_before.iter().filter(|&&earlier| !apart(earlier)).map(|&earlier| (earlier, 0)));
            }
        } else {
            if reads_memory || writes_memory {
                edges.extend(store.map(|earlier| (earlier, 0)));
            }
            if writes_memory {
                edges.extend(loads.iter().map(|&earlier| (earlier, 0)));
            }
        }
        // One edge to each earlier instruction, the longest latency.
        edges.sort_unstable();
        edges.dedup_by(|next, kept| {
            let same = next.0 == kept.0;
            if same {
                kept.1 = kept.1.max(next.1);
            }
            same
        });
        for (earlier, latency) in edges {
            preds[later].push(earlier);
            latencies[later].push(latency);
        }
        // Record what this one reads and writes.
        for value in &one.uses {
            by_value.entry(*value).or_default().1.push(later);
        }
        for value in &one.defines {
            let entry = by_value.entry(*value).or_default();
            *entry = (Some(later), Vec::new());
        }
        if let Some((reads, writes)) = masks {
            for bit in bits_of(reads) {
                by_lane[bit].readers.push(later);
            }
            for bit in bits_of(writes) {
                // A dead write leaves the defs and readers standing: a later
                // write still comes after the readers of the
                // live def before it.
                let entry = &mut by_lane[bit];
                if dead[later] >> bit & 1 == 0 {
                    entry.defs.clear();
                    entry.readers.clear();
                    entry.live_def = Some(later);
                }
                entry.defs.push(later);
            }
        } else {
            unknown = Some(later);
            since.clear();
            // Everything before it is behind it now.
            by_value.clear();
            by_lane.iter_mut().for_each(|entry| *entry = Lane::default());
            store = None;
            loads.clear();
        }
        if aliasing {
            if writes_memory {
                if one.memory.is_some() {
                    stores_before.push(later);
                } else {
                    // An access of unknown reach is ordered with everything
                    // before it: the earlier ones are behind it now.
                    (stores_before, loads_before) = (vec![later], Vec::new());
                }
            } else if reads_memory {
                loads_before.push(later);
            }
        } else if writes_memory {
            (store, loads) = (Some(later), Vec::new());
        } else if reads_memory {
            loads.push(later);
        }
        since.push(later);
    }
    // `ExitSU` is the instruction that ends the region: what it reads is held
    // for the latency of its def (`addVRegUseDeps`, `addPhysRegDeps`).
    let mut exit_latency = vec![0u32; n];
    if let (true, Some(exit)) = (model.exit_edges(), exit) {
        let mut read = |def: usize| {
            register_succ[def] = true;
            exit_latency[def] = exit_latency[def].max(model.def_latency(&insns[def]));
        };
        for value in &exit.uses {
            if let Some((Some(def), _)) = by_value.get(value) {
                read(*def);
            }
        }
        if let Some(effect) = liveness::effect(regs, bits, exit) {
            for bit in bits_of(effect.all_reads().mask() & !memory_lane) {
                if let Some(&def) = by_lane[bit].defs.last() {
                    read(def);
                }
            }
        }
    }
    // `buildSchedGraph`: a def with no reader in the region, slower than one
    // cycle, holds `latency - 1` to the region end.
    for node in 0..n {
        let one = &insns[node];
        let latency = model.def_latency(one);
        if model.exit_edges() && !register_succ[node] && latency > 1 && (!one.defines.is_empty() || memory(one).0) {
            exit_latency[node] = latency - 1;
        }
    }
    let mut succs: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (later, before) in preds.iter().enumerate() {
        for &earlier in before {
            succs[earlier].push(later);
        }
    }
    Graph {
        preds,
        succs,
        latencies,
        weak_preds: vec![Vec::new(); n],
        weak_succs: vec![Vec::new(); n],
        parent_cluster: vec![None; n],
        exit_latency,
        topo: Topo::new(n),
    }
}

/// One live range segment of a value in a region, in slots: a position `p`
/// reads at `2p` and a dead def ends at `2p + 1`.
struct Segment {
    start: i64,
    end: i64,
}

const LIVE_IN_SLOT: i64 = -1;
const LIVE_OUT_SLOT: i64 = i64::MAX;

/// The positions in a region that read and that define each value, in order
/// (the pin's use-def lists): one pass over the region, so a query for a
/// value does not scan it.
#[derive(Default)]
struct Occurrences {
    reads: HashMap<u32, Vec<usize>>,
    defs: HashMap<u32, Vec<usize>>,
}

impl Occurrences {
    fn of(insns: &[Arc<Insn>]) -> Occurrences {
        let mut found = Occurrences::default();
        for (at, one) in insns.iter().enumerate() {
            for (list, values) in [(&mut found.reads, &one.uses), (&mut found.defs, &one.defines)] {
                for &value in values.iter() {
                    let positions = list.entry(value).or_default();
                    if positions.last() != Some(&at) {
                        positions.push(at);
                    }
                }
            }
        }
        found
    }

    fn reads(
        &self,
        value: u32,
    ) -> &[usize] {
        self.reads.get(&value).map_or(&[], Vec::as_slice)
    }

    fn defs(
        &self,
        value: u32,
    ) -> &[usize] {
        self.defs.get(&value).map_or(&[], Vec::as_slice)
    }
}

/// The segments of `value` in `insns` and the positions of its defs. A def
/// that reads the value (two-address) ends one segment and starts the next at
/// the same instruction, as the pin's live intervals do.
fn segments_of(
    occurrences: &Occurrences,
    value: u32,
    live_out: bool,
) -> (Vec<Segment>, Vec<usize>) {
    let mut segments: Vec<Segment> = Vec::new();
    let mut defs = Vec::new();
    let (reads_at, defs_at) = (occurrences.reads(value), occurrences.defs(value));
    let (mut r, mut d) = (0, 0);
    while r < reads_at.len() || d < defs_at.len() {
        let at = match (reads_at.get(r), defs_at.get(d)) {
            (Some(&read), Some(&def)) => read.min(def),
            (Some(&read), None) => read,
            (None, Some(&def)) => def,
            (None, None) => unreachable!("one list has a position left"),
        };
        let (reads, defines) = (reads_at.get(r) == Some(&at), defs_at.get(d) == Some(&at));
        r += usize::from(reads);
        d += usize::from(defines);
        let slot = 2 * at as i64;
        if reads {
            match segments.last_mut() {
                Some(last) => last.end = slot,
                None => segments.push(Segment { start: LIVE_IN_SLOT, end: slot }),
            }
        }
        if defines {
            defs.push(at);
            // A two-address def starts a segment where the one it reads ends.
            segments.push(Segment { start: slot, end: slot + 1 });
        }
    }
    if live_out && let Some(last) = segments.last_mut() {
        last.end = LIVE_OUT_SLOT;
    }
    (segments, defs)
}

fn copy_of(one: &Insn) -> Option<(u32, u32)> {
    let what = one.what.as_ref()?;
    match (what.op, what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::Move, [Loc::Held(dst)], [Loc::Held(src)]) if dst.value != src.value => Some((dst.value, src.value)),
        _ => None,
    }
}

/// `CopyConstrain::apply` (MachineScheduler.cpp#L2298): for a copy with a
/// local end and a global one, weak edges that open a hole in the global
/// register's range, so the copy's local uses stay beside the copy and it
/// can coalesce. `live_out` tells what lives below the region.
pub fn copy_constrain(
    insns: &[Arc<Insn>],
    graph: &mut Graph,
    live_out: &dyn Fn(u32) -> bool,
) {
    let occurrences = Occurrences::of(insns);
    let mut ranges: HashMap<u32, (Vec<Segment>, Vec<usize>)> = HashMap::default();
    for (copy, one) in insns.iter().enumerate() {
        let Some((dst, src)) = copy_of(one) else { continue };
        for value in [dst, src] {
            ranges.entry(value).or_insert_with(|| segments_of(&occurrences, value, live_out(value)));
        }
        constrain_local_copy(&occurrences, graph, &ranges, copy, dst, src);
    }
}

/// `constrainLocalCopy`.
fn constrain_local_copy(
    occurrences: &Occurrences,
    graph: &mut Graph,
    ranges: &HashMap<u32, (Vec<Segment>, Vec<usize>)>,
    copy: usize,
    dst: u32,
    src: u32,
) {
    let local = |value: u32| {
        let (segments, _) = &ranges[&value];
        segments.first().is_some_and(|first| first.start != LIVE_IN_SLOT)
            && segments.last().is_some_and(|last| last.end != LIVE_OUT_SLOT)
    };
    // A dead destination has nothing to constrain.
    if ranges[&dst].0.iter().any(|segment| segment.start == 2 * copy as i64 && segment.end == segment.start + 1) {
        return;
    }
    let (local_value, global_value) = if local(src) {
        (src, dst)
    } else if local(dst) {
        (dst, src)
    } else {
        return;
    };
    let (local_segments, local_defs) = &ranges[&local_value];
    let (global_segments, global_defs) = &ranges[&global_value];
    let begin = local_segments[0].start;
    // The global segment after the start of the local range: the one that
    // ends after it, or the next if it holds it.
    let Some(mut at) = global_segments.iter().position(|segment| segment.end > begin) else { return };
    if global_segments[at].start <= begin {
        at += 1;
    }
    let Some(next) = global_segments.get(at) else { return };
    if at > 0 {
        let before = &global_segments[at - 1];
        // Two-address defs leave no hole, and the instruction that starts the
        // local range cannot also make the hole.
        if before.end.div_euclid(2) == next.start.div_euclid(2) || before.start.div_euclid(2) == begin.div_euclid(2) {
            return;
        }
    }
    if next.start == LIVE_IN_SLOT {
        return;
    }
    let global_def = (next.start / 2) as usize;
    let first_local = (begin / 2) as usize;
    // The uses of the last local def go before the global def.
    let last_local_def = *local_defs.last().expect("a local range has a def");
    let reads = occurrences.reads(local_value);
    let local_uses: Vec<usize> = reads[reads.partition_point(|&at| at <= last_local_def)..]
        .iter()
        .copied()
        .filter(|&at| at != global_def)
        .collect();
    if local_uses.iter().any(|&user| graph.reaches(global_def, user)) {
        return;
    }
    // And the earlier readers of the global before the start of the local.
    let after = global_defs.iter().copied().filter(|&at| at < global_def).max().map_or(0, |at| at + 1);
    let reads = occurrences.reads(global_value);
    let global_uses: Vec<usize> = reads
        [reads.partition_point(|&at| at < after)..reads.partition_point(|&at| at < global_def)]
        .iter()
        .copied()
        .filter(|&at| at != first_local)
        .collect();
    if global_uses.iter().any(|&user| graph.reaches(first_local, user)) {
        return;
    }
    if trace() {
        eprintln!(
            "machinesched constraining copy {copy} (local {local_value}, global {global_value}): {local_uses:?} -> {global_def}, {global_uses:?} -> {first_local}"
        );
    }
    for user in local_uses {
        graph.add_weak(user, global_def);
    }
    for user in global_uses {
        graph.add_weak(user, first_local);
    }
}

/// `-fast-cluster-threshold`: a region whose accesses times instructions pass
/// this per thousand is clustered by chain predecessor in the pin. DEVIATE
/// SB5b: such a region is not clustered at all.
const FAST_CLUSTER_THRESHOLD: usize = 1000;

/// `MemOpInfo::operator<`: by base operand (registers before frame indices; a
/// frame index of a stack that grows down, highest first), then offset, then
/// instruction number.
fn memop_order(
    a: &(usize, MemOp),
    b: &(usize, MemOp),
) -> std::cmp::Ordering {
    let base = match (a.1.base, b.1.base) {
        (MemBase::Frame(x), MemBase::Frame(y)) => y.cmp(&x),
        (x, y) => x.cmp(&y),
    };
    base.then(a.1.offset.cmp(&b.1.offset)).then(a.0.cmp(&b.0))
}

/// `BaseMemOpClusterMutation::apply` (MachineScheduler.cpp#L2264), run for the
/// loads and then the stores of the region: neighbouring accesses the model
/// would `should_cluster` get a weak edge, so the scheduler keeps them
/// together for the load/store optimizer, and the pin's artificial edges that
/// keep other work from separating them.
pub fn cluster_mem_ops(
    insns: &[Arc<Insn>],
    graph: &mut Graph,
    model: &dyn SchedulingModel,
) {
    let ops: Vec<Option<MemOp>> = insns.iter().map(|one| model.mem_op(one)).collect();
    let mut next_cluster = graph.parent_cluster.iter().flatten().max().map_or(0, |last| last + 1);
    for loads in [true, false] {
        let mut records: Vec<(usize, MemOp)> = ops
            .iter()
            .enumerate()
            .filter_map(|(node, op)| op.filter(|op| op.load == loads).map(|op| (node, op)))
            .collect();
        if records.len() < 2 || records.len() * insns.len() / 1000 > FAST_CLUSTER_THRESHOLD {
            continue;
        }
        records.sort_by(memop_order);
        // The length and bytes of the cluster each second member ends.
        let mut ending: HashMap<usize, (u32, u32)> = HashMap::default();
        let mut leader: Vec<usize> = (0..insns.len()).collect();
        let mut members: Vec<usize> = Vec::new();
        for at in 0..records.len() - 1 {
            let (a_node, a_op) = records[at];
            let Some(&(b_node, b_op)) = records[at + 1..].iter().find(|(node, _)| {
                !ending.contains_key(node) && !graph.reaches(*node, a_node) && !graph.reaches(a_node, *node)
            }) else {
                continue;
            };
            let (mut length, mut bytes) = (2, a_op.width + b_op.width);
            if let Some(&(len, so_far)) = ending.get(&a_node) {
                (length, bytes) = (len + 1, so_far + b_op.width);
            }
            if !model.should_cluster((&insns[a_node], &a_op), (&insns[b_node], &b_op), length, bytes) {
                continue;
            }
            let (first, second) = if a_node > b_node { (b_node, a_node) } else { (a_node, b_node) };
            // `addEdge` refuses where the first already follows the second.
            if graph.reaches(second, first) {
                continue;
            }
            graph.add_weak(first, second);
            let (x, y) = (find(&mut leader, first), find(&mut leader, second));
            leader[x] = y;
            members.extend([first, second]);
            if loads {
                // Work that reads the first access's value would otherwise
                // run between the two: it follows the second as well.
                let after: Vec<usize> = graph.succs[first].iter().chain(&graph.weak_succs[first]).copied().collect();
                for succ in after.into_iter().filter(|&succ| succ != second) {
                    graph.add_artificial(second, succ);
                }
            } else {
                // What the second store waits for comes before the first.
                let before: Vec<usize> = graph.preds[second].iter().chain(&graph.weak_preds[second]).copied().collect();
                for pred in before.into_iter().filter(|&pred| pred != first) {
                    graph.add_artificial(pred, first);
                }
            }
            ending.insert(b_node, (length, bytes));
        }
        members.sort_unstable();
        members.dedup();
        let mut ids: HashMap<usize, u32> = HashMap::default();
        for node in members {
            let top = find(&mut leader, node);
            let id = *ids
                .entry(top)
                .or_insert_with(
                    || {
                        next_cluster += 1;
                        next_cluster - 1
                    },
                );
            graph.parent_cluster[node] = Some(id);
        }
    }
}

/// The root of `node` in the union-find `leader`.
fn find(
    leader: &mut [usize],
    mut node: usize,
) -> usize {
    while leader[node] != node {
        leader[node] = leader[leader[node]];
        node = leader[node];
    }
    node
}

/// `LLRM_MS_TRACE`: each pick with the reason `tracePick` of the pin names, for
/// a line-by-line reading against `llc -debug-only=machine-scheduler`.
fn trace() -> bool {
    llrm_support::env_flag!("LLRM_MS_TRACE")
}

/// What the tracker needs to follow a region's pressure bottom-up.
pub struct Tracking<'a> {
    pub model: &'a Model,
    /// The values live below the region that count (`closeBottom`).
    pub live_out: Vec<u32>,
    /// `initLiveThru`: the live-out values no untied def in the region makes.
    pub live_thru: Vec<u32>,
}

/// The bottom zone's pressure, for the driver: the tracker, the region's
/// critical sets and the maximum of the program order (`RegionCriticalPSets`,
/// `RegionPressure`).
struct Followed<'a> {
    bottom: Tracker<'a>,
    /// Only in a region that schedules from the top too.
    top: Option<TopTracker<'a>>,
    critical: Vec<PressureChange>,
    region_max: Vec<u32>,
}

impl ZonePressure for Followed<'_> {
    fn delta(
        &self,
        node: usize,
        at_top: bool,
    ) -> Delta {
        match &self.top {
            Some(top) if at_top => top.delta(node, &self.critical, &self.region_max),
            _ => self.bottom.delta(node, &self.critical, &self.region_max),
        }
    }

    fn place(
        &mut self,
        node: usize,
        at_top: bool,
    ) {
        let most = match &mut self.top {
            Some(top) if at_top => {
                top.place(node);
                if trace() {
                    eprintln!("machinesched pressure top {node} cur={:?}", top.current());
                }
                top.max().to_vec()
            }
            _ => {
                self.bottom.place(node);
                if trace() {
                    eprintln!("machinesched pressure bot {node} cur={:?}", self.bottom.current());
                }
                self.bottom.max().to_vec()
            }
        };
        // `updateScheduledPressure`.
        for entry in &mut self.critical {
            let most = most[entry.set] as i32;
            if most > entry.inc {
                entry.inc = most;
            }
        }
    }
}

/// The order of a region scheduled bottom-up as `GenericScheduler` does for
/// the i486 (MachineScheduler.cpp, #L3965): `X86.td` gives it `GenericModel`
/// (issue width 4, a micro-op buffer of 32, no instruction schedules), so
/// `SchedBoundary::releaseNode` never defers a node (`IsBuffered`, and one
/// micro-op never overfills a cycle) and the latency of an edge does not
/// order anything: an instruction is available once every reader is placed,
/// and [`genericsched::try_candidate`] picks among those. With `tracking` (a
/// region of more than half the allocatable registers' worth of instructions,
/// `initPolicy` #L3688) pressure is followed per set.
///
/// Not translated (MS6): `Rem.IsAcyclicLatencyLimited` (SB4 of
/// [`genericdriver`]), which turns on the height and depth heuristic in a loop
/// whose in-flight micro-ops pass the buffer (32).
pub fn order_region(
    insns: &[Arc<Insn>],
    graph: &Graph,
    tracking: Option<&Tracking>,
    model: &dyn SchedulingModel,
) -> Vec<usize> {
    picks(insns, graph, tracking, model).0
}

/// [`order_region`] with each pick in the order made: the node, whether from
/// the top, and the reason.
pub(crate) fn picks(
    insns: &[Arc<Insn>],
    graph: &Graph,
    tracking: Option<&Tracking>,
    model: &dyn SchedulingModel,
) -> (Vec<usize>, Vec<(usize, bool, Reason)>) {
    let n = insns.len();
    let policy = model.policy();
    let region = tracking.map(|_| Region::of(insns));
    let mut followed = tracking
        .zip(region.as_ref())
        .map(
            |(tracking, region)| {
                let mut program = Tracker::over(tracking.model, region, tracking.live_out.iter().copied(), [], false);
                for node in (0..n).rev() {
                    program.place(node);
                }
                let region_max = program.max().to_vec();
                let critical = Tracker::critical_sets(tracking.model, &region_max);
                if trace() {
                    eprintln!(
                        "machinesched region n={n} limits={:?} max={region_max:?} critical={critical:?}",
                        tracking.model.limits
                    );
                }
                Followed {
                    bottom: Tracker::over(
                        tracking.model,
                        region,
                        tracking.live_out.iter().copied(),
                        tracking.live_thru.iter().copied(),
                        true,
                    ),
                    top: (!policy.only_bottom_up).then(|| {
                        TopTracker::over(
                            tracking.model,
                            region,
                            tracking.live_out.iter().copied(),
                            tracking.live_thru.iter().copied(),
                        )
                    }),
                    critical,
                    region_max,
                }
            },
        );
    let machine = model.machine();
    let units: Vec<Unit> = insns.iter().map(|one| model.unit(one)).collect();
    if trace() {
        for (node, one) in insns.iter().enumerate() {
            let name = one.what.as_ref().and_then(|what| what.name.as_deref()).unwrap_or("-");
            eprintln!(
                "machinesched node {node} {name} lat={} uops={} res={:?}",
                model.def_latency(one),
                units[node].micro_ops,
                units[node].writes.iter().map(|write| (write.idx, write.release_at_cycle)).collect::<Vec<_>>()
            );
        }
    }
    let pressure = followed.as_mut().map(|one| one as &mut dyn ZonePressure);
    let (order, reasons) = Scheduler::new(graph, &machine, units, policy, pressure).schedule();
    if trace()
        && let (Some(tracking), Some(region)) = (tracking, region.as_ref())
    {
        let mut after = Tracker::over(tracking.model, region, tracking.live_out.iter().copied(), [], false);
        for &node in order.iter().rev() {
            after.place(node);
        }
        eprintln!("machinesched region n={n} scheduled max={:?}", after.max());
    }
    if trace() {
        for (node, at_top, reason) in &reasons {
            eprintln!(
                "machinesched pick {node} {:?} {} tracked={} {}",
                insns[*node].defines,
                reason.name(),
                tracking.is_some(),
                if *at_top { "top" } else { "bottom" }
            );
        }
    }
    (order, reasons)
}

/// The pre-RA machine scheduler: every region of a block ordered by
/// [`order_region`]. DEVIATE MS4: regions end at more instructions than the
/// pin's. DEVIATE RP1/RP2 are `regpressure.rs`'s.
pub struct MachineScheduler {
    pub classes: Rc<RegisterClasses>,
    pub segments: Segments,
    pub model: Rc<dyn SchedulingModel>,
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
        let registers = &self.classes;
        let segments = &self.segments;
        Ok(scheduled(&body, registers.available.len(), &*self.model, |body, counted| {
            pressure_model(body, registers, segments, counted)
        }))
    }
}

/// The pressure model of `body`: the class and sets each counted value is
/// confined to (`regclass::classes`), each set's limit from the target's
/// pressure sets. Shared by the pre-RA scheduler and EarlyMachineLICM.
pub(crate) fn pressure_model(
    body: &LirBody,
    registers: &RegisterClasses,
    segments: &Segments,
    counted: &dyn Fn(u32) -> bool,
) -> Model {
    let confined = regclass::classes(body, &BTreeSet::new(), segments, registers);
    let mut widths: IndexMap<u32, i64> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        if let Some(what) = &one.what {
            for place in what.dests.iter().chain(&what.sources) {
                if let Loc::Held(held) = place {
                    let held_width = i64::from(held.width);
                    let width = widths.entry(held.value).or_insert(held_width);
                    *width = (*width).max(held_width);
                }
            }
        }
    }
    let everywhere: Vec<RegId> = registers.available.iter().map(|&reg| body.regs().root(reg)).collect();
    // A value no register list confines takes `everywhere`: shared, not
    // copied.
    let allowed: Vec<(u32, i64, Option<Vec<RegId>>)> = widths
        .iter()
        .filter(|&(&value, _)| counted(value))
        .map(|(&value, &width)| {
            let roots = confined.get(&value).map(|set| set.iter().map(|&reg| body.regs().root(reg)).collect());
            (value, width, roots)
        })
        .collect();
    // DEVIATE MS5: a target with no pressure sets (m16) counts every
    // value once against one set of its allocatable registers.
    let Some((sets, limits)) = registers.pressure.as_ref() else {
        let mut model = Model::new(vec![registers.available.len() as u32]);
        let one: Rc<[usize]> = Rc::from(vec![0]);
        for (value, _, _) in &allowed {
            model.count(*value, 1, one.clone());
        }
        return model;
    };
    Model::of_values(
        sets,
        body.regs(),
        |set| limits[set],
        allowed.iter().map(|(value, width, roots)| (*value, *width, roots.as_deref().unwrap_or(&everywhere))),
    )
}

/// The values of the float operations: the x87 stack is its own register set,
/// and they are not counted against the general registers.
pub(crate) fn x87_values<'a>(insns: impl Iterator<Item = &'a Arc<Insn>>) -> HashSet<u32> {
    insns
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
        .collect()
}

/// The registers of the x87 stack the allocator hands out (`FP0`-`FP6`, the
/// pin's `RFP` class).
const X87_STACK_LIMIT: u32 = 7;

/// `body` with each region reordered; a region of more than `limit / 2`
/// instructions is tracked with the model `model` makes of the body (given
/// the test of whether a value counts).
pub fn scheduled(
    body: &LirBody,
    limit: usize,
    sched: &dyn SchedulingModel,
    model: impl FnOnce(&LirBody, &dyn Fn(u32) -> bool) -> Model,
) -> LirBody {
    let (regs, bits) = (body.regs(), body.bits);
    // The x87 stack is its own register set with its own limit: its values
    // (those of the float operations) are not counted against the general
    // registers.
    let stack = if sched.x87_stack() {
        x87_values(body.blocks.iter().flat_map(|block| block.insns.iter()))
    } else {
        HashSet::default()
    };
    let counted = |value: u32| !stack.contains(&value);
    let mut live = None;
    let mut model = Some(model);
    let mut made: Option<Model> = None;
    let mut changed = false;
    let blocks: Vec<_> = body
        .blocks
        .iter()
        .map(|block| {
            let insns = block.insns.to_vec();
            let regions = regions(&insns, regs, bits);
            // A region of fewer than two instructions has nothing to order.
            if regions.iter().all(|region| region.len() < 2) {
                return block.clone();
            }
            // What is live below a region is asked for a region of more than
            // half the allocatable registers only.
            let mut now: HashSet<u32> = HashSet::default();
            let mut now87: HashSet<u32> = HashSet::default();
            let copies = regions.iter().any(|region| insns[region.clone()].iter().any(|one| copy_of(one).is_some()));
            if copies || regions.iter().any(|region| region.len() > limit / 2) {
                let live = live.get_or_insert_with(|| live_rows(body));
                let mut values: Vec<u32> =
                    insns.iter().flat_map(|one| one.defines.iter().chain(&one.uses)).copied().collect();
                values.sort_unstable();
                values.dedup();
                now =
                    values.iter().copied().filter(|&value| counted(value) && live.live_out(block.at, value)).collect();
                // The x87 values live below the block count in the stack's set
                // (the pin: every live-out register is in the bottom pressure).
                now87 = values
                    .iter()
                    .copied()
                    .filter(|&value| stack.contains(&value) && live.live_out(block.at, value))
                    .collect();
                if regions.iter().any(|region| region.len() > limit / 2)
                    && let Some(make) = model.take()
                {
                    let mut model = make(body, &counted);
                    // The x87 stack is the pin's `RFP` register class: a set of
                    // its own with the seven registers
                    // `FP0`-`FP6`.
                    if sched.x87_stack() {
                        let set: Rc<[usize]> = Rc::from(vec![model.limits.len()]);
                        model.limits.push(X87_STACK_LIMIT);
                        for &value in &stack {
                            model.count(value, 1, set.clone());
                        }
                    }
                    made = Some(model);
                }
            }
            let step = |now: &mut HashSet<u32>, one: &Insn| {
                for value in &one.defines {
                    now.remove(value);
                }
                now.extend(one.uses.iter().copied().filter(|&value| counted(value)));
            };
            let step87 = |now: &mut HashSet<u32>, one: &Insn| {
                for value in &one.defines {
                    now.remove(value);
                }
                now.extend(one.uses.iter().copied().filter(|value| stack.contains(value)));
            };
            let mut out = insns.clone();
            let mut end = insns.len();
            for region in regions.iter().rev() {
                for one in insns[region.end..end].iter().rev() {
                    step(&mut now, one);
                    step87(&mut now87, one);
                }
                let slice = &insns[region.clone()];
                if slice.len() >= 2 {
                    let tracking = made.as_ref().filter(|_| slice.len() > limit / 2).map(|model| {
                        // `discoverLiveOut`: the pin finds a live-out value
                        // only where the region defines
                        // or reads it, so a value that
                        // crosses the region untouched is no part of its
                        // pressure.
                        let touched: HashSet<u32> =
                            slice.iter().flat_map(|one| one.defines.iter().chain(&one.uses)).copied().collect();
                        let mut live_out: Vec<u32> =
                            now.iter().chain(&now87).copied().filter(|value| touched.contains(value)).collect();
                        live_out.sort_unstable();
                        // `initLiveThru`: a live-out value counts unless a def
                        // in the region ties it (two-address).
                        let tied: HashSet<u32> = slice
                            .iter()
                            .filter(|one| one.what.as_ref().is_some_and(|what| twoaddr::ties_in(body.registers, what)))
                            .flat_map(|one| one.defines.iter().copied())
                            .collect();
                        let live_thru = live_out.iter().copied().filter(|value| !tied.contains(value)).collect();
                        Tracking { model, live_out, live_thru }
                    });
                    let mut deps = graph(slice, regs, bits, sched, insns.get(region.end).map(|one| &**one));
                    if trace() {
                        let held: Vec<_> = deps.exit_latency.iter().enumerate().filter(|(_, l)| **l > 0).collect();
                        eprintln!("machinesched exit edges {held:?}");
                    }
                    copy_constrain(slice, &mut deps, &|value| now.contains(&value));
                    cluster_mem_ops(slice, &mut deps, sched);
                    let order = order_region(slice, &deps, tracking.as_ref(), sched);
                    if order.iter().enumerate().any(|(place, &node)| place != node) {
                        for (place, &node) in order.iter().enumerate() {
                            out[region.start + place] = Arc::clone(&slice[node]);
                        }
                        changed = true;
                    }
                }
                for one in slice.iter().rev() {
                    step(&mut now, one);
                    step87(&mut now87, one);
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
    use crate::backend::schedmodel::{GenericModel, SchedulingModel};
    use crate::model::ir::{Held, Loc, Mem, Operation, Semantics};
    use crate::model::lir::Insn;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4, offset: 0 })
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

    /// Values 1..=9 weigh one in a single set whose limit is `limit`.
    fn one_set(limit: u32) -> crate::backend::regpressure::Model {
        let mut model = crate::backend::regpressure::Model::new(vec![limit]);
        for value in 1..=9 {
            model.count(value, 1, std::rc::Rc::from(vec![0]));
        }
        model
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

    /// A write whose flags nothing reads joined the defs standing in its
    /// lane, and every later write scanned them all for the live def to order
    /// after: a region of 40,000 flag-writing instructions took seconds
    /// (machinesched `graph` 12.9 M scans of 4,290 instructions).
    #[test]
    fn test_a_long_run_of_dead_flag_writes_builds_its_graph_in_linear_time() {
        use super::graph;
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let insns: Vec<_> = (0..40_000u32)
            .map(|k| {
                let at = i64::from(k);
                insn(
                    at,
                    semantics(Operation::Binary, "add", vec![held(k + 1)], vec![held(k), imm(1)]),
                    vec![k + 1],
                    vec![k],
                )
            })
            .collect();
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let started = std::time::Instant::now();
        let found = graph(&insns, regs, 16, &GenericModel, None);
        assert_eq!(found.preds.len(), insns.len());
        assert!(started.elapsed().as_millis() < 1000, "{:?} for 40,000 adds", started.elapsed());
    }

    /// `copy_constrain` scanned the whole region for each value's ranges and
    /// uses, and walked the whole graph for each reachability query: a region
    /// of 6,000 copies took seconds (machinesched 187 -> 704 Minstr from 128
    /// to 256 statements of a block).
    #[test]
    fn test_constraining_the_copies_of_a_long_region_is_not_quadratic() {
        use super::{copy_constrain, graph};
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let mut insns = Vec::new();
        for k in 0..6_000u32 {
            let (l, g, t, x) = (10 + 4 * k, 11 + 4 * k, 12 + 4 * k, 13 + 4 * k);
            let at = insns.len() as i64;
            insns.push(insn(at, semantics(Operation::Move, "mov", vec![held(l)], vec![held(g)]), vec![l], vec![g]));
            insns.push(insn(at + 1, semantics(Operation::Move, "mov", vec![held(g)], vec![held(x)]), vec![g], vec![x]));
            insns.push(insn(
                at + 2,
                semantics(Operation::Binary, "imul", vec![held(t)], vec![held(l), imm(3)]),
                vec![t],
                vec![l],
            ));
        }
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let mut found = graph(&insns, regs, 16, &GenericModel, None);
        let started = std::time::Instant::now();
        copy_constrain(&insns, &mut found, &|_| false);
        assert!(started.elapsed().as_millis() < 1000, "{:?} to constrain 6,000 copies", started.elapsed());
    }

    /// `l = g; g = x; t = l * 3; out = t + g` (x_iir's `%2 = COPY %8`, `%8 =
    /// COPY %6`, `imul %2`): the pin's `CopyConstrain` keeps the use of the
    /// copy before the global's next def, so the copy dies at once and
    /// coalesces. Without it the schedule was copy, def, `imul`: the copy's
    /// register stayed live across the def and could not coalesce, and a
    /// `mov` more was left in the loop.
    #[test]
    fn test_the_use_of_a_copy_stays_before_the_next_def_of_the_copied_value() {
        use super::{copy_constrain, order_region};
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(2)], vec![held(1)]), vec![2], vec![1]),
            insn(1, semantics(Operation::Move, "mov", vec![held(1)], vec![held(4)]), vec![1], vec![4]),
            insn(2, semantics(Operation::Binary, "imul", vec![held(3)], vec![held(2), imm(3)]), vec![3], vec![2]),
            insn(3, semantics(Operation::Binary, "add", vec![held(5)], vec![held(3), held(1)]), vec![5], vec![3, 1]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let mut found = graph(&insns, regs, 16, &GenericModel, None);
        assert_eq!(
            order_region(&insns, &found, None, &GenericModel),
            [0, 1, 2, 3],
            "premise: program order without the edge"
        );
        copy_constrain(&insns, &mut found, &|value| value == 1);
        assert_eq!(found.weak_preds[1], [2]);
        assert_eq!(order_region(&insns, &found, None, &GenericModel), [0, 2, 1, 3]);
    }

    /// `l = g; g = g + 1; t = l * 3; out = t + g`: the pin returns at "two
    /// address defs have no hole" when the next def of the copied value reads
    /// it, so it adds no edge. Merging the two segments added one: x_ll_arith
    /// (`%15 = COPY %13` ... `%13 = SHLD %13`) got a schedule 16 bytes larger.
    #[test]
    fn test_no_edge_is_added_before_a_two_address_def_of_the_copied_value() {
        use super::copy_constrain;
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(2)], vec![held(1)]), vec![2], vec![1]),
            insn(1, semantics(Operation::Binary, "add", vec![held(1)], vec![held(1), imm(1)]), vec![1], vec![1]),
            insn(2, semantics(Operation::Binary, "imul", vec![held(3)], vec![held(2), imm(3)]), vec![3], vec![2]),
            insn(3, semantics(Operation::Binary, "add", vec![held(5)], vec![held(3), held(1)]), vec![5], vec![3, 1]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let mut found = graph(&insns, regs, 16, &GenericModel, None);
        copy_constrain(&insns, &mut found, &|value| value == 1);
        assert!(found.weak_preds.iter().all(Vec::is_empty), "{:?}", found.weak_preds);
    }

    /// `x = load [m]; y = x + 1; z = load [n]; store [m], y`: a load feeds the
    /// add, the add and both loads precede the store, and the load of `n`
    /// is ordered with neither the first load nor the add.
    #[test]
    fn test_the_graph_orders_data_and_a_load_before_a_store_and_nothing_else() {
        let insns = body();
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let found = graph(&insns, regs, 16, &GenericModel, None);
        assert_eq!(found.preds[1], [0], "the add reads the first load");
        assert!(found.preds[2].is_empty(), "the second load is ordered with nothing before it: {:?}", found.preds);
        assert_eq!(found.preds[3], [0, 1, 2], "the store follows both loads and the add");
    }

    /// A load or store as arm64 states it: the memory lane and, when `object`
    /// is given, what it reaches (`Insn::memory`).
    fn access(
        at: i64,
        store: bool,
        object: Option<(i64, u64, u64)>,
    ) -> Arc<Insn> {
        use crate::backend::peephole::Lanes;
        use crate::model::memref::MemRef;
        let memory = Lanes::flags(Lanes::MEMORY);
        let name = if store { "str" } else { "ldr" };
        let mut one =
            Insn::new(at, Some((at, at)), Some(semantics(Operation::Move, name, vec![], vec![])), vec![], vec![]);
        one.declared = Some((memory, if store { memory } else { Lanes::new() }));
        one.memory = object.map(|(slot, offset, bytes)| MemRef {
            object: Some(llrm_analysis::placealias::Object::Frame { id: slot, fixed: false }),
            offset: Some(offset as i64),
            bytes: Some(bytes),
            store,
        });
        Arc::new(one)
    }

    /// The memory lane ordered every store before every later access, so a load
    /// from one frame slot stayed behind a store to another: the scheduler
    /// could not move it above the store (`addChainDependencies` asks
    /// `MachineInstr::mayAlias`). A load of the stored bytes stays behind; an
    /// access that says nothing is ordered with everything before it.
    #[test]
    fn test_accesses_to_different_slots_are_not_ordered_by_the_memory_lane() {
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let order = |insns: Vec<Arc<Insn>>| graph(&insns, regs, 16, &GenericModel, None).preds;
        // No operand: the lane alone, a store then a load are ordered.
        assert_eq!(order(vec![access(0, true, None), access(1, false, None)])[1], [0]);
        // The operand says another slot: no edge.
        let apart = order(vec![access(0, true, Some((1, 0, 8))), access(1, false, Some((2, 0, 8)))]);
        assert!(apart[1].is_empty(), "{apart:?}");
        // The same bytes: ordered.
        assert_eq!(order(vec![access(0, true, Some((1, 0, 8))), access(1, false, Some((1, 4, 8)))])[1], [0]);
        // A load of the first slot after stores to two: behind the first only.
        let after = order(vec![
            access(0, true, Some((1, 0, 8))),
            access(1, true, Some((2, 0, 8))),
            access(2, false, Some((1, 0, 8))),
            access(3, true, None),
        ]);
        assert_eq!(after[2], [0], "{after:?}");
        assert_eq!(after[3], [0, 1, 2], "an access of unknown reach follows everything: {after:?}");
        // A store after a load of another slot is not ordered after it.
        let war = order(vec![access(0, false, Some((1, 0, 8))), access(1, true, Some((2, 0, 8)))]);
        assert!(war[1].is_empty(), "{war:?}");
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

    /// `modifies_stack_pointer`: `add sp, 4` is neither a push, a pop nor a
    /// call, and `_lanes` leaves SP out of the lanes it orders by, so the two
    /// loads around it were one region and could swap across the write.
    #[test]
    fn test_a_write_of_the_stack_pointer_that_is_not_a_push_or_pop_ends_a_region() {
        let mut insns = body();
        let sp = Loc::Reg(llrm_lir::Reg { register: llrm_lir::registers::RegId::SP, width: 2 });
        let imm = Loc::Imm(crate::model::ir::Imm { value: 4, width: 2, address: None });
        insns.insert(2, insn(9, semantics(Operation::Binary, "add", vec![sp.clone()], vec![sp, imm]), vec![], vec![]));
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        assert_eq!(regions(&insns, regs, 16), vec![0..2, 3..5]);
    }

    /// Three constants feed two adds and a store. In source order all three
    /// are live together (3 values); bottom-up with a limit of two places
    /// the third load just above the add that reads it, so at most two are
    /// live.
    #[test]
    fn test_bottom_up_places_a_load_by_its_reader_when_the_limit_is_two() {
        use super::order_region;
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![imm(11)]), vec![1], vec![]),
            insn(1, semantics(Operation::Move, "mov", vec![held(2)], vec![imm(12)]), vec![2], vec![]),
            insn(2, semantics(Operation::Move, "mov", vec![held(3)], vec![imm(13)]), vec![3], vec![]),
            insn(3, semantics(Operation::Binary, "add", vec![held(4)], vec![held(1), held(2)]), vec![4], vec![1, 2]),
            insn(4, semantics(Operation::Binary, "add", vec![held(5)], vec![held(4), held(3)]), vec![5], vec![4, 3]),
            insn(5, semantics(Operation::Binary, "add", vec![held(6)], vec![held(5), imm(1)]), vec![6], vec![5]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let found = graph(&insns, regs, 16, &GenericModel, None);
        let model = one_set(2);
        let tracking = super::Tracking { model: &model, live_out: vec![6], live_thru: vec![] };
        assert_eq!(order_region(&insns, &found, Some(&tracking), &GenericModel), [0, 1, 3, 2, 4, 5]);
        // Pressure not followed (a small region) leaves the order alone.
        assert_eq!(order_region(&insns, &found, None, &GenericModel), [0, 1, 2, 3, 4, 5]);
    }

    /// The long region of a block was kept in source order whatever the
    /// pressure: three constants were all live at once (3 values) with a limit
    /// of 2. Through the pass the third load sits just above its reader.
    #[test]
    fn test_the_pass_orders_a_long_region_by_pressure() {
        use super::scheduled;
        use crate::model::lir::{LirBlock, LirBody};
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let mut insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![imm(11)]), vec![1], vec![]),
            insn(1, semantics(Operation::Move, "mov", vec![held(2)], vec![imm(12)]), vec![2], vec![]),
            insn(2, semantics(Operation::Move, "mov", vec![held(3)], vec![imm(13)]), vec![3], vec![]),
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
        let defined: Vec<u32> = scheduled(&body, 2, &GenericModel, |_, _| one_set(2)).blocks[0]
            .insns
            .iter()
            .flat_map(|one| one.defines.clone())
            .collect();
        assert_eq!(defined, [1, 2, 4, 3, 5, 6]);
        let kept: Vec<u32> = scheduled(&body, 16, &GenericModel, |_, _| one_set(2)).blocks[0]
            .insns
            .iter()
            .flat_map(|one| one.defines.clone())
            .collect();
        assert_eq!(kept, [1, 2, 3, 4, 5, 6]);
    }

    /// A scheduled instruction took the `at` of the place it moved to, but an
    /// `at` names the instruction: the optimizer's `spares` pairs, and the
    /// compare a branch reads, find theirs by it. After a region is ordered
    /// each instruction still has its own.
    #[test]
    fn test_a_moved_instruction_keeps_its_at() {
        use super::scheduled;
        use crate::model::lir::{LirBlock, LirBody};
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let mut insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![imm(11)]), vec![1], vec![]),
            insn(1, semantics(Operation::Move, "mov", vec![held(2)], vec![imm(12)]), vec![2], vec![]),
            insn(2, semantics(Operation::Move, "mov", vec![held(3)], vec![imm(13)]), vec![3], vec![]),
            insn(3, semantics(Operation::Binary, "add", vec![held(4)], vec![held(1), held(2)]), vec![4], vec![1, 2]),
            insn(4, semantics(Operation::Binary, "add", vec![held(5)], vec![held(4), held(3)]), vec![5], vec![4, 3]),
            insn(5, semantics(Operation::Binary, "add", vec![held(6)], vec![held(5), imm(1)]), vec![6], vec![5]),
        ];
        insns.push(insn(6, semantics(Operation::Return, "ret", vec![], vec![held(6)]), vec![], vec![6]));
        let mut body = LirBody::new(
            "kept",
            0,
            vec![LirBlock::new(0, insns).into()],
            crate::support::hash::IndexMap::default(),
            crate::support::hash::IndexMap::default(),
        );
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let after = scheduled(&body, 2, &GenericModel, |_, _| one_set(2));
        let order: Vec<u32> = after.blocks[0].insns.iter().flat_map(|one| one.defines.clone()).collect();
        assert_ne!(order, [1, 2, 3, 4, 5, 6], "the load of 3 moved");
        for one in after.blocks[0].insns.iter() {
            let first = one.defines.first().copied().unwrap_or(7);
            assert_eq!(one.at, i64::from(first) - 1, "the instruction defining {first} kept its at");
        }
    }

    /// A load was held back four cycles above its reader (the unbuffered
    /// default model's `getLoadLatency`), which hoisted the loads of a block
    /// to its top: x_dct +10 instructions. The i486's `GenericModel` is
    /// buffered, so a load is available as soon as its reader is placed and
    /// the order is the source's.
    #[test]
    fn test_a_load_stays_next_to_its_reader() {
        use super::order_region;
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![imm(1)]), vec![1], vec![]),
            insn(1, semantics(Operation::Move, "mov", vec![held(2)], vec![imm(2)]), vec![2], vec![]),
            insn(2, semantics(Operation::Move, "mov", vec![held(3)], vec![cell(0)]), vec![3], vec![]),
            insn(3, semantics(Operation::Binary, "add", vec![held(4)], vec![held(3), imm(1)]), vec![4], vec![3]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let found = graph(&insns, regs, 16, &GenericModel, None);
        assert_eq!(found.latencies[3], [4], "the reader still waits four cycles for the load");
        assert_eq!(order_region(&insns, &found, None, &GenericModel), [0, 1, 2, 3]);
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
        assert_eq!(graph(&insns, regs, 16, &GenericModel, None).preds[1], [0], "the zero stays before the add");
    }

    /// Flags no one reads before the next flag writer are a dead def, and the
    /// pin adds no output edge between two dead defs (`addPhysRegDeps`). Ours
    /// chained every `add` through the flags, so the seven `add v1, K` leaves
    /// of bench_parity stayed in a row above their readers (4 spill ops, pin
    /// 0).
    #[test]
    fn test_two_flag_writes_nobody_reads_have_no_edge() {
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let insns = vec![
            insn(0, semantics(Operation::Binary, "add", vec![held(2)], vec![held(1), imm(4)]), vec![2], vec![1]),
            insn(1, semantics(Operation::Binary, "add", vec![held(3)], vec![held(1), imm(7)]), vec![3], vec![1]),
            insn(2, semantics(Operation::Binary, "add", vec![held(4)], vec![held(2), held(3)]), vec![4], vec![2, 3]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let found = graph(&insns, regs, 32, &GenericModel, None);
        assert!(found.preds[1].is_empty(), "the second add waits for the first only through flags: {:?}", found.preds);
        assert_eq!(found.preds[2], [0, 1], "the third reads both");
    }

    /// A dead flag write keeps the readers of the def before it: a second dead
    /// write moved above the `adc` that reads the `cmp`'s carry and clobbered
    /// it (call_keeps and regparm3_matrix_4 printed wrong values with the
    /// first version of the dead-def rule, which cleared the readers).
    #[test]
    fn test_a_dead_flag_write_stays_after_the_reader_of_the_def_before_it() {
        let regs = llrm_lir::registers::Regs(&llrm_x86_m32::REGISTER_INFO);
        let add = |at: i64, out: u32, a: u32, b: u32| {
            insn(
                at,
                semantics(Operation::Binary, "add", vec![held(out)], vec![held(a), held(b)]),
                vec![out],
                vec![a, b],
            )
        };
        let compare = insn(0, semantics(Operation::Compare, "cmp", vec![], vec![held(1), held(2)]), vec![], vec![1, 2]);
        let carry =
            insn(1, semantics(Operation::Binary, "adc", vec![held(5)], vec![held(5), held(6)]), vec![5], vec![5, 6]);
        let insns = vec![compare, carry, add(2, 10, 11, 12), add(3, 13, 14, 15), add(4, 16, 17, 18)];
        let g = graph(&insns, regs, 32, &GenericModel, None);
        assert!(g.preds[3].contains(&1), "the second dead write stays after the adc: {g:?}");
    }

    /// `push` writes the stack pointer, which no lane orders: the scheduler put
    /// `push ss; push ax` ahead of the two `pushd` that preceded them, and
    /// the far call read its arguments reversed (regparm3_matrix_4 printed
    /// 2552 for 653). The pin ends a region at what writes
    /// the stack pointer (`isSchedulingBoundary`).
    #[test]
    fn test_a_push_ends_a_region_and_is_in_none() {
        let insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![cell(0)]), vec![1], vec![]),
            insn(1, semantics(Operation::Push, "push", vec![], vec![held(1)]), vec![], vec![1]),
            insn(2, semantics(Operation::Push, "push", vec![], vec![held(1)]), vec![], vec![1]),
            insn(3, semantics(Operation::Move, "mov", vec![held(2)], vec![cell(8)]), vec![2], vec![]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        assert_eq!(regions(&insns, regs, 16), [0..1, 3..4]);
    }

    /// One test per class of the pin's `isSchedulingBoundary`: each is a
    /// boundary, a plain `mov` is not. A `push` was none of them (the stack
    /// pointer is written by no lane we order), so arguments were pushed in
    /// a new order (regparm3_matrix_4 printed 2552 for 653).
    #[test]
    fn test_every_class_of_the_pins_scheduling_boundary_is_one() {
        use super::boundary;
        use crate::model::ir::Reg;
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let sp = Loc::Reg(Reg { register: llrm_lir::registers::RegId::SP, width: 2 });
        let plain = insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![cell(0)]), vec![1], vec![]);
        let frame = {
            let mut one = (*plain).clone();
            one.frame_adjust = true;
            std::sync::Arc::new(one)
        };
        let meta = std::sync::Arc::new(Insn::new(0, Some((0, 0)), None, vec![], vec![]));
        let classes = [
            ("call", insn(0, semantics(Operation::Call, "call", vec![], vec![]), vec![], vec![])),
            ("terminator", insn(0, semantics(Operation::Jump, "jmp", vec![], vec![]), vec![], vec![])),
            ("branch", insn(0, semantics(Operation::Branch, "je", vec![], vec![]), vec![], vec![])),
            ("return", insn(0, semantics(Operation::Return, "ret", vec![], vec![]), vec![], vec![])),
            ("position", meta),
            ("inline asm", insn(0, semantics(Operation::Escape, "asm", vec![], vec![]), vec![], vec![])),
            ("push", insn(0, semantics(Operation::Push, "push", vec![], vec![held(1)]), vec![], vec![1])),
            ("pop", insn(0, semantics(Operation::Pop, "pop", vec![held(1)], vec![]), vec![1], vec![])),
            ("leave", insn(0, semantics(Operation::Leave, "leave", vec![], vec![]), vec![], vec![])),
            (
                "add sp",
                insn(
                    0,
                    semantics(
                        Operation::Binary,
                        "add",
                        vec![sp.clone()],
                        vec![sp.clone(), Loc::Imm(crate::model::ir::Imm { value: 8, width: 2, address: None })],
                    ),
                    vec![],
                    vec![],
                ),
            ),
            ("frame setup or destroy", frame),
        ];
        for (class, one) in &classes {
            assert!(boundary(regs, 16, one), "{class} is a boundary");
        }
        assert!(!boundary(regs, 16, &plain), "a plain mov is not");
    }

    /// An x87 compare's decoded effect writes the status word only; `fnstsw;
    /// sahf` copy it into the flags later. A flag reader after it was
    /// hoisted above it (wc_mixed after_double: `x > 2.5 ? 30 : 20`). In
    /// the graph the compare writes the flags.
    #[test]
    fn test_an_x87_compare_is_ordered_before_a_flag_reader() {
        use super::x87_compare;
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let wide = |value: u32| Loc::Held(crate::model::ir::Held { value, width: 10, offset: 0 });
        let insns = vec![
            insn(0, semantics(Operation::Compare, "fcom", vec![], vec![wide(1), wide(2)]), vec![], vec![1, 2]),
            insn(1, semantics(Operation::Binary, "adc", vec![held(3)], vec![held(3), held(4)]), vec![3], vec![3, 4]),
        ];
        assert_eq!(graph(&insns, regs, 16, &GenericModel, None).preds[1], [0], "the adc stays after the fcom");
        for name in ["fcom", "fcomp", "fcompp", "fucom", "fucomp", "fucompp", "ficom", "ficomp", "ftst", "fxam"] {
            let one = insn(0, semantics(Operation::Compare, name, vec![], vec![]), vec![], vec![]);
            assert!(x87_compare(&one), "{name}");
        }
        let plain = insn(0, semantics(Operation::Compare, "cmp", vec![], vec![held(1), held(2)]), vec![], vec![1, 2]);
        assert!(!x87_compare(&plain));
    }

    /// The FPU control and status words are written and read by name, which no
    /// lane orders.
    #[test]
    fn test_the_fpu_words_are_boundaries() {
        use super::boundary;
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        for name in ["fldcw", "fnstcw", "fnstsw"] {
            let one = insn(0, semantics(Operation::Move, name, vec![], vec![]), vec![], vec![]);
            assert!(boundary(regs, 16, &one), "{name}");
        }
    }

    /// A `seta` was scheduled above the `fcom` whose flags it reads (sdag-d's
    /// wc_mixed after_double: `a*100 + (x > 2.5 ? 30 : 20) + b` added 20 for x
    /// > 2.5).
    #[test]
    fn test_a_set_stays_after_the_float_compare_it_reads() {
        let float = |value: u32| Loc::Held(crate::model::ir::Held { value, width: 10, offset: 0 });
        let byte = Loc::Held(crate::model::ir::Held { value: 3, width: 1, offset: 0 });
        let insns = vec![
            insn(0, semantics(Operation::Compare, "fcom", vec![], vec![float(1), float(2)]), vec![], vec![1, 2]),
            insn(1, semantics(Operation::Unary, "seta", vec![byte], vec![]), vec![3], vec![]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m32::REGISTER_INFO);
        assert_eq!(graph(&insns, regs, 32, &GenericModel, None).preds[1], [0], "the set stays after the fcom");
    }

    /// The audit of the two effects the re-land left unchecked. A shift by `cl`
    /// writes the flags (the pin models it as a def), so a flag reader
    /// after it stays after it; `fcmov` reads the flags, so it stays after
    /// the compare that sets them.
    #[test]
    fn test_a_variable_shift_writes_flags_and_an_fcmov_reads_them() {
        use super::{ARITHMETIC_FLAGS, liveness};
        let regs = llrm_lir::registers::Regs(&llrm_x86_m32::REGISTER_INFO);
        let cl = Loc::Reg(crate::model::ir::Reg { register: llrm_lir::registers::RegId::CL, width: 1 });
        let shift = insn(1, semantics(Operation::Binary, "shl", vec![held(3)], vec![held(3), cl]), vec![3], vec![3]);
        let compare = insn(0, semantics(Operation::Compare, "cmp", vec![], vec![held(1), held(2)]), vec![], vec![1, 2]);
        let carry =
            insn(2, semantics(Operation::Binary, "adc", vec![held(5)], vec![held(5), held(6)]), vec![5], vec![5, 6]);
        let flags = crate::backend::peephole::Lanes::flags(ARITHMETIC_FLAGS);
        let shift_effect = liveness::effect(regs, 32, &shift).expect("a shift by cl has a decoded effect");
        assert!(!shift_effect.writes.is_disjoint(&flags), "the shl writes the flags");
        let g = graph(&[compare.clone(), shift, carry], regs, 32, &GenericModel, None);
        assert!(g.preds[2].contains(&1), "adc stays after the shl: {g:?}");
        let wide = |value: u32| Loc::Held(crate::model::ir::Held { value, width: 10, offset: 0 });
        let fcmov = insn(1, semantics(Operation::Move, "fcmove", vec![wide(7)], vec![wide(8)]), vec![7], vec![7, 8]);
        // Either its effect is unknown (a boundary) or it reads the flags the
        // compare set.
        let known = liveness::effect(regs, 32, &fcmov).map(|effect| !effect.all_reads().is_disjoint(&flags));
        assert!(known.is_none_or(|reads| reads), "an fcmov with a known effect reads the flags");
        let g = graph(&[compare, fcmov], regs, 32, &GenericModel, None);
        assert_eq!(g.preds[1], [0], "the fcmov stays after the cmp: {g:?}");
    }

    /// A zero made between an `add` and the `adc` that takes its carry wrote
    /// the flags in the graph, so the add's carry read as dead and the add
    /// kept no edge to the flag writes before it: the machine scheduler hoisted
    /// the add above an `or` and two `mul`s, and the `adc` read their carry
    /// (x_m64 and softfloat -O2 lost carries of a 128-bit product).
    #[test]
    fn test_an_add_keeps_its_place_after_flag_writes_when_a_zero_sits_before_its_adc() {
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let insns = vec![
            insn(0, semantics(Operation::Binary, "or", vec![held(1)], vec![held(1), imm(1)]), vec![1], vec![1]),
            insn(1, semantics(Operation::Binary, "add", vec![held(2)], vec![held(2), held(3)]), vec![2], vec![2, 3]),
            insn(2, semantics(Operation::Move, "mov", vec![held(4)], vec![imm(0)]), vec![4], vec![]),
            insn(3, semantics(Operation::Binary, "adc", vec![held(5)], vec![held(5), held(4)]), vec![5], vec![5, 4]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m32::REGISTER_INFO);
        let found = graph(&insns, regs, 32, &GenericModel, None);
        assert!(found.preds[1].contains(&0), "the add stays after the or: {found:?}");
    }

    #[test]
    fn test_the_scheduler_sets_are_not_siphashed() {
        // std's `HashSet<u32>` hashed every pressure query with SipHash.
        let none: [Arc<Insn>; 0] = [];
        let _: crate::support::hash::HashSet<u32> = super::x87_values(none.iter());
    }

    /// The order `bottom_up` gave before it ran on the `GenericScheduler`
    /// driver (release/next c682533d86), untracked path: the later node wins a
    /// tie once fewer weak successors wait, and nothing else decides.
    fn old_untracked_order(graph: &super::Graph) -> Vec<usize> {
        let n = graph.succs.len();
        let mut waiting: Vec<usize> = graph.succs.iter().map(Vec::len).collect();
        let mut available: Vec<usize> = (0..n).filter(|&node| waiting[node] == 0).collect();
        let mut weak_left: Vec<usize> = graph.weak_succs.iter().map(Vec::len).collect();
        let mut placed = Vec::with_capacity(n);
        while placed.len() < n {
            let mut best = 0;
            for candidate in 1..available.len() {
                let (try_node, cand_node) = (available[candidate], available[best]);
                let (try_weak, cand_weak) = (weak_left[try_node], weak_left[cand_node]);
                let wins = if try_weak != cand_weak { try_weak < cand_weak } else { try_node > cand_node };
                if wins {
                    best = candidate;
                }
            }
            let node = available.swap_remove(best);
            placed.push(node);
            for &before in &graph.weak_preds[node] {
                weak_left[before] -= 1;
            }
            for &before in &graph.preds[node] {
                waiting[before] -= 1;
                if waiting[before] == 0 {
                    available.push(before);
                }
            }
        }
        placed.reverse();
        placed
    }

    /// The refactor onto the driver must not move an x86 schedule: on random
    /// dependence graphs with weak edges the i486 machine orders exactly as
    /// the hand-written loop it replaced (0 of 300 graphs differ).
    #[test]
    fn test_bottom_up_on_the_driver_orders_as_the_loop_it_replaced() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = move |bound: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % bound as u64) as usize
        };
        for _ in 0..300 {
            let n = 2 + next(24);
            let mut edges = Vec::new();
            for to in 1..n {
                for from in 0..to {
                    if next(5) == 0 {
                        edges.push((from, to, 1 + next(3) as u32));
                    }
                }
            }
            let mut graph = super::Graph::from_edges(n, &edges);
            for to in 1..n {
                for from in 0..to {
                    if next(9) == 0 && !graph.weak_preds[to].contains(&from) {
                        graph.weak_preds[to].push(from);
                        graph.weak_succs[from].push(to);
                    }
                }
            }
            let insns: Vec<Arc<Insn>> = (0..n)
                .map(|at| insn(at as i64, semantics(Operation::Move, "x", vec![], vec![]), vec![], vec![]))
                .collect();
            assert_eq!(
                super::order_region(&insns, &graph, None, &GenericModel),
                old_untracked_order(&graph),
                "{edges:?}"
            );
        }
    }

    /// The old loop with pressure followed (release/next c682533d86): register
    /// excess, register critical, weak, register max, then the later node.
    fn old_tracked_order(
        insns: &[Arc<Insn>],
        graph: &super::Graph,
        tracking: &super::Tracking,
    ) -> Vec<usize> {
        use crate::backend::regpressure::{Pick, Region, Tracker, try_pressure};
        let n = insns.len();
        let mut waiting: Vec<usize> = graph.succs.iter().map(Vec::len).collect();
        let mut available: Vec<usize> = (0..n).filter(|&node| waiting[node] == 0).collect();
        let mut weak_left: Vec<usize> = graph.weak_succs.iter().map(Vec::len).collect();
        let region = Region::of(insns);
        let mut program = Tracker::over(tracking.model, &region, tracking.live_out.iter().copied(), [], false);
        for node in (0..n).rev() {
            program.place(node);
        }
        let region_max = program.max().to_vec();
        let mut critical = Tracker::critical_sets(tracking.model, &region_max);
        let mut tracker = Tracker::over(
            tracking.model,
            &region,
            tracking.live_out.iter().copied(),
            tracking.live_thru.iter().copied(),
            true,
        );
        let mut placed = Vec::with_capacity(n);
        while placed.len() < n {
            let mut best = 0;
            let deltas: Vec<_> = available.iter().map(|&node| tracker.delta(node, &critical, &region_max)).collect();
            for candidate in 1..available.len() {
                let (try_node, cand_node) = (available[candidate], available[best]);
                let (try_delta, cand_delta) = (&deltas[candidate], &deltas[best]);
                let pressure = |try_p, cand_p| match try_pressure(try_p, cand_p, true) {
                    Pick::Try => Some(true),
                    Pick::Cand => Some(false),
                    Pick::Neither => None,
                };
                let wins = pressure(try_delta.excess, cand_delta.excess)
                    .or_else(|| pressure(try_delta.critical_max, cand_delta.critical_max))
                    .or_else(|| {
                        let (try_weak, cand_weak) = (weak_left[try_node], weak_left[cand_node]);
                        (try_weak != cand_weak).then_some(try_weak < cand_weak)
                    })
                    .or_else(|| pressure(try_delta.current_max, cand_delta.current_max))
                    .unwrap_or(try_node > cand_node);
                if wins {
                    best = candidate;
                }
            }
            let node = available.swap_remove(best);
            tracker.place(node);
            for entry in &mut critical {
                let most = tracker.max()[entry.set] as i32;
                if most > entry.inc {
                    entry.inc = most;
                }
            }
            placed.push(node);
            for &before in &graph.weak_preds[node] {
                weak_left[before] -= 1;
            }
            for &before in &graph.preds[node] {
                waiting[before] -= 1;
                if waiting[before] == 0 {
                    available.push(before);
                }
            }
        }
        placed.reverse();
        placed
    }

    /// Same as the untracked oracle with pressure followed: on 300 random
    /// value graphs and limits of 1 to 4 the driver orders as the loop it
    /// replaced (0 differ, and the limit does split the picks: some graphs
    /// order differently from the untracked order).
    #[test]
    fn test_bottom_up_with_pressure_orders_as_the_loop_it_replaced() {
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move |bound: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % bound as u64) as usize
        };
        let mut moved_by_pressure = 0;
        for _ in 0..300 {
            let n = 3 + next(6);
            let insns: Vec<Arc<Insn>> = (0..n)
                .map(|k| {
                    let value = k as u32 + 1;
                    let uses: Vec<u32> = if k == 0 {
                        vec![]
                    } else {
                        (0..1 + next(2))
                            .map(|_| 1 + next(k) as u32)
                            .collect::<std::collections::BTreeSet<_>>()
                            .into_iter()
                            .collect()
                    };
                    let sources =
                        if uses.is_empty() { vec![imm(k as i64)] } else { uses.iter().map(|&u| held(u)).collect() };
                    insn(k as i64, semantics(Operation::Binary, "add", vec![held(value)], sources), vec![value], uses)
                })
                .collect();
            let found = super::graph(&insns, regs, 16, &GenericModel, None);
            let model = one_set(1 + next(4) as u32);
            let tracking = super::Tracking { model: &model, live_out: vec![n as u32], live_thru: vec![] };
            let order = super::order_region(&insns, &found, Some(&tracking), &GenericModel);
            assert_eq!(order, old_tracked_order(&insns, &found, &tracking), "{insns:?}");
            moved_by_pressure += usize::from(order != super::order_region(&insns, &found, None, &GenericModel));
        }
        assert!(moved_by_pressure > 0, "pressure never decided a pick: the oracle compares nothing");
    }

    /// The model of a processor like the generic one that schedules from both
    /// ends (AArch64's `overrideSchedPolicy`).
    struct Both;

    impl crate::backend::schedmodel::SchedulingModel for Both {
        fn machine(&self) -> crate::backend::schedboundary::Machine {
            GenericModel.machine()
        }

        fn policy(&self) -> crate::backend::genericdriver::RegionPolicy {
            crate::backend::genericdriver::RegionPolicy::bidirectional(false)
        }

        fn unit(
            &self,
            insn: &Insn,
        ) -> crate::backend::schedboundary::Unit {
            GenericModel.unit(insn)
        }

        fn def_latency(
            &self,
            insn: &Insn,
        ) -> u32 {
            GenericModel.def_latency(insn)
        }
    }

    /// Limit 2; `v1`-`v3` live in, `v4` and `v5` live out; two independent
    /// nodes, an add and a copy. Hand derived from `pickNodeBidirectional`, as
    /// the pin's `tryCandidate` orders it: neither zone has a single
    /// choice. At the bottom (pressure 2) node 0 would take it to 3, an
    /// excess, and node 1 leaves it as it is: node 1 wins `RegExcess`. At
    /// the top (pressure 3, over the limit) node 0 ends `v1` and `v2` and
    /// starts `v4`: a drop of one, which beats node 1's no change, so node
    /// 0 is the top candidate. Comparing the two, a drop beats no change:
    /// the first pick is node 0 from the top. A region whose top zone
    /// follows no pressure sees two equal candidates and keeps the bottom
    /// one, node 1.
    #[test]
    fn test_a_bidirectional_region_compares_the_pressure_of_both_zones() {
        let insns = vec![
            insn(0, semantics(Operation::Binary, "add", vec![held(4)], vec![held(1), held(2)]), vec![4], vec![1, 2]),
            insn(1, semantics(Operation::Move, "mov", vec![held(5)], vec![held(3)]), vec![5], vec![3]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let found = graph(&insns, regs, 16, &Both, None);
        // Only the add writes flags: the nodes are independent.
        assert_eq!((&found.preds[0][..], &found.preds[1][..]), (&[][..], &[][..]));
        let model = one_set(2);
        let tracking = super::Tracking { model: &model, live_out: vec![4, 5], live_thru: vec![] };
        let (order, made) = super::picks(&insns, &found, Some(&tracking), &Both);
        assert_eq!(made[0], (0, true, super::Reason::RegExcess));
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1], "every node once");
    }

    /// A value live across a region that neither reads nor writes it was
    /// counted in the region's pressure when another region of the block
    /// touched it: the pin finds live-outs only where the region defines or
    /// reads them (`discoverLiveOut`). `v7` is made before a call and read
    /// after the block: the region behind the call peaks at its limit (3) and
    /// is in no excess. Counting `v7` made the peak 4, a critical set, and the
    /// third constant moved down to its reader.
    #[test]
    fn test_a_value_live_across_a_region_untouched_adds_no_pressure() {
        use super::scheduled;
        use crate::model::lir::{LirBlock, LirBody};
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let mut insns = vec![
            insn(10, semantics(Operation::Move, "mov", vec![held(7)], vec![imm(99)]), vec![7], vec![]),
            insn(11, semantics(Operation::Call, "call", vec![], vec![]), vec![], vec![]),
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![imm(11)]), vec![1], vec![]),
            insn(1, semantics(Operation::Move, "mov", vec![held(2)], vec![imm(12)]), vec![2], vec![]),
            insn(2, semantics(Operation::Move, "mov", vec![held(3)], vec![imm(13)]), vec![3], vec![]),
            insn(3, semantics(Operation::Binary, "add", vec![held(4)], vec![held(1), held(2)]), vec![4], vec![1, 2]),
            insn(4, semantics(Operation::Binary, "add", vec![held(5)], vec![held(4), held(3)]), vec![5], vec![4, 3]),
            insn(5, semantics(Operation::Binary, "add", vec![held(6)], vec![held(5), imm(1)]), vec![6], vec![5]),
        ];
        let after =
            vec![insn(20, semantics(Operation::Return, "ret", vec![], vec![held(6), held(7)]), vec![], vec![6, 7])];
        let mut first = LirBlock::new(0, std::mem::take(&mut insns));
        first.succ = vec![1].into();
        let mut body = LirBody::new(
            "across",
            0,
            vec![first.into(), LirBlock::new(1, after).into()],
            crate::support::hash::IndexMap::default(),
            crate::support::hash::IndexMap::default(),
        );
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let defined: Vec<u32> = scheduled(&body, 2, &GenericModel, |_, _| one_set(3)).blocks[0]
            .insns
            .iter()
            .flat_map(|one| one.defines.clone())
            .collect();
        assert_eq!(defined, [7, 1, 2, 3, 4, 5, 6], "the region peaks at 3 of 3: no critical set, source order");
    }

    /// Two loads of neighbouring words: the model clusters them.
    struct Pairs;

    use crate::backend::genericdriver::RegionPolicy;
    use crate::backend::schedmodel::{MemBase, MemOp};

    impl crate::backend::schedmodel::SchedulingModel for Pairs {
        fn machine(&self) -> crate::backend::schedboundary::Machine {
            GenericModel.machine()
        }

        fn policy(&self) -> RegionPolicy {
            GenericModel.policy()
        }

        fn unit(
            &self,
            insn: &Insn,
        ) -> crate::backend::schedboundary::Unit {
            GenericModel.unit(insn)
        }

        fn def_latency(
            &self,
            insn: &Insn,
        ) -> u32 {
            GenericModel.def_latency(insn)
        }

        fn mem_op(
            &self,
            insn: &Insn,
        ) -> Option<MemOp> {
            let what = insn.what.as_ref()?;
            match what.sources.as_slice() {
                [Loc::Mem(cell)] if what.name.as_deref() == Some("mov") => {
                    Some(MemOp { base: MemBase::Virtual(100), offset: cell.offset, width: 4, load: true })
                }
                _ => None,
            }
        }

        fn should_cluster(
            &self,
            (_, a): (&Insn, &MemOp),
            (_, b): (&Insn, &MemOp),
            cluster_size: u32,
            _: u32,
        ) -> bool {
            cluster_size <= 2 && a.base == b.base && a.offset + 4 == b.offset
        }
    }

    /// `BaseMemOpClusterMutation` (MachineScheduler.cpp#L2264): two loads of
    /// neighbouring words get a cluster edge and stay side by side, which
    /// the load/store optimizer needs to make a pair. Without the mutation
    /// the scheduler (a pick by height and then order) put the first load's
    /// reader between them, and the AArch64 pairs of `matmul` and `nbody`
    /// (16 `ldp` of 30) were lost.
    #[test]
    fn test_neighbouring_loads_are_scheduled_together() {
        let insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![cell(0)]), vec![1], vec![]),
            insn(1, semantics(Operation::Move, "mov", vec![held(3)], vec![held(1)]), vec![3], vec![1]),
            insn(2, semantics(Operation::Move, "mov", vec![held(2)], vec![cell(4)]), vec![2], vec![]),
            insn(3, semantics(Operation::Move, "mov", vec![held(4)], vec![held(2)]), vec![4], vec![2]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let mut found = graph(&insns, regs, 16, &Pairs, None);
        let apart = super::order_region(&insns, &found, None, &Pairs);
        let distance = |order: &[usize]| {
            let at = |node: usize| order.iter().position(|&each| each == node).unwrap();
            at(0).abs_diff(at(2))
        };
        assert!(distance(&apart) > 1, "the premise: without clustering the loads are apart, {apart:?}");
        super::cluster_mem_ops(&insns, &mut found, &Pairs);
        let together = super::order_region(&insns, &found, None, &Pairs);
        assert_eq!(distance(&together), 1, "the loads are adjacent, {together:?}");
    }

    /// A model with the region-end edges and a four-cycle def.
    struct Exiting;

    impl crate::backend::schedmodel::SchedulingModel for Exiting {
        fn machine(&self) -> crate::backend::schedboundary::Machine {
            GenericModel.machine()
        }

        fn policy(&self) -> RegionPolicy {
            GenericModel.policy()
        }

        fn unit(
            &self,
            insn: &Insn,
        ) -> crate::backend::schedboundary::Unit {
            GenericModel.unit(insn)
        }

        fn def_latency(
            &self,
            _: &Insn,
        ) -> u32 {
            4
        }

        fn exit_edges(&self) -> bool {
            true
        }
    }

    /// pin: what the instruction that ends the region reads is held for the
    /// latency of its def (`ExitSU`'s data edges), so the compare that feeds a
    /// branch is not ready the moment the bottom zone starts. Without the
    /// edge arm64 `mandel` placed the counter's `add` before the `cmp` and
    /// copied the counter back (5 copies; the pin has the `cmp` first).
    /// A def nothing reads holds one cycle less; a def the end reads, the
    /// whole latency.
    #[test]
    fn test_what_the_region_end_reads_is_held_for_its_defs_latency() {
        let insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![cell(0)]), vec![1], vec![]),
            insn(1, semantics(Operation::Move, "mov", vec![held(2)], vec![cell(4)]), vec![2], vec![]),
        ];
        let end = insn(2, semantics(Operation::Move, "mov", vec![cell(8)], vec![held(1)]), vec![], vec![1]);
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let none = graph(&insns, regs, 16, &Exiting, None);
        assert_eq!(none.exit_latency, [3, 3], "no end: both unread defs hold latency less one");
        let read = graph(&insns, regs, 16, &Exiting, Some(&*end));
        assert_eq!(read.exit_latency, [4, 3], "the end reads the first: it holds the whole latency");
    }

    /// Every target but arm64 keeps `exit_edges()` off: the region end must not
    /// change its graph (x86, m68k, 6502, z80 and wasm are bit-identical).
    #[test]
    fn test_a_model_without_exit_edges_ignores_the_region_end() {
        let insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![cell(0)]), vec![1], vec![]),
            insn(1, semantics(Operation::Move, "mov", vec![held(2)], vec![cell(4)]), vec![2], vec![]),
        ];
        let end = insn(2, semantics(Operation::Move, "mov", vec![cell(8)], vec![held(1)]), vec![], vec![1]);
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let none = graph(&insns, regs, 16, &GenericModel, None);
        let read = graph(&insns, regs, 16, &GenericModel, Some(&*end));
        assert_eq!(read.exit_latency, none.exit_latency, "the end's reads are not held");
        assert_eq!(read.preds, none.preds);
    }
}
