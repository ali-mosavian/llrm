//! `LiveRangeCalc` and `LiveIntervalCalc`: building and extending live ranges
//! from definitions and uses, creating PHI-def values where several values
//! reach a join. Translated from llvm/include/llvm/CodeGen/LiveRangeCalc.h,
//! llvm/lib/CodeGen/LiveRangeCalc.cpp and llvm/lib/CodeGen/LiveIntervalCalc.cpp
//! at d1106deb71cc.
//!
//! # How it works
//! `extend(LR, Use)` makes the range live up to a use. If a def in the use's
//! block reaches it, that segment is extended. Otherwise a search up the
//! predecessors finds the values that reach the block: when there is one and it
//! is unique every path to the use is blitted in; when several reach, the
//! blocks that need the range live-in go to a work list and `updateSSA`
//! decides, from the dominator tree, which blocks need a PHI-def (a new value
//! at the block start) and which take the value of their immediate dominator.
//!
//! ```text
//!   a(0) ... ->  B1 --\
//!   a(1) ... ->  B2 ---+--> B3: use of a     B3 gets a PHI-def value 2@B3
//! ```
//!
//! DEVIATE D-LC1 (subregisters): the `Undefs` argument and
//! `isDefOnEntry` serve sub-register ranges (`<def,read-undef>` operands); a
//! range here has none, so they are not translated, and a use is found by a
//! `Operand` list instead of `MachineRegisterInfo::reg_nodbg_operands`. Several
//! ranges under construction at once (the low-level interface `SplitEditor`
//! uses) are passed as a slice, each `LiveInBlock` naming its range by index.
//! Checked line by line against `extend`, `findReachingDefs`, `updateSSA` and
//! `updateFromLiveIns` at d1106deb71cc on 2026-10-20: nothing else differs, so
//! the cost per call is the input's (ranges the coalescer joined).
//!
//! # Example
//! A value defined in B0 and used in B3, reached through B1 and B2 which do not
//! redefine it: the unique value is blitted into B1, B2 and the part of B3
//! before the use.
//!
//! # Used by
//! `liveintervals`; the split editor of the allocator's shadow core.

use std::rc::Rc;

use super::cfg::{Cfg, DomTree};
use super::liverange::{LiveInterval, LiveRange, LiveRangeUpdater, Segment, VNInfo, Vn};
use super::slotindex::{InstrKey, SlotIndex, SlotIndexes};

/// The function a calculation is about: `MF`, `Indexes`, `DomTree`.
pub struct Ctx<'a> {
    pub cfg: &'a Cfg,
    pub indexes: &'a SlotIndexes,
    pub dom: &'a DomTree,
}

/// A value and the block that defined it (`LiveOutPair`): the domtree node is
/// redundant, it can be computed as the block of the value's def.
type LiveOutPair = (Option<Vn>, Option<usize>);

/// `LiveInBlock` (LiveRangeCalc.h#L89): a block where a live range is known to
/// be live-in, but the value has not yet been determined.
pub struct LiveInBlock {
    /// Which of the ranges being calculated is live-in to this block.
    pub lr: usize,
    /// The block, until the final value has been determined and the range
    /// updated.
    pub dom_node: Option<usize>,
    /// Position in the block where the live-in range ends, or invalid if the
    /// range passes through the block.
    pub kill: SlotIndex,
    /// Live-in value filled in by `updateSSA` once it is known.
    pub value: Option<Vn>,
}

fn same(
    a: &Option<Vn>,
    b: &Option<Vn>,
) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

thread_local! {
    static RESET_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The entries `reset_live_out_map` has cleared on this thread, for a test that
/// a reset costs what was written since the last.
pub fn reset_work() -> usize {
    RESET_WORK.with(std::cell::Cell::get)
}

/// `LiveRangeCalc` (LiveRangeCalc.h#L36).
pub struct LiveRangeCalc {
    /// Active entries in `map`, also the visited set of `findReachingDefs`.
    seen: Vec<bool>,
    /// Each block where a live range is live out, to the live-out value.
    map: Vec<LiveOutPair>,
    /// Work list of blocks where the live-in value has yet to be determined.
    live_in: Vec<LiveInBlock>,
    /// The blocks of `map` written since the last reset: the only ones a reset
    /// has to clear.
    touched: Vec<usize>,
    /// Reserved: a value that is known to be "undef" (`UndefVNI`).
    undef_vni: Vn,
}

impl Default for LiveRangeCalc {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveRangeCalc {
    pub fn new() -> Self {
        Self {
            seen: Vec::new(),
            map: Vec::new(),
            live_in: Vec::new(),
            touched: Vec::new(),
            undef_vni: VNInfo::new(0xbad, SlotIndex::default()),
        }
    }

    /// `resetLiveOutMap` (LiveRangeCalc.cpp#L39).
    pub fn reset_live_out_map(
        &mut self,
        num_blocks: usize,
    ) {
        // Only what was written is cleared: a reset per live range of a
        // function of N blocks was N work each (LLVM's `Seen.clear()` is a
        // bit vector and its `Map.resize` leaves the entries).
        if self.map.len() == num_blocks {
            RESET_WORK.with(|work| work.set(work.get() + self.touched.len()));
            for at in self.touched.drain(..) {
                self.seen[at] = false;
                self.map[at] = (None, None);
            }
        } else {
            RESET_WORK.with(|work| work.set(work.get() + num_blocks));
            self.touched.clear();
            self.seen.clear();
            self.seen.resize(num_blocks, false);
            self.map.clear();
            self.map.resize(num_blocks, (None, None));
        }
    }

    /// `map[mbb]`, to be written: it is cleared at the next reset.
    fn slot(
        &mut self,
        mbb: usize,
    ) -> &mut LiveOutPair {
        self.touched.push(mbb);
        &mut self.map[mbb]
    }

    /// `reset` (LiveRangeCalc.cpp#L46): prepares caches for a new set of
    /// non-overlapping live ranges.
    pub fn reset(
        &mut self,
        num_blocks: usize,
    ) {
        self.reset_live_out_map(num_blocks);
        self.live_in.clear();
    }

    fn is_undef(
        &self,
        vni: &Option<Vn>,
    ) -> bool {
        vni.as_ref().is_some_and(|v| Rc::ptr_eq(v, &self.undef_vni))
    }

    /// `setLiveOutValue` (LiveRangeCalc.h#L225): `vni` is live out from `mbb`.
    pub fn set_live_out_value(
        &mut self,
        mbb: usize,
        vni: Option<Vn>,
    ) {
        self.seen[mbb] = true;
        *self.slot(mbb) = (vni, None);
    }

    /// `addLiveInBlock` (LiveRangeCalc.h#L238): a block with an unknown live-in
    /// value, for range number `lr` of those passed to `calculate_values`.
    pub fn add_live_in_block(
        &mut self,
        lr: usize,
        dom_node: usize,
        kill: SlotIndex,
    ) {
        self.live_in.push(LiveInBlock { lr, dom_node: Some(dom_node), kill, value: None });
    }

    /// `updateFromLiveIns` (LiveRangeCalc.cpp#L58): transfers the information
    /// from the `live_in` vector to the live ranges.
    fn update_from_live_ins(
        &mut self,
        ranges: &mut [&mut LiveRange],
        ctx: &Ctx,
    ) {
        let entries = std::mem::take(&mut self.live_in);
        let mut at = 0;
        while at < entries.len() {
            // One updater for a run of entries of one range.
            let lr = entries[at].lr;
            let mut updater = LiveRangeUpdater::new(Some(&mut *ranges[lr]));
            while at < entries.len() && entries[at].lr == lr {
                let i = &entries[at];
                at += 1;
                let Some(mbb) = i.dom_node else { continue };
                let value = i.value.clone().expect("No live-in value found");
                let (start, mut end) = *ctx.indexes.get_mbb_range(mbb);
                if i.kill.is_valid() {
                    // Value is killed inside this block.
                    end = i.kill;
                } else {
                    // The value is live-through, update LiveOut as well.
                    assert!(self.seen[mbb]);
                    *self.slot(mbb) = (Some(value.clone()), None);
                }
                updater.add(Segment::new(start, end, Some(value)));
            }
        }
    }

    /// `extend` (LiveRangeCalc.cpp#L80): extends the live range of `lr` to
    /// reach `use_`. The existing values in `lr` must be live so they
    /// jointly dominate `use_`; PHI-defs are inserted as required to
    /// preserve SSA form.
    pub fn extend(
        &mut self,
        lr: &mut LiveRange,
        use_: SlotIndex,
        ctx: &Ctx,
    ) {
        assert!(use_.is_valid(), "Invalid SlotIndex");
        let use_mbb = ctx.indexes.get_mbb_from_index(&use_.get_prev_slot());
        // Is there a def in the same MBB we can extend?
        if lr.extend_in_block(&ctx.indexes.get_mbb_start_idx(use_mbb), &use_).is_some() {
            return;
        }
        // Find the single reaching def, or determine if Use is jointly
        // dominated by multiple values, and we may need to create even
        // more phi-defs to preserve VNInfo SSA form.  Perform a search
        // for all predecessor blocks where we know the dominating
        // VNInfo.
        if self.find_reaching_defs(lr, use_mbb, use_, ctx) {
            return;
        }
        // When there were multiple different values, we may need new PHIs.
        self.calculate_values(&mut [lr], ctx);
    }

    /// `calculateValues` (LiveRangeCalc.cpp#L97): the value live-in to each
    /// block added with `add_live_in_block`, PHI-defs added as needed.
    pub fn calculate_values(
        &mut self,
        ranges: &mut [&mut LiveRange],
        ctx: &Ctx,
    ) {
        self.update_ssa(ranges, ctx);
        self.update_from_live_ins(ranges, ctx);
    }

    /// `findReachingDefs` (LiveRangeCalc.cpp#L175): the set of defs that can
    /// reach `use_`, which belongs to `use_mbb`. True when exactly one def
    /// reaches and all paths to it were added to `lr`; false when several do
    /// and the blocks that need `lr` live-in were put in `live_in`.
    fn find_reaching_defs(
        &mut self,
        lr: &mut LiveRange,
        use_mbb: usize,
        mut use_: SlotIndex,
        ctx: &Ctx,
    ) -> bool {
        // Block numbers where LR should be live-in.
        let mut work_list: Vec<usize> = vec![use_mbb];
        // Remember if we have seen more than one value.
        let mut unique_vni = true;
        let mut the_vni: Option<Vn> = None;
        // Using Seen as a visited set, perform a BFS for all reaching defs.
        let mut i = 0;
        while i != work_list.len() {
            let mbb = work_list[i];
            i += 1;
            assert!(!ctx.cfg.preds[mbb].is_empty() || mbb == ctx.cfg.entry, "Use not jointly dominated by defs.");
            for &pred in &ctx.cfg.preds[mbb] {
                // Is this a known live-out block?
                if self.seen[pred] {
                    if let Some(vni) = self.map[pred].0.clone() {
                        if the_vni.is_some() && !same(&the_vni, &Some(vni.clone())) {
                            unique_vni = false;
                        }
                        the_vni = Some(vni);
                    }
                    continue;
                }
                let (start, end) = *ctx.indexes.get_mbb_range(pred);
                // First time we see Pred.  Try to determine the live-out value,
                // but set it as null if Pred is live-through
                // with an unknown value.
                let vni = lr.extend_in_block(&start, &end);
                self.set_live_out_value(pred, vni.clone());
                if let Some(vni) = vni {
                    if the_vni.is_some() && !same(&the_vni, &Some(vni.clone())) {
                        unique_vni = false;
                    }
                    the_vni = Some(vni);
                    continue;
                }
                // No, we need a live-in value for Pred as well
                if pred != use_mbb {
                    work_list.push(pred);
                } else {
                    // Loopback to UseMBB, so value is really live through.
                    use_ = SlotIndex::default();
                }
            }
        }
        self.live_in.clear();
        // Both updateSSA() and LiveRangeUpdater benefit from ordered blocks,
        // but neither require it. Skip the sorting overhead for small
        // updates.
        if work_list.len() > 4 {
            work_list.sort_unstable();
        }
        // If a unique reaching def was found, blit in the live ranges
        // immediately.
        if unique_vni {
            let the_vni = the_vni.expect("a unique value");
            assert!(!self.is_undef(&Some(the_vni.clone())));
            let mut updater = LiveRangeUpdater::new(Some(lr));
            for bn in work_list {
                let (start, mut end) = *ctx.indexes.get_mbb_range(bn);
                // Trim the live range in UseMBB.
                if bn == use_mbb && use_.is_valid() {
                    end = use_;
                } else {
                    *self.slot(bn) = (Some(the_vni.clone()), None);
                }
                updater.add(Segment::new(start, end, Some(the_vni.clone())));
            }
            return true;
        }
        // Multiple values were found, so transfer the work list to the LiveIn
        // array where UpdateSSA will use it as a work list.
        self.live_in.reserve(work_list.len());
        for bn in work_list {
            self.add_live_in_block(0, bn, SlotIndex::default());
            if bn == use_mbb {
                self.live_in.last_mut().expect("an entry").kill = use_;
            }
        }
        false
    }

    /// `updateSSA` (LiveRangeCalc.cpp#L317): computes the values that will be
    /// live in to all requested blocks in `live_in`, creating PHI-def values as
    /// required. This is essentially the iterative algorithm SSAUpdater uses,
    /// except we already have a dominator tree.
    fn update_ssa(
        &mut self,
        ranges: &mut [&mut LiveRange],
        ctx: &Ctx,
    ) {
        // Interate until convergence.
        loop {
            let mut changed = false;
            // Propagate live-out values down the dominator tree, inserting
            // phi-defs when necessary.
            for at in 0..self.live_in.len() {
                // Skip block if the live-in value has already been determined.
                let Some(node) = self.live_in[at].dom_node else { continue };
                let mbb = node;
                let idom = ctx.dom.idom(node);
                let mut idom_value: LiveOutPair = (None, None);
                // We need a live-in value to a block with no immediate
                // dominator? This is probably an unreachable
                // block that has survived somehow.
                let mut need_phi = idom.is_none_or(|d| !self.seen[d]);
                // IDom dominates all of our predecessors, but it may not be
                // their immediate dominator. Check if any of
                // them have live-out values that are
                // properly dominated by IDom. If so, we need a phi-def here.
                if !need_phi {
                    let d = idom.expect("an immediate dominator");
                    idom_value = self.map[d].clone();
                    // Cache the DomTree node that defined the value.
                    if idom_value.0.is_some() && !self.is_undef(&idom_value.0) && idom_value.1.is_none() {
                        let block = ctx.indexes.get_mbb_from_index(&idom_value.0.as_ref().expect("a value").def());
                        idom_value.1 = Some(block);
                        self.slot(d).1 = Some(block);
                    }
                    for &pred in &ctx.cfg.preds[mbb] {
                        let mut value = self.map[pred].clone();
                        if value.0.is_none() || same(&value.0, &idom_value.0) {
                            continue;
                        }
                        if self.is_undef(&value.0) {
                            need_phi = true;
                            break;
                        }
                        // Cache the DomTree node that defined the value.
                        if value.1.is_none() {
                            let block = ctx.indexes.get_mbb_from_index(&value.0.as_ref().expect("a value").def());
                            value.1 = Some(block);
                            self.slot(pred).1 = Some(block);
                        }
                        // This predecessor is carrying something other than
                        // IDomValue. It could be
                        // because IDomValue hasn't propagated yet, or it could
                        // be because MBB is in the
                        // dominance frontier of that value.
                        if ctx.dom.dominates(d, value.1.expect("a node")) {
                            need_phi = true;
                            break;
                        }
                    }
                }
                let kill = self.live_in[at].kill;
                // Create a phi-def if required.
                if need_phi {
                    changed = true;
                    let (start, end) = *ctx.indexes.get_mbb_range(mbb);
                    let lr = &mut *ranges[self.live_in[at].lr];
                    let vni = lr.get_next_value(start);
                    self.live_in[at].value = Some(vni.clone());
                    // This block is done, we know the final value.
                    self.live_in[at].dom_node = None;
                    // Add liveness since updateFromLiveIns now skips this node.
                    if kill.is_valid() {
                        lr.add_segment(Segment::new(start, kill, Some(vni)));
                    } else {
                        lr.add_segment(Segment::new(start, end, Some(vni.clone())));
                        *self.slot(mbb) = (Some(vni), Some(node));
                    }
                } else if idom_value.0.is_some() && !self.is_undef(&idom_value.0) {
                    // No phi-def here. Remember incoming value.
                    self.live_in[at].value = idom_value.0.clone();
                    // If the IDomValue is killed in the block, don't propagate
                    // through.
                    if kill.is_valid() {
                        continue;
                    }
                    // Propagate IDomValue if it isn't killed:
                    // MBB is live-out and doesn't define its own value.
                    if same(&self.map[mbb].0, &idom_value.0) {
                        continue;
                    }
                    changed = true;
                    *self.slot(mbb) = idom_value;
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// `isJointlyDominated` (LiveRangeCalc.cpp#L433): whether the end of `mbb`
    /// is jointly dominated by the blocks of the slot indices in `defs`.
    pub fn is_jointly_dominated(
        mbb: usize,
        defs: &[SlotIndex],
        ctx: &Ctx,
    ) -> bool {
        let mut def_blocks = vec![false; ctx.cfg.len()];
        for i in defs {
            def_blocks[ctx.indexes.get_mbb_from_index(i)] = true;
        }
        let mut pred_queue: Vec<usize> = vec![mbb];
        let mut i = 0;
        while i != pred_queue.len() {
            let bn = pred_queue[i];
            i += 1;
            if def_blocks[bn] {
                continue;
            }
            if bn == ctx.cfg.entry {
                // We found a path from MBB back to the entry block without
                // hitting any of the def blocks.
                return false;
            }
            for &p in &ctx.cfg.preds[bn] {
                if !pred_queue.contains(&p) {
                    pred_queue.push(p);
                }
            }
        }
        true
    }
}

/// A def or a use of a register by an instruction: what
/// `MachineRegisterInfo::reg_nodbg_operands` gives `LiveIntervalCalc`.
#[derive(Clone, Copy, Debug)]
pub struct Operand {
    pub instr: InstrKey,
    pub is_def: bool,
    /// `readsReg()`: a use, or a def that also reads (tied, partial).
    pub reads_reg: bool,
    pub early_clobber: bool,
}

/// `LiveIntervalCalc` (LiveIntervalCalc.h): `LiveRangeCalc` for the live
/// interval of a virtual register.
#[derive(Default)]
pub struct LiveIntervalCalc {
    pub calc: LiveRangeCalc,
}

impl LiveIntervalCalc {
    pub fn new() -> Self {
        Self::default()
    }

    /// `createDeadDef` (LiveIntervalCalc.cpp#L33): the def of operand `mo` in
    /// `lr`.
    fn create_dead_def(
        indexes: &SlotIndexes,
        lr: &mut LiveRange,
        mo: &Operand,
    ) {
        let def_idx = indexes.get_instruction_index(mo.instr).get_reg_slot(mo.early_clobber);
        // Create the def in LR. This may find an existing def.
        lr.create_dead_def(def_idx);
    }

    /// `calculate` (LiveIntervalCalc.cpp#L43), without sub-registers: the live
    /// interval of `li.reg()` from the `operands` that name it.
    pub fn calculate(
        &mut self,
        li: &mut LiveInterval,
        operands: &[Operand],
        ctx: &Ctx,
    ) {
        // Step 1: Create minimal live segments for every definition of Reg.
        // Visit all def operands. If the same instruction has multiple defs of
        // Reg, createDeadDef() will deduplicate.
        for mo in operands {
            if !mo.is_def && !mo.reads_reg {
                continue;
            }
            if mo.is_def {
                Self::create_dead_def(ctx.indexes, &mut li.range, mo);
            }
        }
        // Step 2: Extend live segments to all uses, constructing SSA form as
        // necessary.
        self.calc.reset_live_out_map(ctx.cfg.len());
        self.extend_to_uses(&mut li.range, operands, ctx);
    }

    /// `createDeadDefs` (LiveIntervalCalc.cpp#L128).
    pub fn create_dead_defs(
        &mut self,
        lr: &mut LiveRange,
        operands: &[Operand],
        ctx: &Ctx,
    ) {
        for mo in operands.iter().filter(|mo| mo.is_def) {
            Self::create_dead_def(ctx.indexes, lr, mo);
        }
    }

    /// `extendToUses` (LiveIntervalCalc.cpp#L141): extends `lr` to every
    /// operand that reads the register.
    pub fn extend_to_uses(
        &mut self,
        lr: &mut LiveRange,
        operands: &[Operand],
        ctx: &Ctx,
    ) {
        for mo in operands {
            if !mo.reads_reg {
                continue;
            }
            // Determine the actual place of the use.
            let use_idx = ctx.indexes.get_instruction_index(mo.instr).get_reg_slot(mo.early_clobber);
            // MI is reading Reg. We may have visited MI before if it happens to
            // be reading Reg multiple times. That is OK, extend()
            // is idempotent.
            self.calc.extend(lr, use_idx, ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reset cleared every block's entry, N of them for each live range of
    /// a function of N blocks (5.6 M of d_faces' register allocation). It
    /// clears what was written since the last one.
    #[test]
    fn test_a_reset_costs_what_was_written_not_the_blocks() {
        let mut calc = LiveRangeCalc::new();
        calc.reset_live_out_map(3000);
        let vni = VNInfo::new(1, SlotIndex::default());
        for block in [5, 700, 2999] {
            calc.set_live_out_value(block, Some(vni.clone()));
        }
        let before = reset_work();
        calc.reset_live_out_map(3000);
        assert_eq!(reset_work() - before, 3);
        assert!(!calc.seen[700] && calc.map[700].0.is_none() && calc.map[700].1.is_none(), "what was written is clear");
    }
}
