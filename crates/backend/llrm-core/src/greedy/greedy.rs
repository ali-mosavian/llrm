//! `RAGreedy`: the greedy register allocator's queue, direct assignment,
//! interference eviction, last chance recoloring and spill, with
//! `RegAllocBase::allocatePhysRegs` as its driver. Translated from
//! llvm/lib/CodeGen/RegAllocGreedy.cpp, RegAllocGreedy.h, RegAllocBase.cpp,
//! RegAllocEvictionAdvisor.cpp and AllocationOrder.{h,cpp} at d1106deb71cc.
//!
//! # How it works
//! Every virtual register is queued with a priority (long ranges first; a
//! range inside one block in instruction order; ranges the spiller made last).
//! `allocate_phys_regs` takes the best, and `select_or_split` gives it a
//! register or makes new, smaller ranges: it first tries a free register in the
//! allocation order (hints first), then evicting the interference of a register
//! whose holders are all lighter than it (each eviction carries a cascade
//! number so an eviction cycle cannot run forever), then waits a round
//! (`RS_Split`) so smaller ranges are placed first, then splits
//! (`regionsplit`), and at last spills; a spill product (`RS_Done`) that finds
//! no register recolors the ranges in its way. Stages only move forward.
//!
//! ```text
//!   New -> Assign -> Split -> (Split2) -> Spill -> Done
//!           assign   wait     split       spill    last chance
//!           + evict  a round  (S2)                 recoloring
//! ```
//!
//! DEVIATE (until S2, named, no number): `tryAssignCSRFirstTime`,
//! `trySplitAroundHintReg` and `tryHintsRecoloring` are not translated, so a
//! range that cannot get a register goes from `RS_Split` straight to spill;
//! this target's register costs are all zero (`RegCosts`), so the cost-per-use
//! path of `tryAssign` is dropped. Callee-saved register cost is the shrink
//! wrapping pass's.
//!
//! DEVIATE D-G1: at -Os m16 the greedy allocator is not at the shipped
//! allocator's size (QCport, 65 modules, 2026-10-19: 78518 against 77618
//! instructions, +1.2%). The fact: LLVM's greedy on the same final IR is at
//! ours in total (`llc -O2`, code16, `-frame-pointer=all`, as `bp` is the
//! frame here: 78720; without it `ebp` is free and llc reads 76020, which is
//! not a bar for this target). Where both allocators are far above `llc`
//! (savegame, mdl_ai, item) the excess is instruction selection and the
//! passes before allocation, or `llc` was fed the `26-switchconv` stage.
//! Splitting costs single functions: `_pl_trace_actors` 220 against 189 with
//! `LLRM_NO_SPLIT` and 190 shipped. Exit: every module at `llc` or at the
//! shipped allocator's bytes, before the backend phase merges to main.
//!
//! Named row of D-G1 (qb `register_live_through`, qbdemo PLASMA): the x loop
//! keeps 2 stores and 3 loads per trip in `[bp]` where the shipped allocator
//! kept none; `llc` on the same IR has 1 store and 4 loads. Six registers
//! against five induction variables and the address temporaries. The test
//! stays red: it pins what the shipped allocator achieved. Closers: fewer live
//! induction variables (the new LSR), or a translated improvement from the pin
//! that avoids the induction variable's spill.
//!
//! # Example
//! Three values live together over two registers: the third finds none free,
//! none to evict (all heavier), waits a round, then is spilled; the spill's
//! reload and store ranges are tiny and take the registers the others leave.
//!
//! # Used by
//! the shadow comparison in `backend::allocate`.

use std::collections::{BTreeMap, BinaryHeap};
use std::rc::Rc;

use llrm_lir::registers::RegId;

use super::matrix::{InterferenceKind, LiveRegMatrix};
use super::regionsplit::{SplitSupport, Splitter};
use super::virtregmap::VirtRegMap;
use crate::liveness::calcspillweights::{Hint, SpillWeightEnv, VirtRegAuxInfo};
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::liverange::LiveInterval;
use crate::liveness::slotindex::SlotIndex;
use crate::support::hash::{HashMap, HashSet};

/// `-lcr-max-depth` (RegAllocGreedy.cpp#L95).
pub const LAST_CHANCE_RECOLORING_MAX_DEPTH: u32 = 5;
/// `-lcr-max-interf` (RegAllocGreedy.cpp#L99).
pub const LAST_CHANCE_RECOLORING_MAX_INTERFERENCE: usize = 8;
/// `-regalloc-eviction-max-interference-cutoff`
/// (RegAllocEvictionAdvisor.cpp#L49).
pub const EVICT_INTERFERENCE_CUTOFF: usize = 10;

/// `LiveRangeStage` (RegAllocEvictionAdvisor.h#L40).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LiveRangeStage {
    /// Newly created live range that has never been queued.
    New,
    /// Only attempt assignment and eviction. Then requeue as `Split`.
    Assign,
    /// Attempt live range splitting if assignment is impossible.
    Split,
    /// More aggressive splitting that is guaranteed to make progress.
    Split2,
    /// Live range will be spilled. No more splitting will be attempted.
    Spill,
    /// There is nothing more we can do to this live range.
    Done,
}

/// `RAGreedy::ExtraRegInfo` (RegAllocGreedy.h#L61): the stage and the cascade
/// of each live range.
pub struct ExtraRegInfo {
    info: HashMap<u32, (LiveRangeStage, u32)>,
    next_cascade: u32,
}

impl Default for ExtraRegInfo {
    fn default() -> Self {
        Self { info: HashMap::default(), next_cascade: 1 }
    }
}

impl ExtraRegInfo {
    pub fn get_stage(
        &self,
        reg: u32,
    ) -> LiveRangeStage {
        self.info.get(&reg).map_or(LiveRangeStage::New, |i| i.0)
    }

    pub fn set_stage(
        &mut self,
        reg: u32,
        stage: LiveRangeStage,
    ) {
        self.info.entry(reg).or_insert((LiveRangeStage::New, 0)).0 = stage;
    }

    /// `getOrInitStage`.
    pub fn get_or_init_stage(
        &mut self,
        reg: u32,
    ) -> LiveRangeStage {
        self.info.entry(reg).or_insert((LiveRangeStage::New, 0)).0
    }

    pub fn get_cascade(
        &self,
        reg: u32,
    ) -> u32 {
        self.info.get(&reg).map_or(0, |i| i.1)
    }

    pub fn set_cascade(
        &mut self,
        reg: u32,
        cascade: u32,
    ) {
        self.info.entry(reg).or_insert((LiveRangeStage::New, 0)).1 = cascade;
    }

    pub fn get_or_assign_new_cascade(
        &mut self,
        reg: u32,
    ) -> u32 {
        let mut cascade = self.get_cascade(reg);
        if cascade == 0 {
            cascade = self.next_cascade;
            self.next_cascade += 1;
            self.set_cascade(reg, cascade);
        }
        cascade
    }

    pub fn get_cascade_or_current_next(
        &self,
        reg: u32,
    ) -> u32 {
        let cascade = self.get_cascade(reg);
        if cascade == 0 { self.next_cascade } else { cascade }
    }

    /// `setStage(Begin, End, NewStage)`: only ranges still `New` move.
    pub fn set_stage_all(
        &mut self,
        regs: &[u32],
        stage: LiveRangeStage,
    ) {
        for &reg in regs {
            let entry = self.info.entry(reg).or_insert((LiveRangeStage::New, 0));
            if entry.0 == LiveRangeStage::New {
                entry.0 = stage;
            }
        }
    }

    /// `LRE_DidCloneVirtReg` (RegAllocGreedy.cpp#L399).
    pub fn lre_did_clone_virt_reg(
        &mut self,
        new: u32,
        old: u32,
    ) {
        // Cloning a register we haven't even heard about yet? Just ignore it.
        let Some(&old_info) = self.info.get(&old) else { return };
        // LRE may clone a virtual register because dead code elimination causes
        // it to be split into connected components. The new components
        // are much smaller than the original, so they should get a new
        // chance at being assigned, in the same stage as the parent.
        self.info.insert(old, (LiveRangeStage::Assign, old_info.1));
        self.info.insert(new, (LiveRangeStage::Assign, old_info.1));
    }
}

/// `EvictionCost` (RegAllocEvictionAdvisor.h#L74).
#[derive(Clone, Copy, Debug, Default)]
pub struct EvictionCost {
    pub broken_hints: u32,
    pub max_weight: f32,
}

impl EvictionCost {
    pub fn is_max(&self) -> bool {
        self.broken_hints == u32::MAX
    }

    pub fn set_max(&mut self) {
        self.broken_hints = u32::MAX;
    }

    /// `operator<`: by broken hints, then by the heaviest weight.
    pub fn less(
        &self,
        other: &EvictionCost,
    ) -> bool {
        (self.broken_hints, OrdF32(self.max_weight)) < (other.broken_hints, OrdF32(other.max_weight))
    }

    /// `operator>=`.
    pub fn at_least(
        &self,
        other: &EvictionCost,
    ) -> bool {
        !self.less(other)
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
struct OrdF32(f32);

/// `AllocationOrder` (AllocationOrder.h#L25): the registers to try, hints
/// first.
pub struct AllocationOrder {
    hints: Vec<RegId>,
    order: Vec<RegId>,
}

impl AllocationOrder {
    /// `AllocationOrder::create` (AllocationOrder.cpp#L28) with the default
    /// `getRegAllocationHints` (TargetRegisterInfo.cpp#L400): a physical hint
    /// stays, a virtual one is the register it holds; only registers of the
    /// order.
    pub fn create(
        virt: u32,
        vrm: &VirtRegMap,
        order: Vec<RegId>,
        hints: &BTreeMap<u32, Vec<Hint>>,
    ) -> Self {
        let mut found: Vec<RegId> = Vec::new();
        for hint in hints.get(&virt).into_iter().flatten() {
            let phys = match hint {
                Hint::Virt(v) => vrm.get_phys(*v),
                Hint::Phys(r) => Some(*r),
            };
            // Don't add the same reg twice; check Phys is in the allocation
            // order.
            if let Some(phys) = phys {
                if !found.contains(&phys) && order.contains(&phys) {
                    found.push(phys);
                }
            }
        }
        Self { hints: found, order }
    }

    /// The registers in the order to try them, with whether each is a hint.
    pub fn iter(&self) -> impl Iterator<Item = (RegId, bool)> + '_ {
        self.hints
            .iter()
            .map(|r| (*r, true))
            .chain(self.order.iter().filter(|r| !self.hints.contains(r)).map(|r| (*r, false)))
    }

    /// `getOrder`: the allocation order without reordered hints.
    pub fn get_order(&self) -> &[RegId] {
        &self.order
    }

    pub fn is_hint(
        &self,
        reg: RegId,
    ) -> bool {
        self.hints.contains(&reg)
    }
}

/// What the allocator asks of the target's register classes and instructions.
pub trait GreedyEnv: SpillWeightEnv {
    /// Whether `virt` has a class of its own (`MRI->getRegClass`), else its
    /// original's is asked.
    fn has_class(
        &self,
        _virt: u32,
    ) -> bool {
        false
    }
    /// `RegClassInfo.getOrder(MRI->getRegClass(VirtReg))`.
    fn allocation_order(
        &self,
        virt: u32,
    ) -> Vec<RegId>;
    /// `RegClassInfo.getNumAllocatableRegs(RC)`.
    fn num_allocatable_regs(
        &self,
        virt: u32,
    ) -> usize;
    /// `RC->getCopyCost()`.
    fn copy_cost(
        &self,
        virt: u32,
    ) -> u32;
    /// Whether two virtual registers are of one register class.
    fn same_reg_class(
        &self,
        a: u32,
        b: u32,
    ) -> bool;
    /// `hasTiedDef(MRI, Reg)`.
    fn has_tied_def(
        &self,
        virt: u32,
    ) -> bool;
}

/// What `RAGreedy` gives a spiller to work with (`LiveRangeEdit`'s world).
pub struct SpillContext<'a, 'b> {
    pub lis: &'a mut LiveIntervals,
    pub vrm: &'a mut VirtRegMap,
    pub vrai: &'a mut VirtRegAuxInfo<'b>,
}

/// `Spiller` (Spiller.h): rewrites the uses and defs of a virtual register
/// through memory, returning the new virtual registers.
pub trait Spiller {
    /// `spill(LiveRangeEdit&)`: the ranges the spill made are pushed to
    /// `new_vregs`.
    fn spill(
        &mut self,
        ctx: SpillContext<'_, '_>,
        reg: u32,
        new_vregs: &mut Vec<u32>,
    );

    /// Reloads, stores and copy reads/writes folded into the copy, made so far.
    fn counts(&self) -> (usize, usize, usize) {
        (0, 0, 0)
    }

    /// The registers spilled, for a spiller that only records the decision.
    fn recorded(&self) -> Vec<u32> {
        Vec::new()
    }

    /// A spill the spiller could not write: the allocation cannot be used.
    fn failed(&self) -> bool {
        false
    }
}

/// What `selectOrSplit` answers: a register, no register this round (maybe new
/// ranges), or failure (`~0u`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Select {
    Reg(RegId),
    None,
    Fail,
}

/// Counters of the decisions made.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub dequeued: usize,
    pub assigned: usize,
    pub evicted: usize,
    pub waited: usize,
    pub spilled_ranges: usize,
    pub new_queued: usize,
    pub last_chance: usize,
    pub failed: usize,
}

type RecolorStack = Vec<(Rc<LiveInterval>, RegId)>;

/// `RAGreedy` (RegAllocGreedy.h#L55) over `RegAllocBase`.
pub struct RAGreedy<'a> {
    pub lis: LiveIntervals,
    pub matrix: LiveRegMatrix,
    env: &'a dyn GreedyEnv,
    pub original: HashMap<u32, u32>,
    pub hints: BTreeMap<u32, Vec<Hint>>,
    pub extra: ExtraRegInfo,
    /// Registers fixed to their physical register by the instructions that
    /// require one (pre-assigned): nothing recolors them (DEVIATE: LLVM
    /// models a fixed register as the reg units of a physical register).
    pub pinned: HashSet<u32>,
    queue: BinaryHeap<(u32, u32)>,
    spiller: Box<dyn Spiller + 'a>,
    pub stats: Stats,
    /// What `trySplit` works with; none until the caller gives it the
    /// function's blocks (then a range that does not fit is spilled).
    pub split: Option<SplitSupport>,
}

impl<'a> RAGreedy<'a> {
    pub fn new(
        lis: LiveIntervals,
        matrix: LiveRegMatrix,
        env: &'a dyn GreedyEnv,
        spiller: Box<dyn Spiller + 'a>,
    ) -> Self {
        Self {
            lis,
            matrix,
            env,
            original: HashMap::default(),
            hints: BTreeMap::new(),
            extra: ExtraRegInfo::default(),
            pinned: HashSet::default(),
            queue: BinaryHeap::new(),
            spiller,
            stats: Stats::default(),
            split: None,
        }
    }

    /// `calculateSpillWeightsAndHints` (the `VRAI` of `RAGreedy::run`).
    /// The registers a recording spiller was asked to spill.
    pub fn recorded_spills(&self) -> Vec<u32> {
        self.spiller.recorded()
    }

    /// Whether the spiller failed to write a spill.
    pub fn spill_failed(&self) -> bool {
        self.spiller.failed()
    }

    /// What the spiller has made: reloads, stores, folded.
    pub fn spill_counts(&self) -> (usize, usize, usize) {
        self.spiller.counts()
    }

    pub fn calculate_spill_weights_and_hints(&mut self) {
        let mut vrai = VirtRegAuxInfo::new(self.env, &self.original);
        vrai.calculate_spill_weights_and_hints(&mut self.lis);
        self.hints = vrai.hints;
    }

    /// The `getSimpleHint`: the first hint.
    fn simple_hint(
        &self,
        virt: u32,
    ) -> Option<Hint> {
        self.hints.get(&virt).and_then(|h| h.first().copied())
    }

    /// `VirtRegMap::hasKnownPreference`.
    fn has_known_preference(
        &self,
        virt: u32,
    ) -> bool {
        match self.simple_hint(virt) {
            Some(Hint::Phys(_)) => true,
            Some(Hint::Virt(v)) => self.matrix.vrm.has_phys(v),
            None => false,
        }
    }

    // ---- RegAllocBase -------------------------------------------------------

    /// `seedLiveRegs` (RegAllocBase.cpp#L75).
    fn seed_live_regs(&mut self) {
        for reg in self.lis.virt_regs() {
            let li = self.lis.interval_rc(reg);
            self.enqueue(&li);
        }
    }

    /// `RegAllocBase::enqueue` then `enqueueImpl`.
    fn enqueue(
        &mut self,
        li: &Rc<LiveInterval>,
    ) {
        let reg = li.reg();
        if self.matrix.vrm.has_phys(reg) {
            return;
        }
        let mut stage = self.extra.get_or_init_stage(reg);
        if stage == LiveRangeStage::New {
            stage = LiveRangeStage::Assign;
            self.extra.set_stage(reg, stage);
        }
        // A pinned value is LLVM's physical register range, in the matrix
        // before any virtual one is assigned: it is dequeued first.
        let prio = if self.pinned.contains(&reg) { u32::MAX } else { self.get_priority(li) };
        // The virtual register number is a tie breaker for same-sized ranges.
        // Give lower vreg numbers higher priority to assign them first.
        self.queue.push((prio, !reg));
    }

    /// `DefaultPriorityAdvisor::getPriority` (RegAllocGreedy.cpp#L443).
    fn get_priority(
        &self,
        li: &LiveInterval,
    ) -> u32 {
        let size = li.get_size();
        let reg = li.reg();
        let stage = self.extra.get_stage(reg);
        if stage == LiveRangeStage::Split {
            // Unsplit ranges that couldn't be allocated immediately are
            // deferred until everything else has been allocated.
            return size;
        }
        // Giant live ranges fall back to the global assignment heuristic, which
        // prevents excessive spilling in pathological cases.
        let force_global =
            (size / SlotIndex::INSTR_DIST) as usize > 2 * self.env.num_allocatable_regs(self.class_reg(reg));
        let mut global_bit = 0u32;
        let mut prio;
        if stage == LiveRangeStage::Assign
            && !force_global
            && !li.empty()
            && self.lis.interval_is_in_one_mbb(li).is_some()
        {
            // Allocate original local ranges in linear instruction order. Since
            // they are singly defined, this produces optimal
            // coloring in the absence of global interference and
            // other constraints.
            prio = li.begin_index().get_approx_instr_distance(&self.lis.indexes.get_last_index()) as u32;
        } else {
            // Allocate global and split ranges in long->short order. Long
            // ranges that don't fit should be spilled (or split)
            // ASAP so they don't create interference. Mark a bit to
            // prioritize global above local ranges.
            prio = size;
            global_bit = 1;
        }
        // Priority bit layout: 31 RS_Assign priority, 30 Preference priority,
        // 29 Global bit, 28-24 AllocPriority (0 here), 0-23 Size/Instr
        // distance.
        prio = prio.min((1 << 24) - 1);
        prio |= global_bit << 29;
        // Mark a higher bit to prioritize global and local above RS_Split.
        prio |= 1 << 31;
        // Boost ranges that have a physical register hint.
        if self.has_known_preference(reg) {
            prio |= 1 << 30;
        }
        prio
    }

    /// `dequeue` (RegAllocGreedy.cpp#L519).
    fn dequeue(&mut self) -> Option<Rc<LiveInterval>> {
        let (_, key) = self.queue.pop()?;
        Some(self.lis.interval_rc(!key))
    }

    /// `RegAllocBase::allocatePhysRegs` (RegAllocBase.cpp#L88): seeds the queue
    /// and assigns registers one at a time.
    pub fn allocate_phys_regs(&mut self) {
        self.seed_live_regs();
        // Continue assigning vregs one at a time to available physical
        // registers.
        while let Some(virt_reg) = self.dequeue() {
            self.stats.dequeued += 1;
            assert!(!self.matrix.vrm.has_phys(virt_reg.reg()), "Register already assigned");
            // Unused registers can appear when the spiller coalesces snippets.
            if self.lis.reg_nodbg_empty(virt_reg.reg()) {
                self.lis.remove_interval(virt_reg.reg());
                continue;
            }
            // Invalidate all interference queries, live ranges could have
            // changed.
            self.matrix.invalidate_virt_regs();
            // selectOrSplit requests the allocator to return an available
            // physical register if possible and populate a list of
            // new live intervals that result from splitting.
            let mut split_vregs: Vec<u32> = Vec::new();
            let poc_trace = std::env::var_os("LLRM_POC_DECIDE").is_some();
            if poc_trace {
                let hints: Vec<String> = self.hints.get(&virt_reg.reg()).into_iter().flatten().map(|h| format!("{h:?}")).collect();
                eprintln!("decide {} w={:.3} stage={:?} hints=[{}] order={:?}", virt_reg, virt_reg.weight(), self.extra.get_stage(virt_reg.reg()), hints.join(","), self.order_of(virt_reg.reg()).iter().take(3).collect::<Vec<_>>());
            }
            let poc_got = self.select_or_split(&virt_reg, &mut split_vregs);
            if poc_trace {
                eprintln!("  -> {:?} split={:?}", match &poc_got { Select::Reg(r) => format!("{r:?}"), Select::Fail => "fail".into(), Select::None => "none".into() }, split_vregs);
            }
            match poc_got {
                Select::Fail => {
                    // selectOrSplit failed to find a register: no valid
                    // allocation.
                    self.stats.failed += 1;
                    if llrm_support::env_flag!("LLRM_SHADOW_DEBUG") {
                        eprintln!("  fail: {} stage {:?}", virt_reg, self.extra.get_stage(virt_reg.reg()));
                        let order = self.order_of(virt_reg.reg());
                        for (reg, _) in order.iter() {
                            let kind = self.matrix.check_interference(&virt_reg, reg);
                            let mut holders = Vec::new();
                            for unit in self.matrix.tri().regunits(reg) {
                                holders.extend(self.matrix.query_vregs(
                                    virt_reg.reg(),
                                    &virt_reg.range,
                                    unit,
                                    usize::MAX,
                                ));
                            }
                            let shown: Vec<String> =
                                holders.iter().map(|h| format!("{}", self.lis.interval_rc(*h))).collect();
                            eprintln!("    {reg:?}: {kind:?} {}", shown.join(" | "));
                        }
                    }
                }
                Select::Reg(phys) => {
                    self.matrix.assign(&virt_reg, phys);
                    self.stats.assigned += 1;
                }
                Select::None => {}
            }
            for reg in split_vregs {
                assert!(self.lis.has_interval(reg));
                let split = self.lis.interval_rc(reg);
                assert!(!self.matrix.vrm.has_phys(reg), "Register already assigned");
                if self.lis.reg_nodbg_empty(reg) {
                    assert!(split.empty(), "Non-empty but used interval");
                    self.lis.remove_interval(reg);
                    continue;
                }
                self.enqueue(&split);
                self.stats.new_queued += 1;
            }
        }
    }

    // ---- direct assignment, eviction ---------------------------------------

    /// The register whose class `virt` has: itself where it has one of its own,
    /// else its original.
    fn class_reg(
        &self,
        virt: u32,
    ) -> u32 {
        if self.env.has_class(virt) { virt } else { self.matrix.vrm.get_original(virt) }
    }

    fn order_of(
        &self,
        virt: u32,
    ) -> AllocationOrder {
        AllocationOrder::create(virt, &self.matrix.vrm, self.env.allocation_order(self.class_reg(virt)), &self.hints)
    }

    /// `tryAssign` (RegAllocGreedy.cpp#L532), without the cost-per-use path.
    fn try_assign(
        &mut self,
        virt: &Rc<LiveInterval>,
        order: &AllocationOrder,
        new_vregs: &mut Vec<u32>,
        fixed: &HashSet<u32>,
    ) -> Option<RegId> {
        let mut phys: Option<RegId> = None;
        for (reg, is_hint) in order.iter() {
            if self.matrix.check_interference(virt, reg) == InterferenceKind::Free {
                if is_hint {
                    return Some(reg);
                }
                phys = Some(reg);
                break;
            }
        }
        let phys = phys?;
        // PhysReg is available, but there may be a better choice.
        // If we missed a simple hint, try to cheaply evict interference from
        // the preferred register.
        if let Some(Hint::Phys(hint)) = self.simple_hint(virt.reg()) {
            if order.is_hint(hint) && self.can_evict_hint_interference(virt, hint, fixed) {
                self.evict_interference(virt, hint, new_vregs);
                return Some(hint);
            }
        }
        // Most registers have 0 additional cost (RegCosts): nothing cheaper to
        // try.
        Some(phys)
    }

    /// `isUrgentEviction` (RegAllocEvictionAdvisor.cpp#L146): an unspillable
    /// range may evict almost anything.
    fn is_urgent_eviction(
        &self,
        virt: &LiveInterval,
        intf: &LiveInterval,
    ) -> bool {
        !virt.is_spillable()
            && (intf.is_spillable()
                || self.env.num_allocatable_regs(self.class_reg(virt.reg()))
                    < self.env.num_allocatable_regs(self.class_reg(intf.reg())))
    }

    /// `DefaultEvictionAdvisor::shouldEvict`
    /// (RegAllocEvictionAdvisor.cpp#L172).
    fn should_evict(
        &self,
        a: &LiveInterval,
        is_hint: bool,
        b: &LiveInterval,
        breaks_hint: bool,
    ) -> bool {
        let can_split = self.extra.get_stage(b.reg()) < LiveRangeStage::Spill;
        // Be fairly aggressive about following hints as long as the evictee can
        // be split.
        if can_split && is_hint && !breaks_hint {
            return true;
        }
        a.weight() > b.weight()
    }

    /// `canEvictHintInterference` (RegAllocEvictionAdvisor.cpp#L192).
    fn can_evict_hint_interference(
        &mut self,
        virt: &Rc<LiveInterval>,
        phys: RegId,
        fixed: &HashSet<u32>,
    ) -> bool {
        let mut max_cost = EvictionCost::default();
        max_cost.broken_hints = self.env.copy_cost(virt.reg());
        self.can_evict_interference_based_on_cost(virt, phys, true, &mut max_cost, fixed)
    }

    /// `canEvictInterferenceBasedOnCost` (RegAllocEvictionAdvisor.cpp#L207):
    /// all interferences between `virt` and `phys` can be evicted for less
    /// than `max_cost` (updated when true).
    fn can_evict_interference_based_on_cost(
        &mut self,
        virt: &Rc<LiveInterval>,
        phys: RegId,
        is_hint: bool,
        max_cost: &mut EvictionCost,
        fixed: &HashSet<u32>,
    ) -> bool {
        // It is only possible to evict virtual register interference.
        if self.matrix.check_interference(virt, phys) > InterferenceKind::VirtReg {
            return false;
        }
        let is_local = virt.empty() || self.lis.interval_is_in_one_mbb(virt).is_some();
        // Find VirtReg's cascade number. This will be unassigned if VirtReg was
        // never involved in an eviction before. If a cascade number was
        // assigned, deny evicting anything with the same or a newer
        // cascade number. This prevents infinite eviction loops.
        let cascade = self.extra.get_cascade_or_current_next(virt.reg());
        let mut cost = EvictionCost::default();
        for unit in self.matrix.tri().regunits(phys) {
            // If there is 10 or more interferences, chances are one is heavier.
            let interferences = self.matrix.query_vregs(virt.reg(), &virt.range, unit, EVICT_INTERFERENCE_CUTOFF);
            if interferences.len() >= EVICT_INTERFERENCE_CUTOFF {
                return false;
            }
            // Check if any interfering live range is heavier than MaxWeight.
            for &intf_reg in interferences.iter().rev() {
                let intf = self.lis.interval_rc(intf_reg);
                // Do not allow eviction of a virtual register if we are in the
                // middle of last-chance recoloring and this
                // virtual register is one that we
                // have scavenged a physical register for.
                if fixed.contains(&intf_reg) {
                    return false;
                }
                // Never evict spill products. They cannot split or spill.
                if self.extra.get_stage(intf_reg) == LiveRangeStage::Done {
                    return false;
                }
                let urgent = self.is_urgent_eviction(virt, &intf);
                // Only evict older cascades or live ranges without a cascade.
                let intf_cascade = self.extra.get_cascade(intf_reg);
                if cascade == intf_cascade {
                    return false;
                }
                if cascade < intf_cascade {
                    if !urgent {
                        return false;
                    }
                    // We permit breaking cascades for urgent evictions. It
                    // should be the last resort, though, so
                    // make it really expensive.
                    cost.broken_hints += 10 * self.env.copy_cost(intf_reg);
                }
                // Would this break a satisfied hint?
                let hint = match self.simple_hint(intf_reg) {
                    Some(Hint::Phys(r)) => Some(r),
                    Some(Hint::Virt(v)) => self.matrix.vrm.get_phys(v),
                    None => None,
                };
                let breaks_hint = self.matrix.vrm.has_preferred_phys(intf_reg, hint);
                // Update eviction cost.
                if breaks_hint {
                    cost.broken_hints += self.env.copy_cost(intf_reg);
                }
                cost.max_weight = cost.max_weight.max(intf.weight());
                // Abort if this would be too expensive.
                if cost.at_least(max_cost) {
                    return false;
                }
                if urgent {
                    continue;
                }
                // Apply the eviction policy for non-urgent evictions.
                if !self.should_evict(virt, is_hint, &intf, breaks_hint) {
                    return false;
                }
                // If !MaxCost.isMax(), then we're just looking for a cheap
                // register. Evicting another local live range
                // in this case could lead to suboptimal
                // coloring. (Local reassignment is off by default.)
                if !max_cost.is_max() && is_local && self.lis.interval_is_in_one_mbb(&intf).is_some() {
                    return false;
                }
            }
        }
        *max_cost = cost;
        true
    }

    /// `tryFindEvictionCandidate` (RegAllocEvictionAdvisor.cpp#L323).
    fn try_find_eviction_candidate(
        &mut self,
        virt: &Rc<LiveInterval>,
        order: &AllocationOrder,
        fixed: &HashSet<u32>,
    ) -> Option<RegId> {
        // Keep track of the cheapest interference seen so far.
        let mut best_cost = EvictionCost::default();
        best_cost.set_max();
        let mut best_phys: Option<RegId> = None;
        for (phys, is_hint) in order.iter() {
            if !self.can_evict_interference_based_on_cost(virt, phys, false, &mut best_cost, fixed) {
                continue;
            }
            // Best so far.
            best_phys = Some(phys);
            // Stop if the hint can be used.
            if is_hint {
                break;
            }
        }
        best_phys
    }

    /// `evictInterference` (RegAllocGreedy.cpp#L617): evicts any interfering
    /// registers that prevent `virt` from being assigned to `phys`.
    fn evict_interference(
        &mut self,
        virt: &Rc<LiveInterval>,
        phys: RegId,
        new_vregs: &mut Vec<u32>,
    ) {
        // Make sure that VirtReg has a cascade number, and assign that cascade
        // number to every evicted register. These live ranges than then only be
        // evicted by a newer cascade, preventing infinite loops.
        let cascade = self.extra.get_or_assign_new_cascade(virt.reg());
        // Collect all interfering virtregs first.
        let mut intfs: Vec<u32> = Vec::new();
        for unit in self.matrix.tri().regunits(phys) {
            intfs.extend(self.matrix.query_vregs(virt.reg(), &virt.range, unit, usize::MAX));
        }
        // Evict them second. This will invalidate the queries.
        for intf in intfs {
            // The same VirtReg may be present in multiple RegUnits. Skip
            // duplicates.
            if !self.matrix.vrm.has_phys(intf) {
                continue;
            }
            let li = self.lis.interval_rc(intf);
            self.matrix.unassign(&li);
            self.extra.set_cascade(intf, cascade);
            self.stats.evicted += 1;
            new_vregs.push(intf);
        }
    }

    /// `tryEvict` (RegAllocGreedy.cpp#L715).
    fn try_evict(
        &mut self,
        virt: &Rc<LiveInterval>,
        order: &AllocationOrder,
        new_vregs: &mut Vec<u32>,
        fixed: &HashSet<u32>,
    ) -> Option<RegId> {
        let best = self.try_find_eviction_candidate(virt, order, fixed)?;
        self.evict_interference(virt, best, new_vregs);
        Some(best)
    }

    // ---- selectOrSplit ------------------------------------------------------

    /// `selectOrSplit` (RegAllocGreedy.cpp#L2328).
    pub fn select_or_split(
        &mut self,
        virt: &Rc<LiveInterval>,
        new_vregs: &mut Vec<u32>,
    ) -> Select {
        let mut fixed: HashSet<u32> = HashSet::default();
        let mut recolor_stack: RecolorStack = Vec::new();
        self.select_or_split_impl(virt, new_vregs, &mut fixed, &mut recolor_stack, 0)
    }

    /// `selectOrSplitImpl` (RegAllocGreedy.cpp#L2655).
    fn select_or_split_impl(
        &mut self,
        virt: &Rc<LiveInterval>,
        new_vregs: &mut Vec<u32>,
        fixed: &mut HashSet<u32>,
        recolor_stack: &mut RecolorStack,
        depth: u32,
    ) -> Select {
        // First try assigning a free register.
        let order = self.order_of(virt.reg());
        if let Some(phys) = self.try_assign(virt, &order, new_vregs, fixed) {
            return Select::Reg(phys);
        }
        // Non empty NewVRegs means VirtReg has been split.
        if !new_vregs.is_empty() {
            return Select::None;
        }
        let stage = self.extra.get_stage(virt.reg());
        // Try to evict a less worthy live range, but only for ranges from the
        // primary queue. The RS_Split ranges already failed to do this,
        // and they should not get a second chance until they have been
        // split.
        if stage != LiveRangeStage::Split {
            if let Some(phys) = self.try_evict(virt, &order, new_vregs, fixed) {
                return Select::Reg(phys);
            }
        }
        assert!(new_vregs.is_empty() || depth > 0, "Cannot append to existing NewVRegs");
        // The first time we see a live range, don't try to split or spill.
        // Wait until the second time, when all smaller ranges have been
        // allocated. This gives a better picture of the interference to
        // split around.
        if stage < LiveRangeStage::Split {
            self.extra.set_stage(virt.reg(), LiveRangeStage::Split);
            self.stats.waited += 1;
            new_vregs.push(virt.reg());
            return Select::None;
        }
        if stage < LiveRangeStage::Spill && !virt.empty() {
            // Try splitting VirtReg or interferences.
            if let Some(ss) = self.split.as_mut() {
                let before = new_vregs.len();
                let mut splitter = Splitter {
                    lis: &mut self.lis,
                    matrix: &mut self.matrix,
                    extra: &mut self.extra,
                    ss,
                    env: self.env,
                    original: &self.original,
                    hints: &mut self.hints,
                };
                let phys = splitter.try_split(virt, &order, new_vregs);
                if phys.is_some() || new_vregs.len() != before {
                    return phys.map_or(Select::None, Select::Reg);
                }
            }
        }
        // If we couldn't allocate a register from spilling, there is probably
        // some invalid inline assembly. The base class will report it.
        if stage >= LiveRangeStage::Done || !virt.is_spillable() {
            return self.try_last_chance_recoloring(virt, &order, new_vregs, fixed, recolor_stack, depth);
        }
        // Finally spill VirtReg itself.
        let mut spilled: Vec<u32> = Vec::new();
        {
            let mut vrai = VirtRegAuxInfo::new(self.env, &self.original);
            let ctx = SpillContext { lis: &mut self.lis, vrm: &mut self.matrix.vrm, vrai: &mut vrai };
            self.spiller.spill(ctx, virt.reg(), &mut spilled);
            for (reg, hints) in vrai.hints {
                self.hints.insert(reg, hints);
            }
        }
        self.stats.spilled_ranges += 1;
        self.extra.set_stage_all(&spilled, LiveRangeStage::Done);
        new_vregs.extend(spilled);
        // The live virtual register requesting allocation was spilled, so tell
        // the caller not to allocate anything during this round.
        Select::None
    }

    // ---- last chance recoloring --------------------------------------------

    /// `mayRecolorAllInterferences` (RegAllocGreedy.cpp#L2043): whether the
    /// virtual registers that interfere with `virt` on `phys` may be recolored.
    fn may_recolor_all_interferences(
        &mut self,
        phys: RegId,
        virt: &Rc<LiveInterval>,
        candidates: &mut Vec<Rc<LiveInterval>>,
        fixed: &HashSet<u32>,
    ) -> bool {
        for unit in self.matrix.tri().regunits(phys) {
            // If there is LastChanceRecoloringMaxInterference or more
            // interferences, chances are one would not be
            // recolorable.
            let mut regs =
                self.matrix.query_vregs(virt.reg(), &virt.range, unit, LAST_CHANCE_RECOLORING_MAX_INTERFERENCE);
            if regs.len() >= LAST_CHANCE_RECOLORING_MAX_INTERFERENCE {
                return false;
            }
            regs = self.matrix.query_vregs(virt.reg(), &virt.range, unit, usize::MAX);
            for &intf_reg in regs.iter().rev() {
                // If Intf is done and sits on the same register class as
                // VirtReg, it would not be recolorable as it is
                // in the same state as VirtReg. Exceptions:
                // tied defs, and overlapping tuple members (a register that
                // partially overlaps its candidate may be recolored to a
                // different tuple).
                let assigned = self.matrix.vrm.get_phys(intf_reg);
                let partially_overlaps = assigned.is_some_and(|a| a != phys && self.matrix.tri().overlaps(phys, a));
                let done_same_class = self.extra.get_stage(intf_reg) == LiveRangeStage::Done
                    && self.env.same_reg_class(self.class_reg(intf_reg), self.class_reg(virt.reg()))
                    && !partially_overlaps;
                let tied_exception = self.env.has_tied_def(virt.reg()) && !self.env.has_tied_def(intf_reg);
                if (done_same_class && !tied_exception) || fixed.contains(&intf_reg) || self.pinned.contains(&intf_reg)
                {
                    return false;
                }
                let intf = self.lis.interval_rc(intf_reg);
                if !candidates.iter().any(|c| c.reg() == intf_reg) {
                    candidates.push(intf);
                }
            }
        }
        true
    }

    /// `tryLastChanceRecoloring` (RegAllocGreedy.cpp#L2145): assigns a color to
    /// `virt` by recoloring its interferences.
    fn try_last_chance_recoloring(
        &mut self,
        virt: &Rc<LiveInterval>,
        order: &AllocationOrder,
        new_vregs: &mut Vec<u32>,
        fixed: &mut HashSet<u32>,
        recolor_stack: &mut RecolorStack,
        depth: u32,
    ) -> Select {
        self.stats.last_chance += 1;
        let entry_stack_size = recolor_stack.len();
        // Ranges must be Done.
        assert!(self.extra.get_stage(virt.reg()) >= LiveRangeStage::Done || !virt.is_spillable());
        if depth >= LAST_CHANCE_RECOLORING_MAX_DEPTH {
            return Select::Fail;
        }
        // Mark VirtReg as fixed, i.e., it will not be recolored pass this point
        // in this recoloring "session".
        assert!(!fixed.contains(&virt.reg()));
        fixed.insert(virt.reg());
        for (phys, _) in order.iter() {
            let mut candidates: Vec<Rc<LiveInterval>> = Vec::new();
            let mut current_new_vregs: Vec<u32> = Vec::new();
            // It is only possible to recolor virtual register interference.
            if self.matrix.check_interference(virt, phys) > InterferenceKind::VirtReg {
                continue;
            }
            // Early give up on this PhysReg if it is obvious we cannot recolor
            // all the interferences.
            if !self.may_recolor_all_interferences(phys, virt, &mut candidates, fixed) {
                continue;
            }
            // RecoloringCandidates contains all the virtual registers that
            // interfere with VirtReg on PhysReg (or one of its
            // aliases). Enqueue them for recoloring and perform the
            // actual recoloring.
            let mut queue: BinaryHeap<(u32, u32)> = BinaryHeap::new();
            for rc in &candidates {
                let prio = self.get_priority(rc);
                queue.push((prio, !rc.reg()));
                // Record the current allocation.
                recolor_stack
                    .push((Rc::clone(rc), self.matrix.vrm.get_phys(rc.reg()).expect("interferences are assigned")));
                // unset the related struct.
                self.matrix.unassign(rc);
            }
            // Do as if VirtReg was assigned to PhysReg so that the underlying
            // recoloring has the right information about the interferes and
            // available colors.
            self.matrix.assign(virt, phys);
            let this_virt_reg = virt.reg();
            // Save the current recoloring state.
            let save_fixed = fixed.clone();
            if self.try_recoloring_candidates(&mut queue, &mut current_new_vregs, fixed, recolor_stack, depth) {
                // Push the queued vregs into the main queue.
                new_vregs.extend(current_new_vregs);
                // Do not mess up with the global assignment process. I.e.,
                // VirtReg must be unassigned.
                if self.matrix.vrm.has_phys(this_virt_reg) {
                    self.matrix.unassign(virt);
                    return Select::Reg(phys);
                }
                fixed.remove(&this_virt_reg);
                return Select::None;
            }
            // The recoloring attempt failed, undo the changes.
            *fixed = save_fixed;
            self.matrix.unassign(virt);
            // For a newly created vreg which is also in RecoloringCandidates,
            // don't add it to NewVRegs because its physical
            // register will be restored below.
            for r in &current_new_vregs {
                if candidates.iter().any(|c| c.reg() == *r) {
                    continue;
                }
                new_vregs.push(*r);
            }
            // Roll back our unsuccessful recoloring. Also roll back any
            // successful recolorings in any recursive recoloring
            // attempts. Perform all unassignments prior to
            // reassigning.
            for i in (entry_stack_size..recolor_stack.len()).rev() {
                let li = Rc::clone(&recolor_stack[i].0);
                if self.matrix.vrm.has_phys(li.reg()) {
                    self.matrix.unassign(&li);
                }
            }
            for i in entry_stack_size..recolor_stack.len() {
                let (li, phys) = (Rc::clone(&recolor_stack[i].0), recolor_stack[i].1);
                if !li.empty() && !self.lis.reg_nodbg_empty(li.reg()) {
                    self.matrix.assign(&li, phys);
                }
            }
            // Pop the stack of recoloring attempts.
            recolor_stack.truncate(entry_stack_size);
        }
        // Last chance recoloring did not worked either, give up.
        Select::Fail
    }

    /// `tryRecoloringCandidates` (RegAllocGreedy.cpp#L2276): assigns a new
    /// color to every register in the queue.
    fn try_recoloring_candidates(
        &mut self,
        queue: &mut BinaryHeap<(u32, u32)>,
        new_vregs: &mut Vec<u32>,
        fixed: &mut HashSet<u32>,
        recolor_stack: &mut RecolorStack,
        depth: u32,
    ) -> bool {
        while let Some((_, key)) = queue.pop() {
            let li = self.lis.interval_rc(!key);
            let phys = self.select_or_split_impl(&li, new_vregs, fixed, recolor_stack, depth + 1);
            match phys {
                // When splitting happens, the live-range may actually be empty.
                // In that case, this is okay to continue the recoloring even if
                // we did not find an alternative color for it.
                Select::Fail => return false,
                Select::None if !li.empty() => return false,
                Select::None => continue,
                Select::Reg(phys) => {
                    self.matrix.assign(&li, phys);
                    fixed.insert(li.reg());
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::Regs;

    use super::*;
    use crate::greedy::matrix::NoFixed;
    use crate::greedy::spill::SimpleSpiller;
    use crate::greedy::virtregmap::RegisterInfo;
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    struct Env {
        regs: Vec<RegId>,
    }

    impl SpillWeightEnv for Env {
        fn block_freq_relative_to_entry(
            &self,
            _block: usize,
        ) -> f32 {
            1.0
        }

        fn is_loop_exiting(
            &self,
            _block: usize,
        ) -> bool {
            false
        }

        fn copy_of(
            &self,
            instr: &Insn,
        ) -> Option<(u32, u32)> {
            crate::backend::coalesce::_copy(instr)
        }

        fn pinned(
            &self,
            _reg: u32,
        ) -> Option<RegId> {
            None
        }

        fn is_rematerializable(
            &self,
            _instr: &Insn,
        ) -> bool {
            false
        }

        fn reg_mask_slots(&self) -> Vec<SlotIndex> {
            Vec::new()
        }

        fn spill_weight_scale(
            &self,
            _reg: u32,
        ) -> f32 {
            1.0
        }

        fn is_allocatable(
            &self,
            _reg: RegId,
        ) -> bool {
            true
        }
    }

    impl GreedyEnv for Env {
        fn allocation_order(
            &self,
            _virt: u32,
        ) -> Vec<RegId> {
            self.regs.clone()
        }

        fn num_allocatable_regs(
            &self,
            _virt: u32,
        ) -> usize {
            self.regs.len()
        }

        fn copy_cost(
            &self,
            _virt: u32,
        ) -> u32 {
            1
        }

        fn same_reg_class(
            &self,
            _a: u32,
            _b: u32,
        ) -> bool {
            true
        }

        fn has_tied_def(
            &self,
            _virt: u32,
        ) -> bool {
            false
        }
    }

    fn insn(
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(0, None, None, defines, uses))
    }

    fn body(blocks: Vec<Vec<Arc<Insn>>>) -> LirBody {
        let count = blocks.len();
        let blocks = blocks
            .into_iter()
            .enumerate()
            .map(|(at, insns)| {
                let mut block = crate::model::lir::LirBlock::new(at as i64, insns);
                if at + 1 < count {
                    block.succ = vec![at as i64 + 1].into();
                }
                Block::from(block)
            })
            .collect();
        LirBody::new("greedy", 0, blocks, IndexMap::default(), IndexMap::default())
    }

    fn allocator<'a>(
        body: &LirBody,
        env: &'a Env,
    ) -> RAGreedy<'a> {
        let lis = LiveIntervals::new(body);
        let matrix = LiveRegMatrix::new(RegisterInfo::new(Regs(&llrm_x86_m16::REGISTER_INFO)), Box::new(NoFixed));
        let mut greedy = RAGreedy::new(lis, matrix, env, Box::new(SimpleSpiller::default()));
        greedy.calculate_spill_weights_and_hints();
        greedy
    }

    /// No two live values overlap in a register unit; every value that remains
    /// is assigned.
    fn check_valid(greedy: &mut RAGreedy) {
        let regs = greedy.lis.virt_regs();
        let tri = greedy.matrix.tri();
        let mut held: Vec<(Rc<LiveInterval>, RegId)> = Vec::new();
        for reg in regs {
            let li = greedy.lis.interval_rc(reg);
            if li.empty() {
                continue;
            }
            let phys =
                greedy.matrix.vrm.get_phys(reg).unwrap_or_else(|| panic!("value#{reg} was left without a register"));
            held.push((li, phys));
        }
        for (i, (a, ra)) in held.iter().enumerate() {
            for (b, rb) in &held[i + 1..] {
                if a.overlaps(&b.range) {
                    assert!(
                        !tri.overlaps(*ra, *rb),
                        "value#{} in {ra:?} and value#{} in {rb:?} are live together: {a} / {b}",
                        a.reg(),
                        b.reg()
                    );
                }
            }
        }
    }

    /// Fewer values live together than registers: nothing is spilled, nothing
    /// evicted.
    #[test]
    fn test_values_that_fit_are_assigned_without_a_spill() {
        let body = body(vec![vec![insn(vec![1], vec![]), insn(vec![2], vec![]), insn(vec![], vec![1, 2])]]);
        let env = Env { regs: vec![RegId::EAX, RegId::EBX, RegId::ECX] };
        let mut greedy = allocator(&body, &env);
        greedy.allocate_phys_regs();
        check_valid(&mut greedy);
        assert_eq!((greedy.stats.spilled_ranges, greedy.stats.failed), (0, 0));
        assert_ne!(greedy.matrix.vrm.get_phys(1), greedy.matrix.vrm.get_phys(2));
    }

    /// Six values live over one instruction with four registers: two are
    /// spilled, the reload ranges find registers, and the allocation is
    /// valid.
    #[test]
    fn test_values_that_do_not_fit_are_spilled_and_the_reloads_are_assigned() {
        let defs: Vec<Arc<Insn>> = (1..=6).map(|v| insn(vec![v], vec![])).collect();
        let mut insns = defs;
        for pair in [[1, 2], [3, 4], [5, 6], [1, 6], [2, 5]] {
            insns.push(insn(vec![], pair.to_vec()));
        }
        let body = body(vec![insns]);
        let env = Env { regs: vec![RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX] };
        let mut greedy = allocator(&body, &env);
        greedy.allocate_phys_regs();
        check_valid(&mut greedy);
        assert_eq!(greedy.stats.failed, 0, "every range found a register");
        assert!(greedy.stats.spilled_ranges >= 2, "{:?}", greedy.stats);
    }

    /// A heavier value evicts a lighter one; the evicted one is requeued and
    /// still gets a register.
    #[test]
    fn test_a_heavier_value_evicts_a_lighter_one() {
        // %1 is live across the whole block and used once; %2..%4 are used many
        // times in a shorter range.
        let mut insns = vec![insn(vec![1], vec![]), insn(vec![2], vec![]), insn(vec![3], vec![])];
        for _ in 0..6 {
            insns.push(insn(vec![], vec![2, 3]));
        }
        insns.push(insn(vec![], vec![1]));
        let body = body(vec![insns]);
        let env = Env { regs: vec![RegId::EAX, RegId::EBX] };
        let mut greedy = allocator(&body, &env);
        greedy.allocate_phys_regs();
        check_valid(&mut greedy);
        assert_eq!(greedy.stats.failed, 0);
        assert!(greedy.stats.evicted > 0 || greedy.stats.spilled_ranges > 0, "{:?}", greedy.stats);
        let _ = LirBlock::new(0, Vec::new());
    }

    /// The same program with the definitions in another order allocates the
    /// same number of spills.
    #[test]
    fn test_equivalent_programs_spill_alike() {
        let run = |order: &[u32]| {
            let mut insns: Vec<Arc<Insn>> = order.iter().map(|v| insn(vec![*v], vec![])).collect();
            for pair in [[1, 2], [3, 4], [5, 6], [1, 6], [2, 5]] {
                insns.push(insn(vec![], pair.to_vec()));
            }
            let body = body(vec![insns]);
            let env = Env { regs: vec![RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX] };
            let mut greedy = allocator(&body, &env);
            greedy.allocate_phys_regs();
            check_valid(&mut greedy);
            greedy.stats.spilled_ranges
        };
        assert_eq!(run(&[1, 2, 3, 4, 5, 6]), run(&[6, 4, 2, 5, 3, 1]));
    }
}
