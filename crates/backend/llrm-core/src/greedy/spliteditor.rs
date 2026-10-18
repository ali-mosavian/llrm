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

use super::liverangeedit::LiveRangeEdit;
use super::splitkit::{BlockInfo, SplitAnalysis, SplitEnv, first_insert_instr};
use super::virtregmap::VirtRegMap;
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::liverange::{LiveInterval, Vn};
use crate::liveness::slotindex::{InstrKey, SlotIndex};

/// What the editor works on (the C++ editor holds references to them, which
/// Rust cannot across the allocator's own mutations).
pub struct SplitCx<'a> {
    pub lis: &'a mut LiveIntervals,
    pub vrm: &'a mut VirtRegMap,
    pub sa: &'a mut SplitAnalysis,
    pub env: &'a dyn SplitEnv,
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
        (LiveIntervals::new(&body), insns)
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
        let mut cx = SplitCx { lis: &mut lis, vrm: &mut vrm, sa: &mut sa, env: &Env };
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
        let mut cx = SplitCx { lis: &mut lis, vrm: &mut vrm, sa: &mut sa, env: &Env };
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
        let mut cx = SplitCx { lis: &mut lis, vrm: &mut vrm, sa: &mut sa, env: &Env };
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
}
