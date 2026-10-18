//! `SplitEditor`: splits a live range by choosing intervals and inserting the
//! copies between them. Translated from llvm/lib/CodeGen/SplitKit.{h,cpp} at
//! d1106deb71cc. This file holds the editing calls and the block-level splits;
//! `finish` follows.
//!
//! # How it works
//! `openIntv` makes a new register (index 0 is the complement, the range left
//! as it was). `enterIntvBefore`/`enterIntvAfter`/`enterIntvAtEnd` insert a
//! copy from the parent into the open register,
//! `leaveIntvAfter`/`leaveIntvBefore`/ `leaveIntvAtTop` a copy into the
//! complement, and `useIntv` records in `RegAssign` which register holds the
//! value over a range of slots. The copies read the parent register; `finish`
//! rewrites every operand of the parent to the register `RegAssign` gives at
//! its slot.
//!
//! ```text
//!   parent     |--o----o----o--|
//!   RegAssign        [=====)        enterIntvBefore, leaveIntvAfter
//!   result     |--o--c1 o  o c2--|   c1: open <- parent, c2: complement <- parent
//! ```
//!
//! DEVIATE D-SE1: no rematerialization in `defFromParent`
//! (`canRematerializeAt`, `rematerializeAt`, `rematWillIncreaseRestriction`),
//! as D-E1 of `LiveRangeEdit`; every definition is a copy.
//!
//! DEVIATE D-SE2: no sub-ranges or lane masks (`addDeadDef`'s sub-range
//! branches, `buildSingleSubRegCopy`, `getLiveLaneMaskAt`, the
//! `IMPLICIT_DEF` of an empty lane mask): a value is one range over its whole
//! root, as in the shadow allocator. The fact: `LiveInterval` here has no
//! sub-ranges.
//!
//! DEVIATE D-SE3: a tied use is a value in both `defines` and `uses` of one
//! instruction (`hasTiedUseOf`, `MI->readsVirtualRegister`).
//!
//! DEVIATE D-SE4: `LRSplit` is not flagged on the copies: nothing here reads
//! it.
//!
//! # Example
//! A value live through `b1` with a use in it: `openIntv`; `enterIntvBefore`
//! the use inserts `c1 = copy v`; `leaveIntvAfter` it inserts `c0 = copy v`;
//! `useIntv(c1.def, c0.def)` assigns the span between them to `c1`.
//!
//! # Used by
//! `RAGreedy::trySplit` and its region, block and local splits.

use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use llrm_mir::blockfrequency::frequency::BlockFrequency;

use super::liverangeedit::LiveRangeEdit;
use super::splitkit::{BlockInfo, SplitAnalysis, SplitEnv, first_insert_instr};
use super::virtregmap::VirtRegMap;
use crate::liveness::blockfrequency::MachineBlockFrequencyInfo;
use crate::liveness::calcspillweights::VirtRegAuxInfo;
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::liverange::{LiveInterval, Segment, Vn};
use crate::liveness::liverangecalc::{Ctx, LiveIntervalCalc};
use crate::liveness::slotindex::{InstrKey, SlotIndex};

/// What the editor works on (the C++ editor holds references to them, which
/// Rust cannot across the allocator's own mutations).
pub struct SplitCx<'a> {
    pub lis: &'a mut LiveIntervals,
    pub vrm: &'a mut VirtRegMap,
    pub sa: &'a mut SplitAnalysis,
    pub env: &'a dyn SplitEnv,
    /// `MBFI`, for the spill modes that hoist copies.
    pub mbfi: Option<&'a MachineBlockFrequencyInfo>,
}

/// `SplitEditor::ComplementSpillMode`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.h#L291>).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComplementSpillMode {
    /// Try to create phi-defs in the complement for any value that can't be
    /// split.
    Partition,
    /// Place the back-copies in a dominating block to save code size.
    Size,
    /// Place the back-copies where they are cheapest to run.
    Speed,
}

/// `RegAssignMap`, an `IntervalMap<SlotIndex, unsigned>`: the register index
/// that holds the value over each range of slots; holes are the complement
/// (index 0).
#[derive(Default)]
pub struct RegAssignMap {
    /// start -> (stop, register index).
    map: BTreeMap<SlotIndex, (SlotIndex, u32)>,
}

impl RegAssignMap {
    pub fn clear(&mut self) {
        self.map.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// `insert(a, b, y)`: the new range joins a neighbour that ends or starts
    /// at its edge with the same value.
    pub fn insert(
        &mut self,
        mut start: SlotIndex,
        mut stop: SlotIndex,
        value: u32,
    ) {
        assert!(start < stop, "Empty interval");
        if let Some((_, (prev_stop, _))) = self.map.range(..=start).next_back() {
            assert!(*prev_stop <= start, "RegAssign ranges overlap");
        }
        if let Some((next_start, _)) = self.map.range(start..).next() {
            assert!(*next_start >= stop, "RegAssign ranges overlap");
        }
        if let Some((&left, &(left_stop, left_value))) = self.map.range(..start).next_back() {
            if left_stop == start && left_value == value {
                self.map.remove(&left);
                start = left;
            }
        }
        if let Some(&(right_stop, right_value)) = self.map.get(&stop) {
            if right_value == value {
                self.map.remove(&stop);
                stop = right_stop;
            }
        }
        self.map.insert(start, (stop, value));
    }

    /// `lookup(x)`: the value of the range holding `x`, 0 in a hole.
    pub fn lookup(
        &self,
        x: SlotIndex,
    ) -> u32 {
        match self.map.range(..=x).next_back() {
            Some((_, &(stop, value))) if stop > x => value,
            _ => 0,
        }
    }

    /// `find(x)`: the range holding `x`, or None (`I.valid()` false or
    /// `I.start() > x`: the caller asks the start).
    pub fn find(
        &self,
        x: SlotIndex,
    ) -> Option<(SlotIndex, SlotIndex, u32)> {
        match self.map.range(..=x).next_back() {
            Some((&start, &(stop, value))) if stop > x => Some((start, stop, value)),
            _ => self.map.range(x..).next().map(|(&start, &(stop, value))| (start, stop, value)),
        }
    }

    /// `setStop` of the range starting at `start`.
    pub fn set_stop(
        &mut self,
        start: SlotIndex,
        stop: SlotIndex,
    ) {
        self.map.get_mut(&start).expect("a range starts there").0 = stop;
    }

    /// The ranges in order: (start, stop, value).
    pub fn iter(&self) -> impl Iterator<Item = (SlotIndex, SlotIndex, u32)> + '_ {
        self.map.iter().map(|(&start, &(stop, value))| (start, stop, value))
    }
}

/// `ValueForcePair`: the simple def of a (register, parent value), or none once
/// it has several, and whether liveness is recomputed for it.
#[derive(Clone, Default)]
struct ValueForce {
    value: Option<Vn>,
    force: bool,
}

/// `SplitEditor`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.h#L263>).
pub struct SplitEditor<'d> {
    edit: Option<LiveRangeEdit<'d>>,
    /// `Edit->getParent()`.
    parent: Option<Rc<LiveInterval>>,
    /// The register index being filled by the `enter`/`use` calls.
    open_idx: usize,
    spill_mode: ComplementSpillMode,
    /// Where each register of the edit holds the value.
    pub reg_assign: RegAssignMap,
    /// (register index, parent value id) -> the value defining it there.
    values: HashMap<(usize, u32), ValueForce>,
    /// `LICalc[2]`: the second serves the registers after the complement in the
    /// spill modes.
    lical: [LiveIntervalCalc; 2],
}

impl<'d> Default for SplitEditor<'d> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'d> SplitEditor<'d> {
    pub fn new() -> Self {
        Self {
            edit: None,
            parent: None,
            open_idx: 0,
            spill_mode: ComplementSpillMode::Partition,
            reg_assign: RegAssignMap::default(),
            values: HashMap::new(),
            lical: [LiveIntervalCalc::new(), LiveIntervalCalc::new()],
        }
    }

    /// `reset`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L366>).
    pub fn reset(
        &mut self,
        lis: &LiveIntervals,
        lre: LiveRangeEdit<'d>,
        sm: ComplementSpillMode,
    ) {
        self.parent = Some(lis.get_interval_rc(lre.parent()));
        self.edit = Some(lre);
        self.spill_mode = sm;
        self.open_idx = 0;
        self.reg_assign.clear();
        self.values.clear();
        // Reset the LiveIntervalCalc instances needed for this spill mode.
        self.lical[0].calc.reset(lis.cfg.len());
        if sm != ComplementSpillMode::Partition {
            self.lical[1].calc.reset(lis.cfg.len());
        }
    }

    /// `Edit`.
    pub fn edit(&self) -> &LiveRangeEdit<'d> {
        self.edit.as_ref().expect("reset first")
    }

    fn edit_mut(&mut self) -> &mut LiveRangeEdit<'d> {
        self.edit.as_mut().expect("reset first")
    }

    /// `Edit->getParent()`.
    fn parent(&self) -> Rc<LiveInterval> {
        Rc::clone(self.parent.as_ref().expect("reset first"))
    }

    /// `currentIntv`.
    pub fn current_intv(&self) -> usize {
        self.open_idx
    }

    /// `defValue`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L482>):
    /// define a value in `reg_idx` for the `parent_vni` at `idx`.
    fn def_value(
        &mut self,
        cx: &mut SplitCx,
        reg_idx: usize,
        parent_vni: &Vn,
        idx: SlotIndex,
        _original: bool,
    ) -> Vn {
        assert!(idx.is_valid(), "Invalid SlotIndex");
        assert!(self.parent().get_vn_info_at(&idx).is_some_and(|v| Rc::ptr_eq(&v, parent_vni)), "Bad Parent VNI");
        let reg = self.edit().get(reg_idx);
        let li = cx.lis.interval_mut(reg);

        // Create a new value.
        let vni = li.get_next_value(idx);

        match self.values.entry((reg_idx, parent_vni.id())) {
            // The first time (RegIdx, ParentVNI) was mapped: keep it as a
            // simple def without any liveness.
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(ValueForce { value: Some(vni.clone()), force: false });
            }
            std::collections::hash_map::Entry::Occupied(mut slot) => {
                // If the previous value was a simple mapping, add liveness for
                // it now, and switch to a complex mapping.
                if let Some(old) = slot.get_mut().value.take() {
                    li.create_dead_def_vn(&old);
                }
                // This is a complex mapping, add liveness for VNI.
                li.create_dead_def_vn(&vni);
            }
        }
        vni
    }

    /// `forceRecompute`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L519>).
    pub fn force_recompute(
        &mut self,
        cx: &mut SplitCx,
        reg_idx: usize,
        parent_vni: &Vn,
    ) {
        let reg = self.edit().get(reg_idx);
        let vfp = self.values.entry((reg_idx, parent_vni.id())).or_default();
        // ParentVNI was either unmapped or already complex mapped. Either way,
        // just set the force bit.
        let Some(vni) = vfp.value.take() else {
            vfp.force = true;
            return;
        };
        // This was previously a single mapping. Make sure the old def is
        // represented by a trivial live range.
        cx.lis.interval_mut(reg).create_dead_def_vn(&vni);
        // Mark as complex mapped, forced.
        *vfp = ValueForce { value: None, force: true };
    }

    /// `defFromParent`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L639>):
    /// a copy of the parent into register `reg_idx`, inserted before `before`
    /// (or at the end of `mbb`).
    fn def_from_parent(
        &mut self,
        cx: &mut SplitCx,
        reg_idx: usize,
        parent_vni: &Vn,
        _use_idx: SlotIndex,
        mbb: usize,
        before: Option<InstrKey>,
    ) -> Vn {
        // We may be trying to avoid interference that ends at a deleted
        // instruction, so always begin RegIdx 0 early and all others late.
        let late = reg_idx != 0;
        let to = self.edit().get(reg_idx);
        let from = self.edit().get_reg();
        let copy = cx.env.build_copy(to, from);
        let def = cx.lis.insert_instr_before_at(mbb, before, copy, late).get_reg_slot(false);
        self.def_value(cx, reg_idx, parent_vni, def, false)
    }

    /// `openIntv`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L690>):
    /// create a new virtual register and live interval.
    pub fn open_intv(
        &mut self,
        cx: &mut SplitCx,
    ) -> usize {
        // Create the complement as index 0.
        if self.edit().empty() {
            self.edit_mut().create_empty_interval(cx.lis, cx.vrm);
        }
        // Create the open interval.
        self.open_idx = self.edit().size();
        self.edit_mut().create_empty_interval(cx.lis, cx.vrm);
        self.open_idx
    }

    /// `selectIntv`.
    pub fn select_intv(
        &mut self,
        idx: usize,
    ) {
        assert!(idx != 0, "Cannot select the complement interval");
        assert!(idx < self.edit().size(), "Can only select previously opened interval");
        self.open_idx = idx;
    }

    /// The block `idx` falls in and the instruction there.
    fn instr_at(
        cx: &SplitCx,
        idx: &SlotIndex,
    ) -> (usize, InstrKey) {
        let mi = cx.lis.indexes.get_instruction_from_index(idx).expect("called with an invalid index");
        (cx.lis.indexes.get_mbb_from_index(idx), mi)
    }

    /// The instruction after `mi` in `mbb`, None at the end (`std::next`).
    fn next_instr(
        cx: &SplitCx,
        mbb: usize,
        mi: InstrKey,
    ) -> Option<InstrKey> {
        let insns = cx.lis.block_insns(mbb);
        let at = insns.iter().position(|i| i.id() == mi).expect("an instruction of the block");
        insns.get(at + 1).map(|i| i.id())
    }

    /// `enterIntvBefore`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L708>):
    /// enter the open interval before the instruction at `idx`.
    pub fn enter_intv_before(
        &mut self,
        cx: &mut SplitCx,
        idx: SlotIndex,
    ) -> SlotIndex {
        assert!(self.open_idx != 0, "openIntv not called before enterIntvBefore");
        let idx = idx.get_base_index();
        let Some(parent_vni) = self.parent().get_vn_info_at(&idx) else {
            return idx;
        };
        let (mbb, mi) = Self::instr_at(cx, &idx);
        self.def_from_parent(cx, self.open_idx, &parent_vni, idx, mbb, Some(mi)).def()
    }

    /// `enterIntvAfter`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L725>).
    pub fn enter_intv_after(
        &mut self,
        cx: &mut SplitCx,
        idx: SlotIndex,
    ) -> SlotIndex {
        assert!(self.open_idx != 0, "openIntv not called before enterIntvAfter");
        let idx = idx.get_boundary_index();
        let Some(parent_vni) = self.parent().get_vn_info_at(&idx) else {
            return idx;
        };
        let (mbb, mi) = Self::instr_at(cx, &idx);
        let next = Self::next_instr(cx, mbb, mi);
        self.def_from_parent(cx, self.open_idx, &parent_vni, idx, mbb, next).def()
    }

    /// `enterIntvAtEnd`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L743>).
    pub fn enter_intv_at_end(
        &mut self,
        cx: &mut SplitCx,
        mbb: usize,
    ) -> SlotIndex {
        assert!(self.open_idx != 0, "openIntv not called before enterIntvAtEnd");
        let end = cx.lis.indexes.get_mbb_end_idx(mbb);
        let mut last = end.get_prev_slot();
        let Some(mut parent_vni) = self.parent().get_vn_info_at(&last) else {
            return end;
        };
        let lsp = cx.sa.get_last_split_point(cx.lis, cx.env, mbb);
        if lsp < last {
            // It could be that the use after LSP is a def, and thus the
            // ParentVNI just selected starts at that def. For this case to
            // exist, the def must be part of a tied def/use pair, and thus we
            // can insert the def into the VNI of the use and the tied def/use
            // pair can live in the resulting interval.
            last = lsp;
            match self.parent().get_vn_info_at(&last) {
                Some(v) => parent_vni = v,
                // undef use --> undef tied def
                None => return end,
            }
        }
        let before = last_split_point_instr(cx, mbb, lsp);
        let vni = self.def_from_parent(cx, self.open_idx, &parent_vni, last, mbb, before);
        self.reg_assign.insert(vni.def(), end, self.open_idx as u32);
        vni.def()
    }

    /// `useIntv(MBB)`: all instructions in `mbb` use the open interval.
    pub fn use_intv_block(
        &mut self,
        cx: &SplitCx,
        mbb: usize,
    ) {
        let (start, end) = *cx.lis.indexes.get_mbb_range(mbb);
        self.use_intv(start, end);
    }

    /// `useIntv`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L784>).
    pub fn use_intv(
        &mut self,
        start: SlotIndex,
        end: SlotIndex,
    ) {
        assert!(self.open_idx != 0, "openIntv not called before useIntv");
        self.reg_assign.insert(start, end, self.open_idx as u32);
    }

    /// `leaveIntvAfter`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L791>):
    /// leave the open interval after the instruction at `idx`.
    pub fn leave_intv_after(
        &mut self,
        cx: &mut SplitCx,
        idx: SlotIndex,
    ) -> SlotIndex {
        assert!(self.open_idx != 0, "openIntv not called before leaveIntvAfter");
        // The interval must be live beyond the instruction at idx.
        let boundary = idx.get_boundary_index();
        let Some(parent_vni) = self.parent().get_vn_info_at(&boundary) else {
            return boundary.get_next_slot();
        };
        let (mbb, mi) = Self::instr_at(cx, &boundary);

        // In spill mode, make live ranges as short as possible by inserting
        // the copy before MI. This is only possible if that instruction
        // doesn't redefine the value. The inserted COPY is not a kill, and we
        // don't need to recompute the source live range.
        if self.spill_mode != ComplementSpillMode::Partition
            && !SlotIndex::is_same_instr(&parent_vni.def(), &idx)
            && cx.lis.instr(mi).uses.contains(&self.edit().get_reg())
        {
            self.force_recompute(cx, 0, &parent_vni);
            self.def_from_parent(cx, 0, &parent_vni, idx, mbb, Some(mi));
            return idx;
        }

        let next = Self::next_instr(cx, mbb, mi);
        self.def_from_parent(cx, 0, &parent_vni, boundary, mbb, next).def()
    }

    /// `leaveIntvBefore`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L822>).
    pub fn leave_intv_before(
        &mut self,
        cx: &mut SplitCx,
        idx: SlotIndex,
    ) -> SlotIndex {
        assert!(self.open_idx != 0, "openIntv not called before leaveIntvBefore");
        // The interval must be live into the instruction at idx.
        let idx = idx.get_base_index();
        let Some(parent_vni) = self.parent().get_vn_info_at(&idx) else {
            return idx.get_next_slot();
        };
        let (mbb, mi) = Self::instr_at(cx, &idx);
        self.def_from_parent(cx, 0, &parent_vni, idx, mbb, Some(mi)).def()
    }

    /// `leaveIntvAtTop`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L841>).
    pub fn leave_intv_at_top(
        &mut self,
        cx: &mut SplitCx,
        mbb: usize,
    ) -> SlotIndex {
        assert!(self.open_idx != 0, "openIntv not called before leaveIntvAtTop");
        let start = cx.lis.indexes.get_mbb_start_idx(mbb);
        let Some(parent_vni) = self.parent().get_vn_info_at(&start) else {
            return start;
        };
        let before = first_insert_instr(cx.lis, mbb);
        let vni = self.def_from_parent(cx, 0, &parent_vni, start, mbb, before);
        self.reg_assign.insert(start, vni.def(), self.open_idx as u32);
        vni.def()
    }

    /// `overlapIntv`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L868>).
    pub fn overlap_intv(
        &mut self,
        cx: &mut SplitCx,
        start: SlotIndex,
        end: SlotIndex,
    ) {
        assert!(self.open_idx != 0, "openIntv not called before overlapIntv");
        let parent = self.parent();
        let parent_vni = parent.get_vn_info_at(&start);
        assert!(
            match (&parent_vni, parent.get_vn_info_before(&end)) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, &b),
                (None, None) => true,
                _ => false,
            },
            "Parent changes value in extended range"
        );
        assert!(
            cx.lis.indexes.get_mbb_from_index(&start) == cx.lis.indexes.get_mbb_from_index(&end),
            "Range cannot span basic blocks"
        );

        // The complement interval will be extended as needed by
        // LICalc.extend().
        if let Some(vni) = &parent_vni {
            self.force_recompute(cx, 0, vni);
        }

        // If the last use is tied to a def, we can't mark it as live for the
        // interval which includes only the use. That would cause the tied pair
        // to end up in two different intervals.
        if let Some(mi) = cx.lis.indexes.get_instruction_from_index(&end) {
            let reg = self.edit().get_reg();
            let insn = cx.lis.instr(mi);
            if insn.defines.contains(&reg) && insn.uses.contains(&reg) {
                return;
            }
        }

        self.reg_assign.insert(start, end, self.open_idx as u32);
    }

    /// `splitSingleBlock`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1650>).
    pub fn split_single_block(
        &mut self,
        cx: &mut SplitCx,
        bi: &BlockInfo,
    ) {
        self.open_intv(cx);
        let last_split_point = cx.sa.get_last_split_point(cx.lis, cx.env, bi.mbb);
        let seg_start = self.enter_intv_before(cx, bi.first_instr.min(last_split_point));
        if !bi.live_out || bi.last_instr < last_split_point {
            let stop = self.leave_intv_after(cx, bi.last_instr);
            self.use_intv(seg_start, stop);
        } else {
            // The last use is after the last valid split point.
            let seg_stop = self.leave_intv_before(cx, last_split_point);
            self.use_intv(seg_start, seg_stop);
            self.overlap_intv(cx, seg_stop, bi.last_instr);
        }
    }

    /// `splitLiveThroughBlock`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1676>).
    pub fn split_live_through_block(
        &mut self,
        cx: &mut SplitCx,
        mbb_num: usize,
        intv_in: usize,
        leave_before: SlotIndex,
        intv_out: usize,
        enter_after: SlotIndex,
    ) {
        let (start, stop) = *cx.lis.indexes.get_mbb_range(mbb_num);

        assert!(intv_in != 0 || intv_out != 0, "Use splitSingleBlock for isolated blocks");
        assert!(!leave_before.is_valid() || leave_before < stop, "Interference after block");
        assert!(intv_in == 0 || !leave_before.is_valid() || leave_before > start, "Impossible intf");
        assert!(!enter_after.is_valid() || enter_after >= start, "Interference before block");

        if intv_out == 0 {
            //        <<<<<<<<<    Possible LeaveBefore interference.
            //    |-----------|    Live through.
            //    -____________    Spill on entry.
            self.select_intv(intv_in);
            let idx = self.leave_intv_at_top(cx, mbb_num);
            assert!(!leave_before.is_valid() || idx <= leave_before, "Interference");
            return;
        }

        if intv_in == 0 {
            //    >>>>>>>          Possible EnterAfter interference.
            //    |-----------|    Live through.
            //    ___________--    Reload on exit.
            self.select_intv(intv_out);
            let idx = self.enter_intv_at_end(cx, mbb_num);
            assert!(!enter_after.is_valid() || idx >= enter_after, "Interference");
            return;
        }

        if intv_in == intv_out && !leave_before.is_valid() && !enter_after.is_valid() {
            //    |-----------|    Live through.
            //    -------------    Straight through, same intv, no interference.
            self.select_intv(intv_out);
            self.use_intv(start, stop);
            return;
        }

        // We cannot legally insert splits after LSP.
        let lsp = cx.sa.get_last_split_point(cx.lis, cx.env, mbb_num);
        assert!(intv_out == 0 || !enter_after.is_valid() || enter_after < lsp, "Impossible intf");

        if intv_in != intv_out
            && (!leave_before.is_valid()
                || !enter_after.is_valid()
                || leave_before.get_base_index() > enter_after.get_boundary_index())
        {
            //    >>>>     <<<<    Non-overlapping EnterAfter/LeaveBefore
            // interference.    |-----------|    Live through.
            //    ------=======    Switch intervals between interference.
            self.select_intv(intv_out);
            let idx;
            if leave_before.is_valid() && leave_before < lsp {
                idx = self.enter_intv_before(cx, leave_before);
                self.use_intv(idx, stop);
            } else {
                idx = self.enter_intv_at_end(cx, mbb_num);
            }
            self.select_intv(intv_in);
            self.use_intv(start, idx);
            assert!(!leave_before.is_valid() || idx <= leave_before, "Interference");
            assert!(!enter_after.is_valid() || idx >= enter_after, "Interference");
            return;
        }

        //    >>><><><><<<<    Overlapping EnterAfter/LeaveBefore interference.
        //    |-----------|    Live through.
        //    ==---------==    Switch intervals before/after interference.
        assert!(leave_before <= enter_after, "Missed case");

        self.select_intv(intv_out);
        let idx = self.enter_intv_after(cx, enter_after);
        self.use_intv(idx, stop);
        assert!(!enter_after.is_valid() || idx >= enter_after, "Interference");

        self.select_intv(intv_in);
        let idx = self.leave_intv_before(cx, leave_before);
        self.use_intv(start, idx);
        assert!(!leave_before.is_valid() || idx <= leave_before, "Interference");
    }

    /// `splitRegInBlock`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1778>).
    pub fn split_reg_in_block(
        &mut self,
        cx: &mut SplitCx,
        bi: &BlockInfo,
        intv_in: usize,
        leave_before: SlotIndex,
    ) {
        let (start, _stop) = *cx.lis.indexes.get_mbb_range(bi.mbb);

        assert!(intv_in != 0, "Must have register in");
        assert!(bi.live_in, "Must be live-in");
        assert!(!leave_before.is_valid() || leave_before > start, "Bad interference");

        if !bi.live_out && (!leave_before.is_valid() || leave_before >= bi.last_instr) {
            //               <<<    Interference after kill.
            //     |---o---x   |    Killed in block.
            //     =========        Use IntvIn everywhere.
            self.select_intv(intv_in);
            self.use_intv(start, bi.last_instr);
            return;
        }

        let lsp = cx.sa.get_last_split_point(cx.lis, cx.env, bi.mbb);

        if !leave_before.is_valid() || leave_before > bi.last_instr.get_boundary_index() {
            //               <<<    Possible interference after last use.
            //     |---o---o---|    Live-out on stack.
            //     =========____    Leave IntvIn after last use.
            //
            //                 <    Interference after last use.
            //     |---o---o--o|    Live-out on stack, late last use.
            //     ============     Copy to stack after LSP, overlap IntvIn.
            //            \_____    Stack interval is live-out.
            if bi.last_instr < lsp {
                self.select_intv(intv_in);
                let idx = self.leave_intv_after(cx, bi.last_instr);
                self.use_intv(start, idx);
                assert!(!leave_before.is_valid() || idx <= leave_before, "Interference");
            } else {
                self.select_intv(intv_in);
                let idx = self.leave_intv_before(cx, lsp);
                self.overlap_intv(cx, idx, bi.last_instr);
                self.use_intv(start, idx);
                assert!(!leave_before.is_valid() || idx <= leave_before, "Interference");
            }
            return;
        }

        // The interference is overlapping somewhere we wanted to use IntvIn.
        // That means we need to create a local interval that can be allocated
        // a different register.
        self.open_intv(cx);

        if !bi.live_out || bi.last_instr < lsp {
            //           <<<<<<<    Interference overlapping uses.
            //     |---o---o---|    Live-out on stack.
            //     =====----____    Leave IntvIn before interference, then
            // spill.
            let to = self.leave_intv_after(cx, bi.last_instr);
            let from = self.enter_intv_before(cx, leave_before);
            self.use_intv(from, to);
            self.select_intv(intv_in);
            self.use_intv(start, from);
            assert!(!leave_before.is_valid() || from <= leave_before, "Interference");
            return;
        }

        //           <<<<<<<    Interference overlapping uses.
        //     |---o---o--o|    Live-out on stack, late last use.
        //     =====-------     Copy to stack before LSP, overlap LocalIntv.
        //            \_____    Stack interval is live-out.
        let to = self.leave_intv_before(cx, lsp);
        self.overlap_intv(cx, to, bi.last_instr);
        let from = self.enter_intv_before(cx, min_valid(to, leave_before));
        self.use_intv(from, to);
        self.select_intv(intv_in);
        self.use_intv(start, from);
        assert!(!leave_before.is_valid() || from <= leave_before, "Interference");
    }

    /// `splitRegOutBlock`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1871>).
    pub fn split_reg_out_block(
        &mut self,
        cx: &mut SplitCx,
        bi: &BlockInfo,
        intv_out: usize,
        enter_after: SlotIndex,
    ) {
        let (_start, stop) = *cx.lis.indexes.get_mbb_range(bi.mbb);
        let lsp = cx.sa.get_last_split_point(cx.lis, cx.env, bi.mbb);

        assert!(intv_out != 0, "Must have register out");
        assert!(bi.live_out, "Must be live-out");
        assert!(!enter_after.is_valid() || enter_after < lsp, "Bad interference");

        if !bi.live_in && (!enter_after.is_valid() || enter_after <= bi.first_instr) {
            //    >>>>             Interference before def.
            //    |   o---o---|    Defined in block.
            //        =========    Use IntvOut everywhere.
            self.select_intv(intv_out);
            self.use_intv(bi.first_instr, stop);
            return;
        }

        if !enter_after.is_valid() || enter_after < bi.first_instr.get_base_index() {
            //    >>>>             Interference before def.
            //    |---o---o---|    Live-through, stack-in.
            //    ____=========    Enter IntvOut before first use.
            self.select_intv(intv_out);
            let idx = self.enter_intv_before(cx, lsp.min(bi.first_instr));
            self.use_intv(idx, stop);
            assert!(!enter_after.is_valid() || idx >= enter_after, "Interference");
            return;
        }

        // The interference is overlapping somewhere we wanted to use IntvOut.
        // That means we need to create a local interval that can be allocated
        // a different register.
        //    >>>>>>>          Interference overlapping uses.
        //    |---o---o---|    Live-through, stack-in.
        //    ____---======    Create local interval for interference range.
        self.select_intv(intv_out);
        let idx = self.enter_intv_after(cx, enter_after);
        self.use_intv(idx, stop);
        assert!(!enter_after.is_valid() || idx >= enter_after, "Interference");

        self.open_intv(cx);
        let from = self.enter_intv_before(cx, idx.min(bi.first_instr));
        self.use_intv(from, idx);
    }
}

impl<'d> SplitEditor<'d> {
    /// `getLICalc`: the calculator that serves register `reg_idx`.
    fn lic_of(
        &self,
        reg_idx: usize,
    ) -> usize {
        (self.spill_mode != ComplementSpillMode::Partition && reg_idx != 0) as usize
    }

    /// The registers of the edit, in order.
    fn regs(&self) -> Vec<u32> {
        (0..self.edit().size()).map(|i| self.edit().get(i)).collect()
    }

    /// `removeBackCopies`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L898>).
    fn remove_back_copies(
        &mut self,
        cx: &mut SplitCx,
        copies: &[Vn],
    ) {
        let li_reg = self.edit().get(0);
        let parent = self.parent();

        for c in copies {
            let def = c.def();
            let mi = cx.lis.indexes.get_instruction_from_index(&def).expect("No instruction for back-copy");
            let mbb = cx.lis.indexes.get_mbb_from_index(&def);
            let insns = cx.lis.block_insns(mbb);
            let at = insns.iter().position(|i| i.id() == mi).expect("an instruction of the block");
            // The previous instruction (`--MBBI`, no debug instructions here).
            let prev = if at == 0 { None } else { Some(insns[at - 1].id()) };

            LiveIntervals::remove_vreg_def_at(cx.lis.interval_mut(li_reg), &def);
            cx.lis.remove_instr(mbb, mi);

            // Adjust RegAssign if a register assignment is killed at Def. We
            // want to avoid calculating the live range of the source register
            // if possible.
            let Some((a_start, a_stop, reg_idx)) = self.reg_assign.find(def.get_prev_slot()) else { continue };
            if a_start >= def {
                continue;
            }
            // If MI doesn't kill the assigned register, just leave it.
            if a_stop != def {
                continue;
            }
            // We could hoist back-copy right after another back-copy. As a
            // result the previous instruction is actually dead now. We cannot
            // set its stop to it, which would be the same as start.
            let kill = prev.map(|k| cx.lis.indexes.get_instruction_index(k).get_reg_slot(false));
            let reads = prev.is_some_and(|k| cx.lis.instr(k).uses.contains(&self.edit().get_reg()));
            match kill {
                Some(kill) if reads && kill > a_start => {
                    self.reg_assign.set_stop(a_start, kill);
                }
                _ => {
                    let vni = parent.get_vn_info_at(&def).expect("the parent is live at the back-copy");
                    self.force_recompute(cx, reg_idx as usize, &vni);
                }
            }
        }
    }

    /// `findShallowDominator`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L950>).
    fn find_shallow_dominator(
        &self,
        cx: &SplitCx,
        mut mbb: usize,
        def_mbb: usize,
    ) -> usize {
        if mbb == def_mbb {
            return mbb;
        }
        assert!(cx.lis.dom.dominates(def_mbb, mbb), "MBB must be dominated by the def.");

        let def_loop = cx.env.loop_of(def_mbb);

        // Best candidate so far.
        let mut best_mbb = mbb;
        let mut best_depth = u32::MAX;

        loop {
            let l = cx.env.loop_of(mbb);

            // MBB isn't in a loop, it doesn't get any better. All dominators
            // have a higher frequency by definition.
            let Some(l) = l else { return mbb };

            // We'll never be able to exit the DefLoop.
            if Some(l) == def_loop {
                return mbb;
            }

            // Least busy dominator seen so far.
            let depth = cx.env.loop_depth(l);
            if depth < best_depth {
                best_mbb = mbb;
                best_depth = depth;
            }

            // Leave loop by going to the immediate dominator of the loop
            // header. This is a bigger stride than simply walking up the
            // dominator tree.
            let idom = cx.lis.dom.idom(cx.env.loop_header(l));

            // Too far up the dominator tree?
            match idom {
                Some(idom) if cx.lis.dom.dominates(def_mbb, idom) => mbb = idom,
                _ => return best_mbb,
            }
        }
    }

    /// `computeRedundantBackCopies`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1004>).
    fn compute_redundant_back_copies(
        &mut self,
        cx: &mut SplitCx,
        not_to_hoist: &std::collections::HashSet<u32>,
        back_copies: &mut Vec<Vn>,
    ) {
        let li_reg = self.edit().get(0);
        let parent = self.parent();
        let n = parent.get_num_val_nums() as usize;
        let mut equal_vns: Vec<Vec<Vn>> = vec![Vec::new(); n];
        let mut dominated: Vec<Vn> = Vec::new();

        // Aggregate VNIs having the same value as ParentVNI.
        for vni in cx.lis.get_interval(li_reg).valnos.clone() {
            if vni.is_unused() {
                continue;
            }
            let pvni = parent.get_vn_info_at(&vni.def()).expect("the parent is live at a complement def");
            equal_vns[pvni.id() as usize].push(vni);
        }

        // For VNI aggregation of each ParentVNI, collect dominated, i.e.,
        // redundant VNIs to BackCopies.
        for i in 0..n {
            let pvni = parent.get_val_num_info(i as u32);
            if !not_to_hoist.contains(&pvni.id()) {
                continue;
            }
            let group = &equal_vns[pvni.id() as usize];
            for (a, it1) in group.iter().enumerate() {
                for it2 in &group[a + 1..] {
                    let in_dom = |v: &Vn, d: &[Vn]| d.iter().any(|x| Rc::ptr_eq(x, v));
                    if in_dom(it1, &dominated) || in_dom(it2, &dominated) {
                        continue;
                    }
                    let mbb1 = cx.lis.indexes.get_mbb_from_index(&it1.def());
                    let mbb2 = cx.lis.indexes.get_mbb_from_index(&it2.def());
                    if mbb1 == mbb2 {
                        dominated.push(if it1.def() < it2.def() { it2.clone() } else { it1.clone() });
                    } else if cx.lis.dom.dominates(mbb1, mbb2) {
                        dominated.push(it2.clone());
                    } else if cx.lis.dom.dominates(mbb2, mbb1) {
                        dominated.push(it1.clone());
                    }
                }
            }
            if !dominated.is_empty() {
                self.force_recompute(cx, 0, &pvni);
                back_copies.append(&mut dominated);
            }
        }
    }

    /// `hoistCopies`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1057>):
    /// for the size mode, find a common dominator for all the back-copies of
    /// the same parent value and hoist them there; for the speed mode, if the
    /// common dominator is hot, remove the dominated back-copies instead.
    fn hoist_copies(
        &mut self,
        cx: &mut SplitCx,
    ) {
        // Get the complement interval, always RegIdx 0.
        let li_reg = self.edit().get(0);
        let parent = self.parent();
        let nvn = parent.get_num_val_nums() as usize;
        let mbfi = cx.mbfi.expect("the spill modes hoist by block frequency");

        // Track the nearest common dominator for all back-copies for each
        // ParentVNI. If a single back-copy dominates, put it in the second.
        let mut nearest_dom: Vec<(Option<usize>, SlotIndex)> = vec![(None, SlotIndex::default()); nvn];
        // The total cost of all the back-copies for each ParentVNI.
        let mut costs: Vec<BlockFrequency> = vec![BlockFrequency(0); nvn];
        // The ParentVNI->id set for which hoisting back-copies are not
        // beneficial for Speed.
        let mut not_to_hoist: std::collections::HashSet<u32> = std::collections::HashSet::new();

        // Find the nearest common dominator for parent values with multiple
        // back-copies.
        for vni in cx.lis.get_interval(li_reg).valnos.clone() {
            if vni.is_unused() {
                continue;
            }
            let pvni = parent.get_vn_info_at(&vni.def()).expect("Parent not live at complement def");
            let val_mbb = cx.lis.indexes.get_mbb_from_index(&vni.def());
            let id = pvni.id() as usize;

            // Keep directly defined parent values. This is either a PHI or an
            // instruction in the complement range. All other copies of
            // ParentVNI should be eliminated.
            if vni.def() == pvni.def() {
                nearest_dom[id] = (Some(val_mbb), vni.def());
                continue;
            }
            // Skip the singly mapped values. There is nothing to gain from
            // hoisting a single back-copy.
            if self.values.get(&(0, pvni.id())).is_some_and(|v| v.value.is_some()) {
                continue;
            }

            match nearest_dom[id].0 {
                // First time we see ParentVNI. VNI dominates itself.
                None => nearest_dom[id] = (Some(val_mbb), vni.def()),
                // Two defs in the same block. Pick the earlier def.
                Some(first) if first == val_mbb => {
                    let second = nearest_dom[id].1;
                    if !second.is_valid() || vni.def() < second {
                        nearest_dom[id].1 = vni.def();
                    }
                    costs[id] = costs[id] + mbfi.get_block_freq(val_mbb);
                }
                // Different basic blocks. Check if one dominates.
                Some(first) => {
                    let near = cx.lis.dom.find_nearest_common_dominator(first, val_mbb);
                    if near == val_mbb {
                        // Def ValMBB dominates.
                        nearest_dom[id] = (Some(val_mbb), vni.def());
                    } else if near != first {
                        // None dominate. Hoist to common dominator, need new
                        // def.
                        nearest_dom[id] = (Some(near), SlotIndex::default());
                    }
                    costs[id] = costs[id] + mbfi.get_block_freq(val_mbb);
                }
            }
        }

        // Insert the hoisted copies.
        for i in 0..nvn {
            let (Some(dom_mbb), second) = nearest_dom[i] else { continue };
            if second.is_valid() {
                continue;
            }
            // This value needs a hoisted copy inserted at the end of dom_mbb.
            let pvni = parent.get_val_num_info(i as u32);
            let def_mbb = cx.lis.indexes.get_mbb_from_index(&pvni.def());
            // Get a less loopy dominator than dom_mbb.
            let dom_mbb = self.find_shallow_dominator(cx, dom_mbb, def_mbb);
            nearest_dom[i].0 = Some(dom_mbb);
            if self.spill_mode == ComplementSpillMode::Speed && mbfi.get_block_freq(dom_mbb) > costs[i] {
                not_to_hoist.insert(pvni.id());
                continue;
            }
            let lsp = cx.sa.get_last_split_point(cx.lis, cx.env, dom_mbb);
            if lsp <= pvni.def() {
                not_to_hoist.insert(pvni.id());
                continue;
            }
            let before = last_split_point_instr(cx, dom_mbb, lsp);
            nearest_dom[i].1 = self.def_from_parent(cx, 0, &pvni, lsp, dom_mbb, before).def();
        }

        // Remove redundant back-copies that are now known to be dominated by
        // another def with the same value.
        let mut back_copies: Vec<Vn> = Vec::new();
        for vni in cx.lis.get_interval(li_reg).valnos.clone() {
            if vni.is_unused() {
                continue;
            }
            let pvni = parent.get_vn_info_at(&vni.def()).expect("Parent not live at complement def");
            let (dom_mbb, second) = nearest_dom[pvni.id() as usize];
            if dom_mbb.is_none() || second == vni.def() || not_to_hoist.contains(&pvni.id()) {
                continue;
            }
            back_copies.push(vni);
            self.force_recompute(cx, 0, &pvni);
        }

        // If it is not beneficial to hoist all the BackCopies, simply remove
        // redundant BackCopies in speed mode.
        if self.spill_mode == ComplementSpillMode::Speed && !not_to_hoist.is_empty() {
            self.compute_redundant_back_copies(cx, &not_to_hoist, &mut back_copies);
        }

        self.remove_back_copies(cx, &back_copies);
    }

    /// `transferValues`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1180>):
    /// transfer all possible values to the new live ranges. Values with forced
    /// recomputation are left alone, they need `LICalc.extend()`.
    fn transfer_values(
        &mut self,
        cx: &mut SplitCx,
    ) -> bool {
        let regs = self.regs();
        let mut ints: Vec<LiveInterval> = regs.iter().map(|&r| cx.lis.take_interval(r)).collect();
        let parent = self.parent();
        let assigned: Vec<(SlotIndex, SlotIndex, u32)> = self.reg_assign.iter().collect();
        let mut assign_i = 0usize;
        let mut skipped = false;

        for s in &parent.segments {
            let parent_vni = s.valno.clone().expect("a segment has a value");
            // RegAssign has holes where RegIdx 0 should be used.
            let mut start = s.start;
            // `AssignI.advanceTo(Start)`.
            while assign_i < assigned.len() && assigned[assign_i].1 <= start {
                assign_i += 1;
            }
            loop {
                let reg_idx: usize;
                let mut end = s.end;
                if assign_i >= assigned.len() {
                    reg_idx = 0;
                } else if assigned[assign_i].0 <= start {
                    reg_idx = assigned[assign_i].2 as usize;
                    if assigned[assign_i].1 < end {
                        end = assigned[assign_i].1;
                        assign_i += 1;
                    }
                } else {
                    reg_idx = 0;
                    end = end.min(assigned[assign_i].0);
                }

                // The interval [Start;End) is continuously mapped to RegIdx,
                // ParentVNI.

                // Check for a simply defined value that can be blitted
                // directly.
                let vfp = self.values.get(&(reg_idx, parent_vni.id())).cloned().unwrap_or_default();
                if let Some(vni) = vfp.value {
                    ints[reg_idx].add_segment(Segment::new(start, end, Some(vni)));
                    start = end;
                    if start == s.end {
                        break;
                    }
                    continue;
                }

                // Skip values with forced recomputation.
                if vfp.force {
                    skipped = true;
                    start = end;
                    if start == s.end {
                        break;
                    }
                    continue;
                }

                let lic = self.lic_of(reg_idx);

                // This value has multiple defs in RegIdx, but it wasn't
                // rematerialized, so the live range is accurate. Add live-in
                // blocks in [Start;End) to the LiveInBlocks.
                let mut mbb = cx.lis.indexes.get_mbb_from_index(&start);
                let (mut block_start, mut block_end) = *cx.lis.indexes.get_mbb_range(mbb);

                // The first block may be live-in, or it may have its own def.
                if start != block_start {
                    let vni = ints[reg_idx]
                        .extend_in_block(&block_start, &block_end.min(end))
                        .expect("Missing def for complex mapped value");
                    // MBB has its own def. Is it also live-out?
                    if block_end <= end {
                        self.lical[lic].calc.set_live_out_value(mbb, Some(vni));
                    }
                    // Skip to the next block for live-in.
                    mbb += 1;
                    block_start = block_end;
                }

                // Handle the live-in blocks covered by [Start;End).
                assert!(start <= block_start, "Expected live-in block");
                while block_start < end {
                    block_end = cx.lis.indexes.get_mbb_end_idx(mbb);
                    if block_start == parent_vni.def() {
                        // This block has the def of a parent PHI, so it isn't
                        // live-in.
                        assert!(parent_vni.is_phi_def(), "Non-phi defined at block start?");
                        let vni = ints[reg_idx]
                            .extend_in_block(&block_start, &block_end.min(end))
                            .expect("Missing def for complex mapped parent PHI");
                        if end >= block_end {
                            // Live-out as well.
                            self.lical[lic].calc.set_live_out_value(mbb, Some(vni));
                        }
                    } else if end < block_end {
                        // This block needs a live-in value. The last block
                        // covered may not be live-out.
                        self.lical[lic].calc.add_live_in_block(reg_idx, mbb, end);
                    } else {
                        // Live-through, and we don't know the value.
                        self.lical[lic].calc.add_live_in_block(reg_idx, mbb, SlotIndex::default());
                        self.lical[lic].calc.set_live_out_value(mbb, None);
                    }
                    block_start = block_end;
                    mbb += 1;
                }
                start = end;
                if start == s.end {
                    break;
                }
            }
        }

        {
            let ctx = Ctx { cfg: &cx.lis.cfg, indexes: &cx.lis.indexes, dom: &cx.lis.dom };
            let mut ranges: Vec<&mut crate::liveness::liverange::LiveRange> =
                ints.iter_mut().map(|l| &mut l.range).collect();
            self.lical[0].calc.calculate_values(&mut ranges, &ctx);
            if self.spill_mode != ComplementSpillMode::Partition {
                self.lical[1].calc.calculate_values(&mut ranges, &ctx);
            }
        }
        for li in ints {
            cx.lis.insert_interval(li);
        }
        skipped
    }

    /// `removeDeadSegment`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1288>).
    fn remove_dead_segment(
        def: SlotIndex,
        lr: &mut crate::liveness::liverange::LiveRange,
    ) -> bool {
        let at = lr.find_segment_containing(&def);
        if at == lr.segments.len() {
            return true;
        }
        if lr.segments[at].end != def.get_dead_slot() {
            return false;
        }
        // This is a dead PHI. Remove it.
        lr.remove_segment_at(at, true);
        true
    }

    /// `extendPHIKillRanges`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1317>):
    /// extend live ranges to be live-out for successor PHI values.
    fn extend_phi_kill_ranges(
        &mut self,
        cx: &mut SplitCx,
    ) {
        // Visit each PHI def slot in the parent live interval. If the def is
        // dead, remove it. Otherwise, extend the live interval to reach the
        // end indexes of all predecessor blocks.
        let parent = self.parent();
        let regs = self.regs();
        let mut ints: Vec<LiveInterval> = regs.iter().map(|&r| cx.lis.take_interval(r)).collect();
        for v in &parent.valnos {
            if v.is_unused() || !v.is_phi_def() {
                continue;
            }
            let reg_idx = self.reg_assign.lookup(v.def()) as usize;
            let lic = self.lic_of(reg_idx);
            let b = cx.lis.indexes.get_mbb_from_index(&v.def());
            if !Self::remove_dead_segment(v.def(), &mut ints[reg_idx].range) {
                // `extendPHIRange`
                // (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1299>).
                for &p in &cx.lis.cfg.preds[b] {
                    let end = cx.lis.indexes.get_mbb_end_idx(p);
                    let last_use = end.get_prev_slot();
                    // The predecessor may not have a live-out value. That is
                    // OK, like an undef PHI operand.
                    if parent.live_at(&last_use) {
                        let ctx = Ctx { cfg: &cx.lis.cfg, indexes: &cx.lis.indexes, dom: &cx.lis.dom };
                        self.lical[lic].calc.extend(&mut ints[reg_idx].range, end, &ctx);
                    }
                }
            }
        }
        for li in ints {
            cx.lis.insert_interval(li);
        }
    }

    /// `rewriteAssigned`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1361>):
    /// rewrite all uses of the parent register to the register `RegAssign`
    /// gives at the operand, extending ranges where asked.
    ///
    /// DEVIATE D-SE5: one instruction names a value once for all its
    /// operands, so every operand of the parent in it goes to one register. An
    /// instruction that both reads and defines the parent (a tied pair) must
    /// find the same register at its read and at its def; the assert is the
    /// boundary.
    fn rewrite_assigned(
        &mut self,
        cx: &mut SplitCx,
        extend_ranges: bool,
    ) {
        let parent_reg = self.edit().get_reg();
        // (instruction, reads, defines) in order of first mention.
        let mut seen: Vec<(InstrKey, bool, bool)> = Vec::new();
        for op in cx.lis.operands(parent_reg) {
            match seen.iter_mut().find(|(k, ..)| *k == op.instr) {
                Some(entry) => {
                    entry.1 |= !op.is_def;
                    entry.2 |= op.is_def;
                }
                None => seen.push((op.instr, !op.is_def, op.is_def)),
            }
        }

        let mut extend: Vec<(usize, SlotIndex)> = Vec::new();
        for (key, reads, defines) in seen {
            let base = cx.lis.indexes.get_instruction_index(key);
            let block = cx.lis.indexes.get_mbb_from_index(&base);
            // Reads look up at the instruction, defs at its register slot.
            let read_idx = reads.then(|| self.reg_assign.lookup(base) as usize);
            let def_idx = defines.then(|| self.reg_assign.lookup(base.get_reg_slot(false)) as usize);
            if let (Some(r), Some(d)) = (read_idx, def_idx) {
                assert!(r == d, "a tied pair is split over registers {r} and {d} (D-SE5)");
            }
            let reg_idx = read_idx.or(def_idx).expect("an operand");
            let new_reg = self.edit().get(reg_idx);

            let old = std::sync::Arc::clone(cx.lis.instr(key));
            let new = old.renamed(&|v| if v == parent_reg { new_reg } else { v });
            cx.lis.rewrite_instr(block, key, new);

            // Extend liveness to the instruction if it reads the register.
            if extend_ranges && reads {
                extend.push((reg_idx, base.get_reg_slot(false)));
            }
        }

        if extend.is_empty() {
            return;
        }
        let regs = self.regs();
        let mut ints: Vec<LiveInterval> = regs.iter().map(|&r| cx.lis.take_interval(r)).collect();
        {
            let ctx = Ctx { cfg: &cx.lis.cfg, indexes: &cx.lis.indexes, dom: &cx.lis.dom };
            for (reg_idx, next) in extend {
                let lic = self.lic_of(reg_idx);
                self.lical[lic].calc.extend(&mut ints[reg_idx].range, next, &ctx);
            }
        }
        for li in ints {
            cx.lis.insert_interval(li);
        }
    }

    /// `deleteRematVictims`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1484>).
    ///
    /// DEVIATE D-SE6: `eliminateDeadDefs` is not translated (D-E1); only the
    /// copies this editor made whose value nothing reads are erased. The fact:
    /// without rematerialization (D-SE1) the instructions that lose every use
    /// here are the editor's own copies.
    fn delete_remat_victims(
        &mut self,
        cx: &mut SplitCx,
    ) {
        let mut dead: Vec<InstrKey> = Vec::new();
        for reg in self.regs() {
            for s in &cx.lis.get_interval(reg).segments {
                let vni = s.valno.as_ref().expect("a segment has a value");
                // Dead defs end at the dead slot.
                if s.end != vni.def().get_dead_slot() || vni.is_phi_def() {
                    continue;
                }
                let mi =
                    cx.lis.indexes.get_instruction_from_index(&vni.def()).expect("Missing instruction for dead def");
                if cx.env.is_copy_like(mi) && cx.lis.instr(mi).defines.len() == 1 {
                    dead.push(mi);
                }
            }
        }
        for mi in dead {
            let block = cx.lis.indexes.get_mbb_from_index(&cx.lis.indexes.get_instruction_index(mi));
            let reg = cx.lis.instr(mi).defines[0];
            let def = cx.lis.indexes.get_instruction_index(mi).get_reg_slot(false);
            LiveIntervals::remove_vreg_def_at(cx.lis.interval_mut(reg), &def);
            cx.lis.remove_instr(block, mi);
        }
    }

    /// `finish`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1545>):
    /// complete the split: define the original values, hoist copies, move the
    /// values to the new ranges, rewrite the operands, and weigh the new
    /// intervals. `lr_map` gets, for each register of the edit, the index it
    /// came from.
    pub fn finish(
        &mut self,
        cx: &mut SplitCx,
        vrai: &mut VirtRegAuxInfo,
        lr_map: Option<&mut Vec<usize>>,
    ) {
        self.finish_values(cx, lr_map);
        // Calculate spill weight and allocation hints for new intervals.
        for reg in self.regs() {
            let mut li = cx.lis.take_interval(reg);
            vrai.calculate_spill_weight_and_hint(&mut li, cx.lis);
            cx.lis.insert_interval(li);
        }
    }

    /// `finish` but for the weights: the values, operands and live ranges.
    pub fn finish_values(
        &mut self,
        cx: &mut SplitCx,
        lr_map: Option<&mut Vec<usize>>,
    ) {
        // At this point, the live intervals in Edit contain VNInfos
        // corresponding to the inserted copies.

        // Add the original defs from the parent interval.
        let parent = self.parent();
        for pvni in &parent.valnos {
            if pvni.is_unused() {
                continue;
            }
            let reg_idx = self.reg_assign.lookup(pvni.def()) as usize;
            self.def_value(cx, reg_idx, pvni, pvni.def(), true);
        }

        // Hoist back-copies to the complement interval when in spill mode.
        match self.spill_mode {
            // Leave all back-copies as is.
            ComplementSpillMode::Partition => {}
            ComplementSpillMode::Size | ComplementSpillMode::Speed => self.hoist_copies(cx),
        }

        // Transfer the simply mapped values, check if any are skipped.
        let skipped = self.transfer_values(cx);

        // Rewrite virtual registers, possibly extending ranges.
        self.rewrite_assigned(cx, skipped);

        if skipped {
            self.extend_phi_kill_ranges(cx);
            // Delete defs that were rematted everywhere.
            self.delete_remat_victims(cx);
        }

        // Get rid of unused values and set phi-kill flags.
        let regs = self.regs();
        for &reg in &regs {
            cx.lis.interval_mut(reg).renumber_values();
        }

        // Provide a reverse mapping from original indices to Edit ranges.
        let mut map: Vec<usize> = (0..regs.len()).collect();

        // Now check if any registers were separated into multiple components.
        for (i, &reg) in regs.iter().enumerate() {
            let split = cx.lis.split_separate_components(reg);
            for new in split {
                // The new intervals all map back to i.
                self.edit_mut().create_from(cx.lis, cx.vrm, new, reg);
                map.push(i);
            }
        }
        if let Some(out) = lr_map {
            *out = map;
        }
    }
}

/// `std::min(To, LeaveBefore)` where `LeaveBefore` may be invalid (an invalid
/// index compares below every valid one in C++ only by identity, and the
/// caller has asserted it valid on this path).
fn min_valid(
    a: SlotIndex,
    b: SlotIndex,
) -> SlotIndex {
    assert!(b.is_valid(), "this path has an interference");
    a.min(b)
}

/// `SA.getLastSplitPointIter(MBB)`: the instruction at the last split point,
/// None for the block end.
fn last_split_point_instr(
    cx: &SplitCx,
    mbb: usize,
    lsp: SlotIndex,
) -> Option<InstrKey> {
    if lsp == cx.lis.indexes.get_mbb_end_idx(mbb) { None } else { cx.lis.indexes.get_instruction_from_index(&lsp) }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::greedy::liverangeedit::NoDelegate;
    use crate::model::ir::{Held, Loc, Operation, Semantics};
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    struct Env;

    impl SplitEnv for Env {
        fn first_terminator(
            &self,
            _block: usize,
        ) -> Option<InstrKey> {
            None
        }

        fn is_loop_latch(
            &self,
            _block: usize,
        ) -> bool {
            false
        }

        fn loop_of(
            &self,
            _block: usize,
        ) -> Option<usize> {
            None
        }

        fn loop_depth(
            &self,
            _l: usize,
        ) -> u32 {
            0
        }

        fn loop_header(
            &self,
            _l: usize,
        ) -> usize {
            0
        }

        fn is_copy_like(
            &self,
            _instr: InstrKey,
        ) -> bool {
            false
        }

        fn build_copy(
            &self,
            to: u32,
            from: u32,
        ) -> Arc<Insn> {
            let what = Semantics {
                name: Some("mov".to_owned()),
                dests: vec![Loc::Held(Held { value: to, width: 2 })],
                sources: vec![Loc::Held(Held { value: from, width: 2 })],
                ..Semantics::new(Operation::Move)
            };
            Arc::new(Insn::new(0, None, Some(what), vec![to], vec![from]))
        }
    }

    fn insn(
        defines: &[u32],
        uses: &[u32],
    ) -> Arc<Insn> {
        Arc::new(Insn::new(0, None, None, defines.to_vec(), uses.to_vec()))
    }

    /// `b0: v1 = ..`, `b1: use v1; use v1`, `b2: use v1`, in a chain.
    fn three_blocks() -> (LiveIntervals, Vec<Vec<Arc<Insn>>>) {
        let (lis, insns, _) = three_blocks_body();
        (lis, insns)
    }

    fn three_blocks_body() -> (LiveIntervals, Vec<Vec<Arc<Insn>>>, LirBody) {
        let insns = vec![vec![insn(&[1], &[])], vec![insn(&[], &[1]), insn(&[], &[1])], vec![insn(&[], &[1])]];
        let lir = insns
            .iter()
            .enumerate()
            .map(|(n, i)| {
                let mut b = LirBlock::new(n as i64, i.clone());
                if n + 1 < insns.len() {
                    b.succ = vec![n as i64 + 1].into();
                }
                Block::from(b)
            })
            .collect();
        let body = LirBody::new("edit", 0, lir, IndexMap::default(), IndexMap::default());
        (LiveIntervals::new(&body), insns, body)
    }

    fn copies(
        lis: &LiveIntervals,
        block: usize,
    ) -> Vec<(u32, u32)> {
        lis.block_insns(block)
            .iter()
            .filter(|i| i.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("mov")))
            .map(|i| (i.defines[0], i.uses[0]))
            .collect()
    }

    #[test]
    fn splitting_a_block_copies_in_before_the_first_use_and_out_after_the_last() {
        let (mut lis, insns) = three_blocks();
        let mut vrm = VirtRegMap::new();
        let li = lis.interval_rc(1);
        let mut sa = SplitAnalysis::new(3);
        sa.analyze(&lis, &Env, li);
        let bi = sa.get_use_blocks()[1];
        assert_eq!((bi.mbb, bi.live_in, bi.live_out), (1, true, true));

        let mut delegate = NoDelegate;
        let mut ed = SplitEditor::new();
        ed.reset(&lis, LiveRangeEdit::new(1, &mut delegate), ComplementSpillMode::Partition);
        let mut cx = SplitCx { lis: &mut lis, vrm: &mut vrm, sa: &mut sa, env: &Env, mbfi: None };
        ed.split_single_block(&mut cx, &bi);

        let (complement, open) = (ed.edit().get(0), ed.edit().get(1));
        // `open = copy v1` before the first use, `complement = copy v1` after
        // the last.
        assert_eq!(copies(&lis, 1), vec![(open, 1), (complement, 1)]);
        let at = |k: &Arc<Insn>| lis.indexes.get_instruction_index(k.id());
        let (c_in, first, last, c_out) = {
            let b = lis.block_insns(1);
            (at(&b[0]), at(&b[1]), at(&b[2]), at(&b[3]))
        };
        assert!(c_in < first && first < last && last < c_out);
        assert_eq!(at(&insns[1][0]), first);
        // The open register holds the value from its copy to the one after the
        // last use; everywhere else is the complement.
        let r = &ed.reg_assign;
        assert_eq!(r.lookup(first), 1);
        assert_eq!(r.lookup(last), 1);
        assert_eq!(r.lookup(lis.indexes.get_mbb_start_idx(0)), 0);
        assert_eq!(r.lookup(lis.indexes.get_mbb_start_idx(2)), 0);
        // The complement's value is defined by the copy out.
        assert_eq!(lis.get_interval(complement).valnos.len(), 1);
        assert_eq!(lis.get_interval(complement).valnos[0].def(), c_out.get_reg_slot(false));
    }

    #[test]
    fn a_register_straight_through_a_block_covers_it_with_no_copy() {
        let (mut lis, _) = three_blocks();
        let mut vrm = VirtRegMap::new();
        let li = lis.interval_rc(1);
        let mut sa = SplitAnalysis::new(3);
        sa.analyze(&lis, &Env, li);
        let mut delegate = NoDelegate;
        let mut ed = SplitEditor::new();
        ed.reset(&lis, LiveRangeEdit::new(1, &mut delegate), ComplementSpillMode::Partition);
        let mut cx = SplitCx { lis: &mut lis, vrm: &mut vrm, sa: &mut sa, env: &Env, mbfi: None };
        let intv = ed.open_intv(&mut cx);
        let none = SlotIndex::default();
        ed.split_live_through_block(&mut cx, 1, intv, none, intv, none);
        assert!(copies(&lis, 1).is_empty());
        let (start, stop) = *lis.indexes.get_mbb_range(1);
        let ranges: Vec<_> = ed.reg_assign.iter().collect();
        assert_eq!(ranges, vec![(start, stop, 1)]);
    }

    #[test]
    fn spilling_on_entry_copies_to_the_complement_at_the_top() {
        let (mut lis, _) = three_blocks();
        let mut vrm = VirtRegMap::new();
        let li = lis.interval_rc(1);
        let mut sa = SplitAnalysis::new(3);
        sa.analyze(&lis, &Env, li);
        let mut delegate = NoDelegate;
        let mut ed = SplitEditor::new();
        ed.reset(&lis, LiveRangeEdit::new(1, &mut delegate), ComplementSpillMode::Partition);
        let mut cx = SplitCx { lis: &mut lis, vrm: &mut vrm, sa: &mut sa, env: &Env, mbfi: None };
        let intv = ed.open_intv(&mut cx);
        let none = SlotIndex::default();
        ed.split_live_through_block(&mut cx, 1, intv, none, 0, none);
        let complement = ed.edit().get(0);
        assert_eq!(copies(&lis, 1), vec![(complement, 1)]);
        assert_eq!(lis.block_insns(1)[0].defines, vec![complement], "the copy is the block's first instruction");
        let (start, _) = *lis.indexes.get_mbb_range(1);
        assert_eq!(ed.reg_assign.lookup(start), 1, "the open register holds the value until the copy");
    }

    #[test]
    fn overlapping_ranges_in_reg_assign_are_refused() {
        let (lis, _) = three_blocks();
        let (s0, e0) = *lis.indexes.get_mbb_range(0);
        let (_, e1) = *lis.indexes.get_mbb_range(1);
        let mut r = RegAssignMap::default();
        r.insert(s0, e0, 1);
        r.insert(e0, e1, 1);
        // Adjacent ranges of one register are one entry.
        assert_eq!(r.iter().collect::<Vec<_>>(), vec![(s0, e1, 1)]);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut r = RegAssignMap::default();
            r.insert(s0, e1, 1);
            r.insert(e0, e1, 2);
        }));
        assert!(result.is_err());
    }

    /// After `finish` no instruction names the parent, every operand names the
    /// register `RegAssign` gave it, and the live ranges built by the editor
    /// are the ones computed afresh from the edited function: a wrong range
    /// here makes the allocator interfere where nothing is live, or miss where
    /// something is.
    #[test]
    fn finish_rewrites_the_operands_and_leaves_the_ranges_computed_afresh() {
        let (mut lis, _, body) = three_blocks_body();
        let mut vrm = VirtRegMap::new();
        let li = lis.interval_rc(1);
        let mut sa = SplitAnalysis::new(3);
        sa.analyze(&lis, &Env, li);
        let bi = sa.get_use_blocks()[1];
        let mut delegate = NoDelegate;
        let mut ed = SplitEditor::new();
        ed.reset(&lis, LiveRangeEdit::new(1, &mut delegate), ComplementSpillMode::Partition);
        let mut cx = SplitCx { lis: &mut lis, vrm: &mut vrm, sa: &mut sa, env: &Env, mbfi: None };
        ed.split_single_block(&mut cx, &bi);
        let mut map = Vec::new();
        ed.finish_values(&mut cx, Some(&mut map));
        let (c0, c1) = (ed.edit().get(0), ed.edit().get(1));
        // The complement is dead between the copy in and the copy out: two
        // components, so a third register that maps back to index 0.
        assert_eq!(map, vec![0, 1, 0]);
        let c2 = ed.edit().get(2);

        assert!(lis.reg_nodbg_empty(1), "nothing names the parent any more");
        let names = |b: usize| -> Vec<(Vec<u32>, Vec<u32>)> {
            lis.block_insns(b).iter().map(|i| (i.defines.clone(), i.uses.clone())).collect()
        };
        assert_eq!(names(0), vec![(vec![c0], vec![])]);
        assert_eq!(names(1), vec![(vec![c1], vec![c0]), (vec![], vec![c1]), (vec![], vec![c1]), (vec![c2], vec![c1])]);
        assert_eq!(names(2), vec![(vec![], vec![c2])]);

        lis.remove_interval(1);
        let compared = lis.assert_intervals_equal_afresh(&lis.body_like(&body));
        assert_eq!(compared, 3, "all three new intervals were compared");
    }

    /// The spill modes copy out before the last use and recompute the ranges
    /// the copy cuts: the live ranges must still be the ones computed afresh.
    #[test]
    fn the_size_mode_recomputes_forced_values_and_still_matches_a_fresh_build() {
        for mode in [ComplementSpillMode::Size, ComplementSpillMode::Speed] {
            let (mut lis, _, body) = three_blocks_body();
            let mbfi = MachineBlockFrequencyInfo::of(&body);
            let mut vrm = VirtRegMap::new();
            let li = lis.interval_rc(1);
            let mut sa = SplitAnalysis::new(3);
            sa.analyze(&lis, &Env, li);
            let bi = sa.get_use_blocks()[1];
            let mut delegate = NoDelegate;
            let mut ed = SplitEditor::new();
            ed.reset(&lis, LiveRangeEdit::new(1, &mut delegate), mode);
            let mut cx = SplitCx { lis: &mut lis, vrm: &mut vrm, sa: &mut sa, env: &Env, mbfi: Some(&mbfi) };
            ed.split_single_block(&mut cx, &bi);
            ed.finish_values(&mut cx, None);
            assert!(lis.reg_nodbg_empty(1), "{mode:?}: nothing names the parent");
            // The copy out is redundant: the complement is defined by the
            // original def in b0 and live across, so the back-copy is removed.
            let (c0, c1) = (ed.edit().get(0), ed.edit().get(1));
            let b1 = lis.block_insns(1);
            assert_eq!(b1.len(), 3, "{mode:?}: the back-copy is gone");
            assert_eq!(b1[0].defines, vec![c1], "{mode:?}");
            assert_eq!(lis.block_insns(2)[0].uses, vec![c0], "{mode:?}");
            lis.remove_interval(1);
            let compared = lis.assert_intervals_equal_afresh(&lis.body_like(&body));
            assert_eq!(compared, 2, "{mode:?}");
        }
    }
}
