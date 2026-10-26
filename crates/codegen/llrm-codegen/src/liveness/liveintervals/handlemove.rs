//! `LiveIntervals::handleMove` and its `HMEditor`
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveIntervals.cpp#L1081-L1637>):
//! an instruction moved within its block, and the live range of each value it
//! names brought up to date without computing it afresh.
//!
//! Not translated: `handleMoveIntoNewBundle` and the bundle checks (our
//! instructions are not bundled; blocker: the bundle a paired x87 sequence
//! would make, not before the x87 route moves to MIR, 2026-12-04),
//! `updateRegMaskSlots` (no regmask slots here: a call's clobbers are not in
//! the intervals), kill and dead flags (an instruction here has none), the
//! register-unit and sub-range paths (these intervals are of virtual values
//! alone, without lanes).

use std::sync::Arc;

use super::{InstrKey, LiveIntervals};
use crate::liveness::liverange::{LiveRange, Segment, Vn};
use crate::liveness::slotindex::SlotIndex;
use crate::model::lir::Insn;

type Slot = SlotIndex;

fn earlier(
    a: &Slot,
    b: &Slot,
) -> bool {
    SlotIndex::is_earlier_instr(a, b)
}

fn same(
    a: &Slot,
    b: &Slot,
) -> bool {
    SlotIndex::is_same_instr(a, b)
}

fn vn(segment: &Segment) -> Vn {
    segment.valno.clone().expect("a segment has a value")
}

/// `std::copy(first, last, dest)`: segments `first..last` to `dest..`, in
/// order.
fn copy(
    segments: &mut [Segment],
    first: usize,
    last: usize,
    dest: usize,
) {
    for at in 0..last - first {
        segments[dest + at] = segments[first + at].clone();
    }
}

/// `std::copy_backward(first, last, dest_end)`: segments `first..last` to the
/// range that ends at `dest_end`, last one first.
fn copy_backward(
    segments: &mut [Segment],
    first: usize,
    last: usize,
    dest_end: usize,
) {
    for at in 1..=last - first {
        segments[dest_end - at] = segments[last - at].clone();
    }
}

fn max(
    a: Slot,
    b: Slot,
) -> Slot {
    if a < b { b } else { a }
}

impl LiveIntervals {
    /// `handleMove(MI)` after `MI` was moved in its block: `key` is taken out
    /// of `block` and put immediately before `before` (or at the end).
    /// Returns its new index.
    pub fn handle_move(
        &mut self,
        block: usize,
        key: InstrKey,
        before: Option<InstrKey>,
    ) -> SlotIndex {
        assert!(before != Some(key), "an instruction is not moved before itself");
        let old = self.indexes.get_instruction_index(key);
        assert!(
            self.indexes.get_mbb_start_idx(block) <= old && old < self.indexes.get_mbb_end_idx(block),
            "Cannot handle moves across basic block boundaries."
        );
        let pos = self.position_of(block, key);
        let insn = self.blocks[block].remove(pos);
        self.indexes.remove_machine_instr_from_maps(key);
        let at = before.map_or(self.blocks[block].len(), |next| self.position_of(block, next));
        let prev = if at == 0 {
            self.indexes.get_mbb_start_idx(block)
        } else {
            self.indexes.get_index_before(block, &self.blocks[block], at)
        };
        let next = match before {
            Some(next) => self.indexes.get_instruction_index(next),
            None => self.indexes.get_mbb_end_idx(block),
        };
        let new = self.indexes.insert_machine_instr_in_maps(key, block, &prev, &next, false);
        self.blocks[block].insert(at, Arc::clone(&insn));
        self.update_all_ranges(&insn, &old, &new);
        new
    }

    /// `HMEditor::updateAllRanges`: each value `one` defines or reads, once.
    fn update_all_ranges(
        &mut self,
        one: &Insn,
        old: &Slot,
        new: &Slot,
    ) {
        let mut done: Vec<u32> = Vec::new();
        for &value in one.defines.iter().chain(&one.uses) {
            if done.contains(&value) {
                continue;
            }
            done.push(value);
            // An interval not made yet is made from the body as it is: nothing
            // to update.
            if !self.has_interval(value) {
                continue;
            }
            // `updateRange`: the interval is edited beside the maps
            // `findLastUseBefore` reads.
            let mut li = self.take_interval(value);
            if earlier(old, new) {
                move_down(&mut li, old, new);
            } else {
                let last_use = |before: &Slot| self.find_last_use_before(before, value, old);
                move_up(&mut li, old, new, &last_use);
            }
            debug_assert!(li.range.verify(), "handleMove left the range of %{value} invalid");
            self.insert_interval(li);
        }
    }

    /// `HMEditor::findLastUseBefore`, of a virtual register: the last read of
    /// `value` between `before` and `old`.
    fn find_last_use_before(
        &self,
        before: &Slot,
        value: u32,
        old: &Slot,
    ) -> Slot {
        let mut last = before.clone();
        for operand in self.operands(value) {
            // An undef operand and a def are not uses.
            if operand.is_def || !operand.reads_reg {
                continue;
            }
            let at = self.indexes.get_instruction_index(operand.instr);
            if at > last && at < *old {
                last = at.get_reg_slot(false);
            }
        }
        last
    }
}

/// `HMEditor::handleMoveDown`: `lr` after an instruction moved down from `old`
/// to `new` (`old < new`).
fn move_down(
    lr: &mut LiveRange,
    old: &Slot,
    new: &Slot,
) {
    let e = lr.segments.len();
    // Segment going into OldIdx.
    let old_idx_in = lr.find(&old.get_base_index());

    // No value live before or after OldIdx? Nothing to do.
    if old_idx_in == e || earlier(old, &lr.segments[old_idx_in].start) {
        return;
    }

    let old_idx_out;
    // Do we have a value live-in to OldIdx?
    if earlier(&lr.segments[old_idx_in].start, old) {
        // If the live-in value already extends to NewIdx, there is nothing to
        // do.
        if SlotIndex::is_earlier_equal_instr(new, &lr.segments[old_idx_in].end) {
            return;
        }
        // Is there a def before NewIdx which is not OldIdx?
        let next = old_idx_in + 1;
        if next != e && !same(old, &lr.segments[next].start) && earlier(&lr.segments[next].start, new) {
            // If we are here then OldIdx was just a use but not a def. We only
            // have to ensure liveness extends to NewIdx.
            let new_idx_in = lr.advance_to(next, &new.get_base_index());
            // Extend the segment before NewIdx if necessary.
            if new_idx_in == e || !earlier(&lr.segments[new_idx_in].start, new) {
                lr.segments[new_idx_in - 1].end = new.get_reg_slot(false);
            }
            // Extend OldIdxIn.
            lr.segments[old_idx_in].end = lr.segments[next].start.clone();
            return;
        }

        // Adjust OldIdxIn->end to reach NewIdx. This may temporarily make LR
        // invalid by overlapping ranges.
        let is_kill = same(old, &lr.segments[old_idx_in].end);
        let early_clobber = lr.segments[old_idx_in].end.is_early_clobber();
        lr.segments[old_idx_in].end = new.get_reg_slot(early_clobber);
        // If this was not a kill, then there was no def and we're done.
        if !is_kill {
            return;
        }

        // Did we have a Def at OldIdx?
        old_idx_out = next;
        if old_idx_out == e || !same(old, &lr.segments[old_idx_out].start) {
            return;
        }
    } else {
        old_idx_out = old_idx_in;
    }

    // If we are here then there is a Definition at OldIdx. OldIdxOut points to
    // the segment starting there.
    assert!(old_idx_out != e && same(old, &lr.segments[old_idx_out].start), "No def?");
    let old_idx_vni = vn(&lr.segments[old_idx_out]);
    assert!(old_idx_vni.def() == lr.segments[old_idx_out].start, "Inconsistent def");

    // If the defined value extends beyond NewIdx, just move the beginning of
    // the segment to NewIdx.
    let new_idx_def = new.get_reg_slot(lr.segments[old_idx_out].start.is_early_clobber());
    if earlier(&new_idx_def, &lr.segments[old_idx_out].end) {
        old_idx_vni.set_def(new_idx_def.clone());
        lr.segments[old_idx_out].start = new_idx_def;
        return;
    }

    // If we are here then we have a Definition at OldIdx which ends before
    // NewIdx.

    // Is there an existing Def at NewIdx?
    let after_new_idx = lr.advance_to(old_idx_out, &new.get_reg_slot(false));
    let old_idx_def_is_dead = lr.segments[old_idx_out].end.is_dead();
    if !old_idx_def_is_dead && earlier(&lr.segments[old_idx_out].end, &new_idx_def) {
        // OldIdx is not a dead def, and NewIdxDef is inside a new interval.
        let def_vni: Vn;
        if old_idx_out != 0 && !earlier(&lr.segments[old_idx_out - 1].end, &lr.segments[old_idx_out].start) {
            // There is no gap between OldIdxOut and its predecessor anymore,
            // merge them.
            def_vni = old_idx_vni.clone();
            lr.segments[old_idx_out - 1].end = lr.segments[old_idx_out].end.clone();
        } else {
            // The value is live in to OldIdx. We merge OldIdxOut and its
            // successor; there is always a successor to OldIdxOut in the same
            // block. We don't need INext->valno anymore and will reuse it for
            // the new segment we create later.
            let inext = old_idx_out + 1;
            assert!(inext != e, "Must have following segment");
            def_vni = old_idx_vni.clone();
            lr.segments[inext].start = lr.segments[old_idx_out].end.clone();
            let start = lr.segments[inext].start.clone();
            vn(&lr.segments[inext]).set_def(start);
        }
        // If NewIdx is behind the last segment, extend that and append a new
        // one.
        if after_new_idx == e {
            // OldIdxOut is undef at this point, slide (OldIdxOut;AfterNewIdx]
            // up one position.
            copy(&mut lr.segments, old_idx_out + 1, e, old_idx_out);
            // The last segment is undefined now, reuse it for a dead def.
            let new_segment = e - 1;
            lr.segments[new_segment] =
                Segment::new(new_idx_def.clone(), new_idx_def.get_dead_slot(), Some(def_vni.clone()));
            def_vni.set_def(new_idx_def.clone());
            lr.segments[new_segment - 1].end = new_idx_def;
        } else {
            // Slide (OldIdxOut;AfterNewIdx] up one position.
            copy(&mut lr.segments, old_idx_out + 1, after_new_idx + 1, old_idx_out);
            let prev = after_new_idx - 1;
            if earlier(&lr.segments[prev].start, &new_idx_def) {
                // Case 1: NewIdx is inside a liverange. Split this liverange at
                // NewIdxDef into the segment "Prev" followed by "NewSegment".
                let new_segment = after_new_idx;
                let (prev_start, prev_end, prev_vn) =
                    (lr.segments[prev].start.clone(), lr.segments[prev].end.clone(), lr.segments[prev].valno.clone());
                lr.segments[new_segment] = Segment::new(new_idx_def.clone(), prev_end, prev_vn.clone());
                prev_vn.expect("a value").set_def(new_idx_def.clone());
                lr.segments[prev] = Segment::new(prev_start.clone(), new_idx_def, Some(def_vni.clone()));
                def_vni.set_def(prev_start);
            } else {
                // Case 2: NewIdx is in a lifetime hole. Keep AfterNewIdx as is
                // and turn Prev into a segment from NewIdx to
                // AfterNewIdx->start.
                let to = lr.segments[after_new_idx].start.clone();
                lr.segments[prev] = Segment::new(new_idx_def.clone(), to, Some(def_vni.clone()));
                def_vni.set_def(new_idx_def);
                assert!(!std::rc::Rc::ptr_eq(&def_vni, &vn(&lr.segments[after_new_idx])));
            }
        }
        return;
    }

    if after_new_idx != e && same(&lr.segments[after_new_idx].start, &new_idx_def) {
        // There is an existing def at NewIdx. The def at OldIdx is coalesced
        // into that value.
        assert!(!std::rc::Rc::ptr_eq(&vn(&lr.segments[after_new_idx]), &old_idx_vni), "Multiple defs of value?");
        lr.remove_val_no(&old_idx_vni);
    } else {
        // There was no existing def at NewIdx. We need to create a dead def at
        // NewIdx. Shift segments over the old OldIdxOut segment, this frees a
        // new segment at the place where we want to construct the dead def.
        assert!(after_new_idx != old_idx_out, "Inconsistent iterators");
        copy(&mut lr.segments, old_idx_out + 1, after_new_idx, old_idx_out);
        // We can reuse OldIdxVNI now.
        let new_segment = after_new_idx - 1;
        old_idx_vni.set_def(new_idx_def.clone());
        lr.segments[new_segment] = Segment::new(new_idx_def.clone(), new_idx_def.get_dead_slot(), Some(old_idx_vni));
    }
}

/// `HMEditor::handleMoveUp`: `lr` after an instruction moved up from `old` to
/// `new` (`new < old`).
fn move_up(
    lr: &mut LiveRange,
    old: &Slot,
    new: &Slot,
    find_last_use_before: &dyn Fn(&Slot) -> Slot,
) {
    let e = lr.segments.len();
    // Segment going into OldIdx.
    let mut old_idx_in = lr.find(&old.get_base_index());

    // No value live before or after OldIdx? Nothing to do.
    if old_idx_in == e || earlier(old, &lr.segments[old_idx_in].start) {
        return;
    }

    let old_idx_out;
    // Do we have a value live-in to OldIdx?
    if earlier(&lr.segments[old_idx_in].start, old) {
        // If the live-in value isn't killed here, then we have no Def at
        // OldIdx, moreover the value must be live at NewIdx so there is
        // nothing to do.
        let is_kill = same(old, &lr.segments[old_idx_in].end);
        if !is_kill {
            return;
        }

        // At this point we have to move OldIdxIn->end back to the nearest
        // previous use or (dead-)def but no further than NewIdx.
        let early_clobber = lr.segments[old_idx_in].end.is_early_clobber();
        let def_before_old_idx = max(lr.segments[old_idx_in].start.get_dead_slot(), new.get_reg_slot(early_clobber));
        lr.segments[old_idx_in].end = find_last_use_before(&def_before_old_idx);

        // Did we have a Def at OldIdx? If not we are done now.
        old_idx_out = old_idx_in + 1;
        if old_idx_out == e || !same(old, &lr.segments[old_idx_out].start) {
            return;
        }
    } else {
        old_idx_out = old_idx_in;
        // `E` is `usize::MAX` for "none" below.
        old_idx_in = if old_idx_out != 0 { old_idx_out - 1 } else { e };
    }
    let old_idx_in = old_idx_in;
    let have_in = old_idx_in != e;

    // If we are here then there is a Definition at OldIdx. OldIdxOut points to
    // the segment starting there.
    assert!(old_idx_out != e && same(old, &lr.segments[old_idx_out].start), "No def?");
    let mut old_idx_vni = vn(&lr.segments[old_idx_out]);
    assert!(old_idx_vni.def() == lr.segments[old_idx_out].start, "Inconsistent def");
    let old_idx_def_is_dead = lr.segments[old_idx_out].end.is_dead();

    // Is there an existing def at NewIdx?
    let new_idx_def = new.get_reg_slot(lr.segments[old_idx_out].start.is_early_clobber());
    let new_idx_out = lr.find(&new.get_reg_slot(false));
    if same(&lr.segments[new_idx_out].start, new) {
        assert!(
            !std::rc::Rc::ptr_eq(&vn(&lr.segments[new_idx_out]), &old_idx_vni),
            "Same value defined more than once?"
        );
        // If OldIdx was a dead def remove it.
        if !old_idx_def_is_dead {
            // Remove segment starting at NewIdx and move begin of OldIdxOut to
            // NewIdx so it can take its place.
            old_idx_vni.set_def(new_idx_def.clone());
            lr.segments[old_idx_out].start = new_idx_def;
            let gone = vn(&lr.segments[new_idx_out]);
            lr.remove_val_no(&gone);
        } else {
            // Simply remove the dead def at OldIdx.
            lr.remove_val_no(&old_idx_vni);
        }
    } else {
        // Previously nothing was live after NewIdx, so all we have to do now
        // is move the begin of OldIdxOut to NewIdx.
        if !old_idx_def_is_dead {
            // Do we have any intermediate Defs between OldIdx and NewIdx?
            if have_in && earlier(&new_idx_def, &lr.segments[old_idx_in].start) {
                // OldIdx is not a dead def and NewIdx is before predecessor
                // start.
                let new_idx_in = new_idx_out;
                assert!(new_idx_in == lr.find(&new.get_base_index()));
                let split_pos = new_idx_def.clone();
                old_idx_vni = vn(&lr.segments[old_idx_in]);

                let mut new_def_end_point = lr.segments[new_idx_in + 1].end.clone();
                if old_idx_in != 0 && earlier(new, &lr.segments[old_idx_in - 1].end) {
                    // If the segment before OldIdx read a value defined earlier
                    // than NewIdx, the moved instruction also reads and
                    // forwards that value. Extend the
                    // lifetime of the new def point.
                    //
                    // Extend to where the previous range started, unless there
                    // is another redef first.
                    let a = lr.segments[old_idx_in].start.clone();
                    let b = lr.segments[new_idx_out + 1].start.clone();
                    new_def_end_point = if b < a { b } else { a };
                }

                // Merge the OldIdxIn and OldIdxOut segments into OldIdxOut.
                let out_vn = vn(&lr.segments[old_idx_out]);
                out_vn.set_def(lr.segments[old_idx_in].start.clone());
                lr.segments[old_idx_out] = Segment::new(
                    lr.segments[old_idx_in].start.clone(),
                    lr.segments[old_idx_out].end.clone(),
                    Some(out_vn),
                );
                // OldIdxIn and OldIdxVNI are now undef and can be overridden.
                // We slide [NewIdxIn, OldIdxIn) down one position.
                copy_backward(&mut lr.segments, new_idx_in, old_idx_in, old_idx_out);
                // NewIdxIn is now considered undef so we can reuse it for the
                // moved value.
                let new_segment = new_idx_in;
                let next = new_segment + 1;
                if earlier(&lr.segments[next].start, new) {
                    // There is no gap between NewSegment and its predecessor.
                    lr.segments[new_segment] = Segment::new(
                        lr.segments[next].start.clone(),
                        split_pos.clone(),
                        lr.segments[next].valno.clone(),
                    );
                    lr.segments[next] = Segment::new(split_pos.clone(), new_def_end_point, Some(old_idx_vni.clone()));
                    old_idx_vni.set_def(split_pos);
                } else {
                    // There is a gap between NewSegment and its predecessor.
                    // Value becomes live in.
                    let to = lr.segments[next].start.clone();
                    lr.segments[new_segment] = Segment::new(split_pos.clone(), to, Some(old_idx_vni.clone()));
                    old_idx_vni.set_def(split_pos);
                }
            } else {
                // Leave the end point of a live def.
                lr.segments[old_idx_out].start = new_idx_def.clone();
                old_idx_vni.set_def(new_idx_def.clone());
                if have_in && earlier(new, &lr.segments[old_idx_in].end) {
                    lr.segments[old_idx_in].end = new_idx_def;
                }
            }
        } else if have_in
            && earlier(&lr.segments[new_idx_out].start, new)
            && earlier(new, &lr.segments[new_idx_out].end)
        {
            // OldIdxVNI is a dead def that has been moved into the middle of
            // another value in LR. The dead def may have been moved across
            // other values in LR, so move OldIdxOut up to
            // NewIdxOut. Slide [NewIdxOut;OldIdxOut) down one
            // position.
            copy_backward(&mut lr.segments, new_idx_out, old_idx_out, old_idx_out + 1);
            // Modify the segment at NewIdxOut and the following segment to meet
            // at the point of the dead def, with the following segment getting
            // OldIdxVNI as its value number.
            let meet = new_idx_def.get_reg_slot(false);
            lr.segments[new_idx_out] = Segment::new(
                lr.segments[new_idx_out].start.clone(),
                meet.clone(),
                lr.segments[new_idx_out].valno.clone(),
            );
            lr.segments[new_idx_out + 1] =
                Segment::new(meet, lr.segments[new_idx_out + 1].end.clone(), Some(old_idx_vni.clone()));
            old_idx_vni.set_def(new_idx_def);
            // Retag the segments that were shifted down from [NewIdxOut + 2,
            // OldIdxOut]. Retagging can make a segment touch another segment
            // with the same value number, so merge as we go. Stop at the
            // original end slot instead of using a segment count because
            // merging may erase segments.
            let retag_end = lr.segments[old_idx_out].end.clone();
            let mut at = new_idx_out + 2;
            while at != lr.segments.len() && lr.segments[at].start < retag_end {
                lr.segments[at].valno = Some(old_idx_vni.clone());
                at = lr.merge_adjacent_segments(at) + 1;
            }
        } else {
            // OldIdxVNI is a dead def. It may have been moved across other
            // values in LR, so move OldIdxOut up to NewIdxOut. Slide
            // [NewIdxOut;OldIdxOut) down one position.
            copy_backward(&mut lr.segments, new_idx_out, old_idx_out, old_idx_out + 1);
            // OldIdxVNI can be reused now to build a new dead def segment.
            lr.segments[new_idx_out] =
                Segment::new(new_idx_def.clone(), new_idx_def.get_dead_slot(), Some(old_idx_vni.clone()));
            old_idx_vni.set_def(new_idx_def);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::lir::{Block, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn insn(
        defines: &[u32],
        uses: &[u32],
    ) -> Arc<Insn> {
        Arc::new(Insn::new(0, None, None, defines.to_vec(), uses.to_vec()))
    }

    /// Block 0 holds `insns` and falls into block 1, which reads `live_out`.
    fn two_blocks(
        insns: &[Arc<Insn>],
        reader: &Arc<Insn>,
    ) -> LirBody {
        let first = LirBlock { succ: vec![1].into(), ..LirBlock::new(0, insns.to_vec()) };
        let second = LirBlock::new(1, vec![Arc::clone(reader)]);
        LirBody::new(
            "moved",
            0,
            vec![Block::from(first), Block::from(second)],
            IndexMap::default(),
            IndexMap::default(),
        )
    }

    struct Lcg(u64);

    impl Lcg {
        fn next(
            &mut self,
            below: usize,
        ) -> usize {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 33) as usize) % below
        }
    }

    /// An instruction that moved to another slot of its block, where no
    /// dependence is crossed, leaves every interval as a fresh computation of
    /// the moved body makes it: moves up and down, of defs read later, dead
    /// defs, reads that end a value, and values live out of the block. Without
    /// the update the intervals of the moved instruction's values are the old
    /// ones.
    #[test]
    fn test_a_moved_instruction_leaves_the_intervals_a_fresh_computation_makes() {
        let mut rng = Lcg(7);
        let mut checked = 0;
        let mut moves = 0;
        for _ in 0..300 {
            // An instruction defines a fresh value or, tied, one defined before
            // that it also reads.
            let count = 3 + rng.next(4);
            let mut shape: Vec<(u32, Vec<u32>)> = Vec::new();
            let mut values = 0u32;
            for at in 0..count {
                let tied = at > 0 && rng.next(3) == 0;
                let define = if tied { 1 + rng.next(values as usize) as u32 } else { values + 1 };
                let mut reads: Vec<u32> =
                    (0..rng.next(3)).filter(|_| values > 0).map(|_| 1 + rng.next(values as usize) as u32).collect();
                if tied {
                    reads.push(define);
                }
                values = values.max(define);
                shape.push((define, reads));
            }
            let live_out: Vec<u32> = (1..=values).filter(|_| rng.next(3) == 0).collect();
            let reader = insn(&[], &live_out);
            for from in 0..count {
                for to in 0..=count {
                    if to == from || to == from + 1 {
                        continue;
                    }
                    // `to` is the index the instruction is put before, in the
                    // block without it.
                    let mut order: Vec<usize> = (0..count).collect();
                    let moved = order.remove(from);
                    let place = if to > from { to - 1 } else { to };
                    // A dependence crossed: the value it defines is read by a
                    // crossed instruction, or it reads what a crossed one
                    // defines.
                    let (lo, hi) = if to > from { (from + 1, to) } else { (to, from) };
                    let crossed = lo..hi;
                    let defines = shape[moved].0;
                    let reads = &shape[moved].1;
                    if crossed
                        .clone()
                        .any(|k| shape[k].1.contains(&defines) || shape[k].0 == defines || reads.contains(&shape[k].0))
                    {
                        continue;
                    }
                    order.insert(place, moved);
                    let made: Vec<Arc<Insn>> = shape.iter().map(|(d, u)| insn(&[*d], u)).collect();
                    let mut lis = LiveIntervals::new(&two_blocks(&made, &reader));
                    for value in 1..=values {
                        lis.interval(value);
                    }
                    let before = (place < count - 1).then(|| made[order[place + 1]].id());
                    lis.handle_move(0, made[moved].id(), before);
                    let now: Vec<Arc<Insn>> = order.iter().map(|&k| Arc::clone(&made[k])).collect();
                    let compared = lis.assert_intervals_equal_afresh(&two_blocks(&now, &reader));
                    assert!(compared > 0);
                    checked += compared;
                    moves += 1;
                }
            }
        }
        assert!(moves > 500 && checked > 2000, "{moves} moves, {checked} intervals compared");
    }
}
