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

use std::collections::BTreeSet;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::{RegId, Regs};

use crate::backend::allocate::{LiveAt, live_rows};
use crate::backend::classes::RegisterClasses;
use crate::backend::liveness;
use crate::backend::regclass;
use crate::backend::regpressure::{Delta, Model, Pick, PressureChange, Tracker, try_pressure};
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

/// CF, PF, AF, ZF, SF, OF of rflags.
const ARITHMETIC_FLAGS: u32 = 0x8d5;

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

/// The edges of a region, by position in it.
#[derive(Debug, Eq, PartialEq)]
pub struct Graph {
    /// For each instruction, the earlier ones that must stay before it.
    pub preds: Vec<Vec<usize>>,
    /// For each instruction, the later ones that must stay after it.
    pub succs: Vec<Vec<usize>>,
    /// The latency of each edge in `preds`, in the same order: what the
    /// earlier instruction takes to produce what the later one reads
    /// ([`def_latency`]), one for an output dependence, none for an anti
    /// dependence or a memory order.
    pub latencies: Vec<Vec<u32>>,
}

/// The latency of what `one` defines for the default scheduling model of a
/// target with none (i486): `TargetInstrInfo::defaultDefLatency` is zero for
/// a transient instruction (a copy), `getLoadLatency` (4) for a load, one for
/// the rest.
fn def_latency(one: &Insn) -> u32 {
    let Some(what) = &one.what else { return 1 };
    let copy = what.op == Operation::Move
        && matches!(
            (what.dests.as_slice(), what.sources.as_slice()),
            ([Loc::Held(_)], [Loc::Held(_)])
        );
    if copy || (what.op == Operation::Nothing && what.name.as_deref().is_none_or(str::is_empty)) {
        0
    } else if memory(one).0 {
        4
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
    let mut by_value: HashMap<u32, (Option<usize>, Vec<usize>)> = HashMap::default();
    // Per lane: the defs still standing (a live def replaces them, a dead one
    // joins them, as the pin's `Defs`) and the readers since.
    let mut by_lane: Vec<(Vec<usize>, Vec<usize>)> = vec![(Vec::new(), Vec::new()); 128];
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
                live = (live & !writes) | effect.all_reads().mask();
            }
            None => live = u128::MAX,
        }
    }
    // The last store and the loads since it; the last instruction of unknown
    // effect and everything since it.
    let (mut store, mut loads): (Option<usize>, Vec<usize>) = (None, Vec::new());
    let (mut unknown, mut since): (Option<usize>, Vec<usize>) = (None, Vec::new());
    for later in 0..n {
        let mut edges: Vec<(usize, u32)> = Vec::new();
        let one = &insns[later];
        let (reads_memory, writes_memory) = memory(one);
        let masks = effects[later].as_ref().map(|effect| (effect.all_reads().mask(), effect.writes.mask()));
        // What every instruction after one of unknown effect keeps behind it.
        edges.extend(unknown.map(|earlier| (earlier, 0)));
        for value in &one.uses {
            if let Some((Some(def), _)) = by_value.get(value) {
                edges.push((*def, def_latency(&insns[*def])));
            }
        }
        for value in &one.defines {
            if let Some((def, readers)) = by_value.get(value) {
                edges.extend(def.map(|earlier| (earlier, 1)));
                edges.extend(readers.iter().map(|&earlier| (earlier, 0)));
            }
        }
        match masks {
            Some((reads, writes)) => {
                for bit in bits_of(reads) {
                    if let Some(&def) = by_lane[bit].0.last() {
                        edges.push((def, def_latency(&insns[def])));
                    }
                }
                for bit in bits_of(writes) {
                    // No output edge between two dead defs (`addPhysRegDeps`).
                    let this_dead = dead[later] >> bit & 1 == 1;
                    edges.extend(
                        by_lane[bit]
                            .0
                            .iter()
                            .filter(|&&earlier| !(this_dead && dead[earlier] >> bit & 1 == 1))
                            .map(|&earlier| (earlier, 1)),
                    );
                    edges.extend(by_lane[bit].1.iter().map(|&earlier| (earlier, 0)));
                }
            }
            // An unknown effect is ordered with everything before it.
            None => edges.extend(since.iter().map(|&earlier| (earlier, 0))),
        }
        if reads_memory || writes_memory {
            edges.extend(store.map(|earlier| (earlier, 0)));
        }
        if writes_memory {
            edges.extend(loads.iter().map(|&earlier| (earlier, 0)));
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
                by_lane[bit].1.push(later);
            }
            for bit in bits_of(writes) {
                // A dead write leaves the defs and readers standing: a later
                // write still comes after the readers of the
                // live def before it.
                let entry = &mut by_lane[bit];
                if dead[later] >> bit & 1 == 0 {
                    entry.0.clear();
                    entry.1.clear();
                }
                entry.0.push(later);
            }
        } else {
            unknown = Some(later);
            since.clear();
            // Everything before it is behind it now.
            by_value.clear();
            by_lane.iter_mut().for_each(|entry| *entry = (Vec::new(), Vec::new()));
            store = None;
            loads.clear();
        }
        if writes_memory {
            (store, loads) = (Some(later), Vec::new());
        } else if reads_memory {
            loads.push(later);
        }
        since.push(later);
    }
    let mut succs: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (later, before) in preds.iter().enumerate() {
        for &earlier in before {
            succs[earlier].push(later);
        }
    }
    Graph { preds, succs, latencies }
}

/// `LLRM_MS_TRACE`: each pick with the reason `tracePick` of the pin names, for
/// a line-by-line reading against `llc -debug-only=machine-scheduler`.
fn trace() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LLRM_MS_TRACE").is_some())
}

/// What the tracker needs to follow a region's pressure bottom-up.
pub struct Tracking<'a> {
    pub model: &'a Model,
    /// The values live below the region that count (`closeBottom`).
    pub live_out: Vec<u32>,
    /// `initLiveThru`: the live-out values no untied def in the region makes.
    pub live_thru: Vec<u32>,
}

/// `tryCandidate` (MachineScheduler.cpp#L3965) of `GenericScheduler` for a
/// bottom-up region of a target with no schedule model: `RegExcess`,
/// `RegCritical`, `RegMax` where pressure is tracked, then `NodeOrder` (the
/// later one first). Left out: `tryBiasPhysRegs` (no physical-register copy is
/// in a region, MS4), stalls, clusters, weak edges and resources (no model),
/// and latency, which only `ReduceLatency` turns on and `pickNode` of a
/// bottom-up region never sets (`NoPolicy`).
fn try_candidate(
    try_node: usize,
    try_delta: &Delta,
    cand_node: usize,
    cand_delta: &Delta,
    tracked: bool,
) -> Option<&'static str> {
    if tracked {
        for (name, try_p, cand_p) in [
            ("REG-EXCESS", try_delta.excess, cand_delta.excess),
            ("REG-CRIT", try_delta.critical_max, cand_delta.critical_max),
            ("REG-MAX", try_delta.current_max, cand_delta.current_max),
        ] {
            match try_pressure(try_p, cand_p) {
                Pick::Try => return Some(name),
                Pick::Cand => return None,
                Pick::Neither => {}
            }
        }
    }
    (try_node > cand_node).then_some("ORDER")
}

/// The order of a region scheduled bottom-up as `GenericScheduler` does for
/// the i486 (MachineScheduler.cpp, #L3965): `X86.td` gives it `GenericModel`
/// (issue width 4, a micro-op buffer of 32, no instruction schedules), so
/// `SchedBoundary::releaseNode` never defers a node (`IsBuffered`, and one
/// micro-op never overfills a cycle) and the latency of an edge does not
/// order anything: an instruction is available once every reader is placed,
/// and [`try_candidate`] picks among those. With `tracking` (a region of more
/// than half the allocatable registers' worth of instructions, `initPolicy`
/// #L3688) pressure is followed per set.
///
/// Not translated (MS6): `Rem.IsAcyclicLatencyLimited`, which turns on the
/// height and depth heuristic in a loop whose in-flight micro-ops pass the
/// buffer (32).
pub fn bottom_up(
    insns: &[Arc<Insn>],
    graph: &Graph,
    tracking: Option<&Tracking>,
) -> Vec<usize> {
    let n = insns.len();
    let mut waiting: Vec<usize> = graph.succs.iter().map(Vec::len).collect();
    let mut available: Vec<usize> = (0..n).filter(|&node| waiting[node] == 0).collect();
    let mut placed = Vec::with_capacity(n);
    // `RegionPressure` (program order) and `RegionCriticalPSets`.
    let mut region_max: Vec<u32> = Vec::new();
    let mut critical: Vec<PressureChange> = Vec::new();
    let mut tracker = tracking.map(|tracking| {
        let mut program = Tracker::new(tracking.model, tracking.live_out.iter().copied(), []);
        for one in insns.iter().rev() {
            program.advance_upward(one);
        }
        region_max = program.max().to_vec();
        critical = Tracker::critical_sets(tracking.model, &region_max);
        Tracker::new(tracking.model, tracking.live_out.iter().copied(), tracking.live_thru.iter().copied())
    });
    while placed.len() < n {
        let mut best = 0;
        let mut why = "ONLY1";
        // `pickOnlyChoice` when only one is available.
        if available.len() > 1 {
            why = "FIRST";
            let deltas: Vec<Delta> = match &tracker {
                Some(tracker) => {
                    available.iter().map(|&node| tracker.upward_delta(&insns[node], &critical, &region_max)).collect()
                }
                None => vec![Delta::default(); available.len()],
            };
            for candidate in 1..available.len() {
                if let Some(reason) = try_candidate(
                    available[candidate],
                    &deltas[candidate],
                    available[best],
                    &deltas[best],
                    tracker.is_some(),
                ) {
                    best = candidate;
                    why = reason;
                }
            }
        }
        let node = available.swap_remove(best);
        if trace() {
            eprintln!("machinesched pick {node} {:?} {why} tracked={}", insns[node].defines, tracker.is_some());
        }
        if let Some(tracker) = &mut tracker {
            tracker.advance_upward(&insns[node]);
            // `updateScheduledPressure`.
            for entry in &mut critical {
                let most = tracker.max()[entry.set] as i32;
                if most > entry.inc {
                    entry.inc = most;
                }
            }
        }
        placed.push(node);
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

/// The pre-RA machine scheduler: every region of a block ordered by
/// [`bottom_up`]. DEVIATE MS4: regions end at more instructions than the
/// pin's. DEVIATE RP1/RP2 are `regpressure.rs`'s.
pub struct MachineScheduler {
    pub classes: Rc<RegisterClasses>,
    pub segments: Segments,
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
        Ok(scheduled(&body, registers.available.len(), |body, counted| {
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
        }))
    }
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

/// `body` with each region reordered; a region of more than `limit / 2`
/// instructions is tracked with the model `model` makes of the body (given
/// the test of whether a value counts).
pub fn scheduled(
    body: &LirBody,
    limit: usize,
    model: impl FnOnce(&LirBody, &dyn Fn(u32) -> bool) -> Model,
) -> LirBody {
    let (regs, bits) = (body.regs(), body.bits);
    // The x87 stack is its own register set with its own limit (DEVIATE MS3):
    // its values (those of the float operations) are not counted against
    // the general registers.
    let stack = x87_values(body.blocks.iter().flat_map(|block| block.insns.iter()));
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
            if regions.iter().any(|region| region.len() > limit / 2) {
                let live = live.get_or_insert_with(|| live_rows(body));
                let mut values: Vec<u32> =
                    insns.iter().flat_map(|one| one.defines.iter().chain(&one.uses)).copied().collect();
                values.sort_unstable();
                values.dedup();
                now = values.into_iter().filter(|&value| counted(value) && live.live_out(block.at, value)).collect();
                if let Some(make) = model.take() {
                    made = Some(make(body, &counted));
                }
            }
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
                if slice.len() >= 2 {
                    let tracking = made.as_ref().filter(|_| slice.len() > limit / 2).map(|model| {
                        let mut live_out: Vec<u32> = now.iter().copied().collect();
                        live_out.sort_unstable();
                        // `initLiveThru`: a live-out value counts unless a def
                        // in the region ties it (two-address).
                        let tied: HashSet<u32> = slice
                            .iter()
                            .filter(|one| one.what.as_ref().is_some_and(twoaddr::ties))
                            .flat_map(|one| one.defines.iter().copied())
                            .collect();
                        let live_thru = live_out.iter().copied().filter(|value| !tied.contains(value)).collect();
                        Tracking { model, live_out, live_thru }
                    });
                    let order = bottom_up(slice, &graph(slice, regs, bits), tracking.as_ref());
                    if order.iter().enumerate().any(|(place, &node)| place != node) {
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

    /// Three constants feed two adds and a store. In source order all three
    /// are live together (3 values); bottom-up with a limit of two places
    /// the third load just above the add that reads it, so at most two are
    /// live.
    #[test]
    fn test_bottom_up_places_a_load_by_its_reader_when_the_limit_is_two() {
        use super::bottom_up;
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
        let found = graph(&insns, regs, 16);
        let model = one_set(2);
        let tracking = super::Tracking { model: &model, live_out: vec![6], live_thru: vec![] };
        assert_eq!(bottom_up(&insns, &found, Some(&tracking)), [0, 1, 3, 2, 4, 5]);
        // Pressure not followed (a small region) leaves the order alone.
        assert_eq!(bottom_up(&insns, &found, None), [0, 1, 2, 3, 4, 5]);
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
        let defined: Vec<u32> =
            scheduled(&body, 2, |_, _| one_set(2)).blocks[0].insns.iter().flat_map(|one| one.defines.clone()).collect();
        assert_eq!(defined, [1, 2, 4, 3, 5, 6]);
        let kept: Vec<u32> = scheduled(&body, 16, |_, _| one_set(2)).blocks[0]
            .insns
            .iter()
            .flat_map(|one| one.defines.clone())
            .collect();
        assert_eq!(kept, [1, 2, 3, 4, 5, 6]);
    }

    /// A load was held back four cycles above its reader (the unbuffered
    /// default model's `getLoadLatency`), which hoisted the loads of a block
    /// to its top: x_dct +10 instructions. The i486's `GenericModel` is
    /// buffered, so a load is available as soon as its reader is placed and
    /// the order is the source's.
    #[test]
    fn test_a_load_stays_next_to_its_reader() {
        use super::bottom_up;
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let insns = vec![
            insn(0, semantics(Operation::Move, "mov", vec![held(1)], vec![imm(1)]), vec![1], vec![]),
            insn(1, semantics(Operation::Move, "mov", vec![held(2)], vec![imm(2)]), vec![2], vec![]),
            insn(2, semantics(Operation::Move, "mov", vec![held(3)], vec![cell(0)]), vec![3], vec![]),
            insn(3, semantics(Operation::Binary, "add", vec![held(4)], vec![held(3), imm(1)]), vec![4], vec![3]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO);
        let found = graph(&insns, regs, 16);
        assert_eq!(found.latencies[3], [4], "the reader still waits four cycles for the load");
        assert_eq!(bottom_up(&insns, &found, None), [0, 1, 2, 3]);
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
        let found = graph(&insns, regs, 32);
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
        let g = graph(&insns, regs, 32);
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
        let wide = |value: u32| Loc::Held(crate::model::ir::Held { value, width: 10 });
        let insns = vec![
            insn(0, semantics(Operation::Compare, "fcom", vec![], vec![wide(1), wide(2)]), vec![], vec![1, 2]),
            insn(1, semantics(Operation::Binary, "adc", vec![held(3)], vec![held(3), held(4)]), vec![3], vec![3, 4]),
        ];
        assert_eq!(graph(&insns, regs, 16).preds[1], [0], "the adc stays after the fcom");
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
        let float = |value: u32| Loc::Held(crate::model::ir::Held { value, width: 10 });
        let byte = Loc::Held(crate::model::ir::Held { value: 3, width: 1 });
        let insns = vec![
            insn(0, semantics(Operation::Compare, "fcom", vec![], vec![float(1), float(2)]), vec![], vec![1, 2]),
            insn(1, semantics(Operation::Unary, "seta", vec![byte], vec![]), vec![3], vec![]),
        ];
        let regs = llrm_lir::registers::Regs(&llrm_x86_m32::REGISTER_INFO);
        assert_eq!(graph(&insns, regs, 32).preds[1], [0], "the set stays after the fcom");
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
        let g = graph(&[compare.clone(), shift, carry], regs, 32);
        assert!(g.preds[2].contains(&1), "adc stays after the shl: {g:?}");
        let wide = |value: u32| Loc::Held(crate::model::ir::Held { value, width: 10 });
        let fcmov = insn(1, semantics(Operation::Move, "fcmove", vec![wide(7)], vec![wide(8)]), vec![7], vec![7, 8]);
        // Either its effect is unknown (a boundary) or it reads the flags the
        // compare set.
        let known = liveness::effect(regs, 32, &fcmov).map(|effect| !effect.all_reads().is_disjoint(&flags));
        assert!(known.is_none_or(|reads| reads), "an fcmov with a known effect reads the flags");
        let g = graph(&[compare, fcmov], regs, 32);
        assert_eq!(g.preds[1], [0], "the fcmov stays after the cmp: {g:?}");
    }

    #[test]
    fn test_the_scheduler_sets_are_not_siphashed() {
        // std's `HashSet<u32>` hashed every pressure query with SipHash.
        let none: [Arc<Insn>; 0] = [];
        let _: crate::support::hash::HashSet<u32> = super::x87_values(none.iter());
    }
}
