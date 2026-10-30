//! `IntEqClasses` and `ConnectedVNInfoEqClasses`: the connected components of
//! the values of a live range. Translated from llvm/lib/Support/
//! IntEqClasses.cpp, llvm/lib/CodeGen/LiveInterval.cpp and
//! llvm/lib/CodeGen/LiveRangeUtils.h at d1106deb71cc.
//!
//! # How it works
//! `classify` joins values that are one: a phi-def with the values live out of
//! its predecessors, and a def with the value live just before it (a
//! redefinition). The classes are numbered by their lowest value.
//! `distribute_range` moves the segments and values of every class but the
//! first to new ranges.
//!
//! DEVIATE D-EQ1: no sub-ranges (`Distribute`'s sub-range branch): see
//! `liverange`.
//!
//! # Example
//! `v = [0,8:0) [24,32:1)` with a hole between: two components, so
//! `classify` returns 2 and `distribute_range` leaves value 0 in the range and
//! moves `[24,32)` with value 1 to the new one.
//!
//! # Used by
//! `LiveIntervals::split_separate_components`, `SplitEditor::finish`.

use super::cfg::Cfg;
use super::liverange::{LiveRange, Vn};
use super::slotindex::SlotIndexes;

/// `IntEqClasses`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/IntEqClasses.h#L25>).
#[derive(Default)]
pub struct IntEqClasses {
    ec: Vec<u32>,
    num_classes: u32,
}

impl IntEqClasses {
    pub fn clear(&mut self) {
        self.ec.clear();
        self.num_classes = 0;
    }

    /// `grow`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/IntEqClasses.cpp#L25>).
    pub fn grow(
        &mut self,
        n: usize,
    ) {
        assert!(self.num_classes == 0, "grow() called after compress().");
        while self.ec.len() < n {
            self.ec.push(self.ec.len() as u32);
        }
    }

    /// `join`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/IntEqClasses.cpp#L32>).
    pub fn join(
        &mut self,
        mut a: u32,
        mut b: u32,
    ) -> u32 {
        assert!(self.num_classes == 0, "join() called after compress().");
        let mut eca = self.ec[a as usize];
        let mut ecb = self.ec[b as usize];
        // Update pointers while searching for the leaders, compressing the
        // paths incrementally. The larger leader will eventually be updated,
        // joining the classes.
        while eca != ecb {
            if eca < ecb {
                self.ec[b as usize] = eca;
                b = ecb;
                ecb = self.ec[b as usize];
            } else {
                self.ec[a as usize] = ecb;
                a = eca;
                eca = self.ec[a as usize];
            }
        }
        eca
    }

    /// `compress`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/IntEqClasses.cpp#L60>):
    /// number the classes in the order of their leaders.
    pub fn compress(&mut self) {
        if self.num_classes != 0 {
            return;
        }
        for i in 0..self.ec.len() {
            self.ec[i] = if self.ec[i] as usize == i {
                self.num_classes += 1;
                self.num_classes - 1
            } else {
                self.ec[self.ec[i] as usize]
            };
        }
    }

    /// `getNumClasses`.
    pub fn get_num_classes(&self) -> u32 {
        self.num_classes
    }

    /// `operator[]`.
    pub fn get(
        &self,
        a: u32,
    ) -> u32 {
        self.ec[a as usize]
    }
}

/// `ConnectedVNInfoEqClasses`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/LiveInterval.h#L1013>).
#[derive(Default)]
pub struct ConnectedVNInfoEqClasses {
    eq_class: IntEqClasses,
}

impl ConnectedVNInfoEqClasses {
    pub fn new() -> Self {
        Self::default()
    }

    /// `getEqClass`.
    pub fn get_eq_class(
        &self,
        vni: &Vn,
    ) -> u32 {
        self.eq_class.get(vni.id())
    }

    /// The class of the value numbered `id` (before `distribute_range`).
    pub fn class_of_id(
        &self,
        id: u32,
    ) -> u32 {
        self.eq_class.get(id)
    }

    /// `Classify`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveInterval.cpp#L1333>):
    /// the number of connected components of the values of `lr`.
    pub fn classify(
        &mut self,
        lr: &LiveRange,
        indexes: &SlotIndexes,
        cfg: &Cfg,
    ) -> u32 {
        // Create initial equivalence classes.
        self.eq_class.clear();
        self.eq_class.grow(lr.get_num_val_nums() as usize);

        let mut used: Option<&Vn> = None;
        let mut unused: Option<&Vn> = None;

        // Determine connections.
        for vni in &lr.valnos {
            // Group all unused values into one class.
            if vni.is_unused() {
                if let Some(unused) = unused {
                    self.eq_class.join(unused.id(), vni.id());
                }
                unused = Some(vni);
                continue;
            }
            used = Some(vni);
            if vni.is_phi_def() {
                let mbb = indexes.get_mbb_from_index(&vni.def());
                // Connect to values live out of predecessors.
                for &pred in &cfg.preds[mbb] {
                    if let Some(pvni) = lr.get_vn_info_before(&indexes.get_mbb_end_idx(pred)) {
                        self.eq_class.join(vni.id(), pvni.id());
                    }
                }
            } else if let Some(uvni) = lr.get_vn_info_before(&vni.def()) {
                // Normal value defined by an instruction. Check for two-addr
                // redef. Note that the def may be a use slot for an early
                // clobber def.
                self.eq_class.join(vni.id(), uvni.id());
            }
        }

        // Lump all the unused values in with the last used value.
        if let (Some(used), Some(unused)) = (used, unused) {
            self.eq_class.join(used.id(), unused.id());
        }

        self.eq_class.compress();
        self.eq_class.get_num_classes()
    }
}

/// `DistributeRange`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveRangeUtils.h#L26>):
/// moves the segments and values of every class but 0 of `lr` to
/// `split_lrs[class - 1]`; `classes` maps each value number to its class.
pub fn distribute_range(
    lr: &mut LiveRange,
    split_lrs: &mut [LiveRange],
    class_of: &dyn Fn(u32) -> u32,
) {
    // Move segments to new intervals.
    let mut kept = Vec::with_capacity(lr.segments.len());
    for s in lr.segments.drain(..) {
        let id = s.valno.as_ref().expect("a segment has a value").id();
        match class_of(id) {
            0 => kept.push(s),
            eq => {
                let to = &mut split_lrs[eq as usize - 1];
                assert!(to.empty() || to.expired_at(&s.start), "New intervals should be empty");
                to.segments.push(s);
            }
        }
    }
    lr.segments = kept;

    // Transfer VNInfos to their new owners and renumber them.
    let mut stay: Vec<Vn> = Vec::new();
    for vni in std::mem::take(&mut lr.valnos) {
        match class_of(vni.id()) {
            0 => {
                vni.set_id(stay.len() as u32);
                stay.push(vni);
            }
            eq => {
                let to = &mut split_lrs[eq as usize - 1];
                vni.set_id(to.valnos.len() as u32);
                to.valnos.push(vni);
            }
        }
    }
    lr.valnos = stay;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_are_numbered_by_their_lowest_member() {
        let mut ec = IntEqClasses::default();
        ec.grow(5);
        ec.join(3, 1);
        ec.join(4, 0);
        ec.compress();
        assert_eq!(ec.get_num_classes(), 3);
        assert_eq!([0, 1, 2, 3, 4].map(|i| ec.get(i)), [0, 1, 2, 1, 0]);
    }

    /// A register defined twice with nothing live between is two registers:
    /// left as one, the allocator would hold a register through the hole.
    #[test]
    fn a_register_with_a_hole_becomes_one_register_per_component() {
        use std::sync::Arc;

        use crate::liveness::liveintervals::LiveIntervals;
        use crate::model::lir::{Block, Insn, LirBlock, LirBody};
        use crate::support::hash::IndexMap;

        let insn = |d: &[u32], u: &[u32]| Arc::new(Insn::new(0, None, None, d.to_vec(), u.to_vec()));
        let block = LirBlock::new(0, vec![insn(&[1], &[]), insn(&[], &[1]), insn(&[1], &[]), insn(&[], &[1])]);
        let body = LirBody::new("eq", 0, vec![Block::from(block)], IndexMap::default(), IndexMap::default());
        let mut lis = LiveIntervals::new(&body);
        lis.interval_rc(1);
        let new = lis.split_separate_components(1);
        assert_eq!(new.len(), 1);
        let names: Vec<_> = lis.block_insns(0).iter().map(|i| (i.defines.clone(), i.uses.clone())).collect();
        assert_eq!(names, vec![(vec![1], vec![]), (vec![], vec![1]), (vec![new[0]], vec![]), (vec![], vec![new[0]])]);
        assert_eq!(lis.get_interval(1).valnos.len(), 1);
        assert_eq!(lis.get_interval(new[0]).valnos[0].id(), 0, "values are renumbered in their new range");
        assert_eq!(lis.assert_intervals_equal_afresh(&lis.body_like(&body)), 2);
        // One component: nothing to do.
        assert!(lis.split_separate_components(1).is_empty());
    }
}
