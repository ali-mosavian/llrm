//! `VNInfo`, `LiveRange::Segment`, `LiveRange`, `LiveInterval`,
//! `LiveQueryResult` and `LiveRangeUpdater`, translated from
//! llvm/include/llvm/CodeGen/LiveInterval.h and llvm/lib/CodeGen/LiveInterval.
//! cpp at d1106deb71cc.
//!
//! # How it works
//! A live range is an ordered list of segments `[start, end)` of slot indexes,
//! each carrying the value number (`VNInfo`) that is live in it, plus the list
//! of value numbers. Where a new value is defined, or different values reach a
//! join, a new segment with a new value number starts, so a range is in SSA
//! form. A `LiveInterval` is a range for one virtual register with a spill
//! weight.
//!
//! ```text
//!   %5  [16r,48r:0)[64r,80d:1)   0@16r 1@64r   weight:3.5
//! ```
//!
//! DEVIATE (container, no number): `VNInfo*` is `Rc<VNInfo>` with `Cell`/
//! `RefCell` fields (`id` and `def` change in `join`, `RenumberValues`,
//! `MergeValueNumberInto`); iterators are indices into `segments`. NOT
//! translated: sub-register ranges (`SubRange`, `refineSubRanges`,
//! `computeSubRangeUndefs`; x86's AL/AH are register units of our register
//! classes, not lanes of one virtual register), the `SegmentSet` mode of
//! `LiveRange` (it serves `computeRegUnitRange` of physical registers),
//! `findIndexesLiveAt` (S2: it comes with SplitKit); `ConnectedVNInfoEqClasses`
//! is in `eqclasses`. `extendSegmentStartTo` returns the index of the extended
//! segment where the C++ returns an iterator that its own `erase` invalidated.
//!
//! # Example
//! `a = 1` at 16, `b = a + 2` at 32, a read of `a` at 48: `a` is
//! `[16r,48r:0)`, `b` is `[32r,48d:0)` if never read.
//!
//! # Used by
//! `liveintervalunion` and the allocator's shadow core; the coalescer's
//! `JoinVals`.

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::fmt;
use std::rc::Rc;

use super::slotindex::{InstrKey, SlotIndex, SlotIndexes};

thread_local! {
    static VERIFIES: Cell<usize> = const { Cell::new(0) };
}

/// How many times this thread has verified a range, for a test that a join
/// does not (LLVM's `assert(verify())` is compiled out of a release build).
pub fn verifies() -> usize {
    VERIFIES.with(Cell::get)
}

/// Whether the ranges are verified after each edit: with
/// `LLRM_CHECK_LIVEINTERVALS`. The check is linear in the range, so run at
/// every join it makes the coalescer quadratic.
fn checking() -> bool {
    llrm_support::env_flag!("LLRM_CHECK_LIVEINTERVALS")
}

/// `VNInfo` (LiveInterval.h#L52): value number information, a value's
/// definition point.
pub struct VNInfo {
    id: Cell<u32>,
    def: RefCell<SlotIndex>,
}

/// A `VNInfo*`: a value number is its identity (`Rc::ptr_eq`).
pub type Vn = Rc<VNInfo>;

impl VNInfo {
    pub fn new(
        id: u32,
        def: SlotIndex,
    ) -> Vn {
        Rc::new(Self { id: Cell::new(id), def: RefCell::new(def) })
    }

    /// `VNInfo(unsigned i, const VNInfo &orig)`: copies the definition, takes a
    /// new number.
    pub fn copy_of(
        id: u32,
        orig: &VNInfo,
    ) -> Vn {
        Self::new(id, orig.def())
    }

    pub fn id(&self) -> u32 {
        self.id.get()
    }

    pub(crate) fn set_id(
        &self,
        id: u32,
    ) {
        self.id.set(id);
    }

    pub fn def(&self) -> SlotIndex {
        self.def.borrow().clone()
    }

    pub fn set_def(
        &self,
        def: SlotIndex,
    ) {
        *self.def.borrow_mut() = def;
    }

    /// `copyFrom`: the definition of `src`.
    pub fn copy_from(
        &self,
        src: &VNInfo,
    ) {
        self.set_def(src.def());
    }

    /// Defined by a PHI instruction (or was): PHI-defs begin at a block
    /// boundary, all other defs at register or early-clobber slots.
    pub fn is_phi_def(&self) -> bool {
        self.def.borrow().is_block()
    }

    pub fn is_unused(&self) -> bool {
        !self.def.borrow().is_valid()
    }

    pub fn mark_unused(&self) {
        self.set_def(SlotIndex::default());
    }
}

impl fmt::Display for VNInfo {
    /// `VNInfo::print` (LiveInterval.cpp#L1043).
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "{}@", self.id())?;
        if self.is_unused() {
            write!(f, "x")
        } else {
            write!(f, "{}", self.def())?;
            if self.is_phi_def() { write!(f, "-phi") } else { Ok(()) }
        }
    }
}

fn same_vn(
    a: &Option<Vn>,
    b: &Option<Vn>,
) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// `LiveQueryResult` (LiveInterval.h#L97): the result of a `LiveRange` query.
pub struct LiveQueryResult {
    early_val: Option<Vn>,
    late_val: Option<Vn>,
    end_point: SlotIndex,
    kill: bool,
}

impl LiveQueryResult {
    pub fn new(
        early_val: Option<Vn>,
        late_val: Option<Vn>,
        end_point: SlotIndex,
        kill: bool,
    ) -> Self {
        Self { early_val, late_val, end_point, kill }
    }

    /// The value live-in to the instruction.
    pub fn value_in(&self) -> Option<Vn> {
        self.early_val.clone()
    }

    pub fn is_kill(&self) -> bool {
        self.kill
    }

    pub fn is_dead_def(&self) -> bool {
        self.end_point.is_valid() && self.end_point.is_dead()
    }

    /// The value leaving the instruction; a dead def returns None.
    pub fn value_out(&self) -> Option<Vn> {
        if self.is_dead_def() { None } else { self.late_val.clone() }
    }

    pub fn value_out_or_dead(&self) -> Option<Vn> {
        self.late_val.clone()
    }

    /// The value defined by this instruction, if any (dead defs included).
    pub fn value_defined(&self) -> Option<Vn> {
        if same_vn(&self.early_val, &self.late_val) { None } else { self.late_val.clone() }
    }

    pub fn end_point(&self) -> SlotIndex {
        self.end_point.clone()
    }
}

/// `LiveRange::Segment` (LiveInterval.h#L146): `[start, end)` with the value
/// live in it.
#[derive(Clone, Default)]
pub struct Segment {
    pub start: SlotIndex,
    pub end: SlotIndex,
    pub valno: Option<Vn>,
}

impl Segment {
    pub fn new(
        start: SlotIndex,
        end: SlotIndex,
        valno: Option<Vn>,
    ) -> Self {
        assert!(start < end, "Cannot create empty or backwards segment");
        Self { start, end, valno }
    }

    pub fn contains(
        &self,
        i: &SlotIndex,
    ) -> bool {
        self.start <= *i && *i < self.end
    }

    pub fn contains_interval(
        &self,
        s: &SlotIndex,
        e: &SlotIndex,
    ) -> bool {
        assert!(s < e, "Backwards interval?");
        (self.start <= *s && *s < self.end) && (self.start < *e && *e <= self.end)
    }

    fn vn(&self) -> &Vn {
        self.valno.as_ref().expect("a segment has a value")
    }
}

/// Segments compare by (start, end) alone, as in C++.
impl PartialEq for Segment {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.start == other.start && self.end == other.end
    }
}

impl Eq for Segment {}

impl PartialOrd for Segment {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Segment {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        (&self.start, &self.end).cmp(&(&other.start, &other.end))
    }
}

impl fmt::Display for Segment {
    /// `operator<<(raw_ostream&, const LiveRange::Segment&)`
    /// (LiveInterval.cpp#L1036).
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "[{},{}:{})", self.start, self.end, self.valno.as_ref().map_or(u32::MAX, |v| v.id()))
    }
}

impl fmt::Debug for Segment {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// What `overlaps(Other, CP, Indexes)` asks of a coalescer pair
/// (RegisterCoalescer.h `CoalescerPair::isCoalescable`).
pub trait CoalescerPair {
    fn is_coalescable(
        &self,
        instr: Option<InstrKey>,
    ) -> bool;
}

/// `LiveRange` (LiveInterval.h#L163): the liveness of a register, stack slot,
/// etc., as an ordered list of segments.
#[derive(Clone, Default)]
pub struct LiveRange {
    pub segments: Vec<Segment>,
    pub valnos: Vec<Vn>,
}

impl LiveRange {
    pub fn new() -> Self {
        Self::default()
    }

    /// `assign` (LiveInterval.h#L215): copies value numbers and segments.
    pub fn assign(
        &mut self,
        other: &LiveRange,
    ) {
        for vni in &other.valnos {
            self.create_value_copy(vni);
        }
        for s in &other.segments {
            self.segments.push(Segment::new(
                s.start.clone(),
                s.end.clone(),
                Some(self.valnos[s.vn().id() as usize].clone()),
            ));
        }
    }

    /// `advanceTo` (LiveInterval.h#L233): the segment containing `pos`, or the
    /// one after the hole it is in, or `len()` past the end.
    pub fn advance_to(
        &self,
        mut i: usize,
        pos: &SlotIndex,
    ) -> usize {
        assert!(i != self.segments.len());
        if *pos >= self.end_index() {
            return self.segments.len();
        }
        while self.segments[i].end <= *pos {
            i += 1;
        }
        i
    }

    /// `LiveRange::find` (LiveInterval.cpp#L350): the first segment that ends
    /// after `pos`, or `len()`.
    pub fn find(
        &self,
        pos: &SlotIndex,
    ) -> usize {
        self.segments.partition_point(|x| x.end <= *pos)
    }

    pub fn clear(&mut self) {
        self.valnos.clear();
        self.segments.clear();
    }

    pub fn size(&self) -> usize {
        self.segments.len()
    }

    pub fn has_at_least_one_value(&self) -> bool {
        !self.valnos.is_empty()
    }

    pub fn contains_one_value(&self) -> bool {
        self.valnos.len() == 1
    }

    pub fn get_num_val_nums(&self) -> u32 {
        self.valnos.len() as u32
    }

    pub fn get_val_num_info(
        &self,
        val_no: u32,
    ) -> Vn {
        self.valnos[val_no as usize].clone()
    }

    pub fn contains_value(
        &self,
        vni: &Option<Vn>,
    ) -> bool {
        vni.as_ref().is_some_and(|vni| {
            (vni.id() as usize) < self.valnos.len() && Rc::ptr_eq(vni, &self.valnos[vni.id() as usize])
        })
    }

    /// `getNextValue`: a new value number defined at `def`.
    pub fn get_next_value(
        &mut self,
        def: SlotIndex,
    ) -> Vn {
        let vni = VNInfo::new(self.valnos.len() as u32, def);
        self.valnos.push(Rc::clone(&vni));
        vni
    }

    /// `createValueCopy`: identical except for the value number.
    pub fn create_value_copy(
        &mut self,
        orig: &VNInfo,
    ) -> Vn {
        let vni = VNInfo::copy_of(self.valnos.len() as u32, orig);
        self.valnos.push(Rc::clone(&vni));
        vni
    }

    pub fn empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn begin_index(&self) -> SlotIndex {
        assert!(!self.empty(), "Call to beginIndex() on empty range.");
        self.segments[0].start.clone()
    }

    pub fn end_index(&self) -> SlotIndex {
        assert!(!self.empty(), "Call to endIndex() on empty range.");
        self.segments.last().expect("a segment").end.clone()
    }

    pub fn expired_at(
        &self,
        index: &SlotIndex,
    ) -> bool {
        *index >= self.end_index()
    }

    pub fn live_at(
        &self,
        index: &SlotIndex,
    ) -> bool {
        let r = self.find(index);
        r != self.segments.len() && self.segments[r].start <= *index
    }

    /// `FindSegmentContaining`: the segment that contains `idx`, or `len()`.
    pub fn find_segment_containing(
        &self,
        idx: &SlotIndex,
    ) -> usize {
        let i = self.find(idx);
        if i != self.segments.len() && self.segments[i].start <= *idx { i } else { self.segments.len() }
    }

    pub fn get_segment_containing(
        &self,
        idx: &SlotIndex,
    ) -> Option<&Segment> {
        self.segments.get(self.find_segment_containing(idx))
    }

    /// `getVNInfoAt`: the value live at `idx`, or None.
    pub fn get_vn_info_at(
        &self,
        idx: &SlotIndex,
    ) -> Option<Vn> {
        self.segments.get(self.find_segment_containing(idx)).and_then(|s| s.valno.clone())
    }

    /// `getVNInfoBefore`: the value live up to but not necessarily including
    /// `idx`: the reaching def used by an instruction at `idx`.
    pub fn get_vn_info_before(
        &self,
        idx: &SlotIndex,
    ) -> Option<Vn> {
        self.segments.get(self.find_segment_containing(&idx.get_prev_slot())).and_then(|s| s.valno.clone())
    }

    /// `overlaps(const LiveRange&)`.
    pub fn overlaps(
        &self,
        other: &LiveRange,
    ) -> bool {
        if self.empty() || other.empty() {
            return false;
        }
        self.overlaps_from(other, 0)
    }

    /// `overlapsFrom` (LiveInterval.cpp#L389): the intersection of the two live
    /// ranges is not empty; `start_pos` is a hint into `other`.
    pub fn overlaps_from(
        &self,
        other: &LiveRange,
        start_pos: usize,
    ) -> bool {
        assert!(!self.empty(), "empty range");
        let (mut a, mut b): (&[Segment], &[Segment]) = (&self.segments, &other.segments);
        let (mut i, mut j) = (0usize, start_pos);
        assert!((b[j].start <= a[i].start || start_pos == 0) && start_pos != b.len(), "Bogus start position hint!");
        if a[i].start < b[j].start {
            i += a[i..].partition_point(|s| s.start <= b[j].start);
            if i != 0 {
                i -= 1;
            }
        } else if b[j].start < a[i].start {
            let next = start_pos + 1;
            if next != b.len() && b[next].start <= a[i].start {
                j += b[j..].partition_point(|s| s.start <= a[i].start);
                if j != 0 {
                    j -= 1;
                }
            }
        } else {
            return true;
        }
        if j == b.len() {
            return false;
        }
        while i != a.len() {
            if a[i].start > b[j].start {
                std::mem::swap(&mut i, &mut j);
                std::mem::swap(&mut a, &mut b);
            }
            if a[i].end > b[j].start {
                return true;
            }
            i += 1;
        }
        false
    }

    /// `overlaps(Other, CP, Indexes)` (LiveInterval.cpp#L430): overlapping
    /// segments that are not coalescable according to `cp`.
    pub fn overlaps_cp(
        &self,
        other: &LiveRange,
        cp: &dyn CoalescerPair,
        indexes: &SlotIndexes,
    ) -> bool {
        assert!(!self.empty(), "empty range");
        if other.empty() {
            return false;
        }
        // Use binary searches to find initial positions.
        let (mut a, mut b): (&LiveRange, &LiveRange) = (self, other);
        let mut i = a.find(&other.begin_index());
        if i == a.segments.len() {
            return false;
        }
        let mut j = b.find(&a.segments[i].start);
        if j == b.segments.len() {
            return false;
        }
        loop {
            // J has just been advanced to satisfy:
            assert!(b.segments[j].end > a.segments[i].start);
            // Check for an overlap.
            if b.segments[j].start < a.segments[i].end {
                // I and J are overlapping. Find the later start.
                let def = std::cmp::max(a.segments[i].start.clone(), b.segments[j].start.clone());
                // Allow the overlap if Def is a coalescable copy.
                if def.is_block() || !cp.is_coalescable(indexes.get_instruction_from_index(&def)) {
                    return true;
                }
            }
            // Advance the iterator that ends first to check for more overlaps.
            if b.segments[j].end > a.segments[i].end {
                std::mem::swap(&mut i, &mut j);
                std::mem::swap(&mut a, &mut b);
            }
            // Advance J until J->end > I->start.
            loop {
                j += 1;
                if j == b.segments.len() {
                    return false;
                }
                if b.segments[j].end > a.segments[i].start {
                    break;
                }
            }
        }
    }

    /// `overlaps(Start, End)` (LiveInterval.cpp#L473): overlaps `[start, end)`.
    pub fn overlaps_interval(
        &self,
        start: &SlotIndex,
        end: &SlotIndex,
    ) -> bool {
        assert!(start < end, "Invalid range");
        // lower_bound(*this, End): the first segment whose start is not below
        // End.
        let i = self.segments.partition_point(|s| s.start < *end);
        i != 0 && self.segments[i - 1].end > *start
    }

    /// `covers` (LiveInterval.cpp#L479): all segments of `other` are
    /// completely covered by this range.
    pub fn covers(
        &self,
        other: &LiveRange,
    ) -> bool {
        if self.empty() {
            return other.empty();
        }
        let mut i = 0usize;
        for o in &other.segments {
            i = self.advance_to_or_end(i, &o.start);
            if i == self.segments.len() || self.segments[i].start > o.start {
                return false;
            }
            // Check adjacent live segments and see if we can get behind O.end.
            while self.segments[i].end < o.end {
                let last = i;
                // Get next segment and abort if it was not adjacent.
                i += 1;
                if i == self.segments.len() || self.segments[last].end != self.segments[i].start {
                    return false;
                }
            }
        }
        true
    }

    fn advance_to_or_end(
        &self,
        i: usize,
        pos: &SlotIndex,
    ) -> usize {
        if i == self.segments.len() { i } else { self.advance_to(i, pos) }
    }

    /// `markValNoForDeletion` (LiveInterval.cpp#L504): `val_no` is dead; the
    /// largest number is dropped (and any unused ones that neighbour it), any
    /// other is marked unused to be dropped later.
    fn mark_val_no_for_deletion(
        &mut self,
        val_no: &Vn,
    ) {
        if val_no.id() == self.get_num_val_nums() - 1 {
            loop {
                self.valnos.pop();
                if !self.valnos.last().is_some_and(|v| v.is_unused()) {
                    break;
                }
            }
        } else {
            val_no.mark_unused();
        }
    }

    /// `RenumberValues` (LiveInterval.cpp#L516): renumbers all values in order
    /// of appearance and removes unused values.
    pub fn renumber_values(&mut self) {
        let mut seen: Vec<*const VNInfo> = Vec::new();
        self.valnos.clear();
        for s in &self.segments {
            let vni = s.vn();
            if seen.contains(&Rc::as_ptr(vni)) {
                continue;
            }
            seen.push(Rc::as_ptr(vni));
            assert!(!vni.is_unused(), "Unused valno used by live segment");
            vni.set_id(self.valnos.len() as u32);
            self.valnos.push(Rc::clone(vni));
        }
    }

    // ---- CalcLiveRangeUtilBase / CalcLiveRangeUtilVector
    // (LiveInterval.cpp#L65) ----

    /// `createDeadDef(SlotIndex, Allocator)` (LiveInterval.cpp#L355): makes
    /// sure the range has a value defined at `def`.
    pub fn create_dead_def(
        &mut self,
        def: SlotIndex,
    ) -> Vn {
        self.create_dead_def_for(def, None)
    }

    /// `createDeadDef(VNInfo*)` (LiveInterval.cpp#L363).
    pub fn create_dead_def_vn(
        &mut self,
        vni: &Vn,
    ) -> Vn {
        self.create_dead_def_for(vni.def(), Some(vni.clone()))
    }

    /// `CalcLiveRangeUtilBase::createDeadDef` (LiveInterval.cpp#L87).
    fn create_dead_def_for(
        &mut self,
        mut def: SlotIndex,
        for_vni: Option<Vn>,
    ) -> Vn {
        assert!(!def.is_dead(), "Cannot define a value at the dead slot");
        assert!(for_vni.as_ref().is_none_or(|v| v.def() == def), "If ForVNI is specified, it must match Def");
        let i = self.find(&def);
        if i == self.segments.len() {
            let vni = for_vni.unwrap_or_else(|| self.get_next_value(def.clone()));
            self.segments.push(Segment::new(def.clone(), def.get_dead_slot(), Some(vni.clone())));
            return vni;
        }
        if SlotIndex::is_same_instr(&def, &self.segments[i].start) {
            assert!(for_vni.as_ref().is_none_or(|v| Rc::ptr_eq(v, self.segments[i].vn())), "Value number mismatch");
            assert!(self.segments[i].vn().def() == self.segments[i].start, "Inconsistent existing value def");
            // It is possible to have both normal and early-clobber defs of the
            // same register on an instruction. Just convert
            // everything to early-clobber.
            def = std::cmp::min(def, self.segments[i].start.clone());
            if def != self.segments[i].start {
                self.segments[i].start = def.clone();
                self.segments[i].vn().set_def(def);
            }
            return self.segments[i].vn().clone();
        }
        assert!(SlotIndex::is_earlier_instr(&def, &self.segments[i].start), "Already live at def");
        let vni = for_vni.unwrap_or_else(|| self.get_next_value(def.clone()));
        self.segments.insert(i, Segment::new(def.clone(), def.get_dead_slot(), Some(vni.clone())));
        vni
    }

    /// `findInsertPos` of the vector variant: the first segment whose start is
    /// after `s.start` (`upper_bound`).
    fn find_insert_pos(
        &self,
        start: &SlotIndex,
    ) -> usize {
        self.segments.partition_point(|x| x.start <= *start)
    }

    /// `extendInBlock(StartIdx, Use)` (LiveInterval.cpp#L111; the public one at
    /// #L580).
    pub fn extend_in_block(
        &mut self,
        start_idx: &SlotIndex,
        use_: &SlotIndex,
    ) -> Option<Vn> {
        if self.segments.is_empty() {
            return None;
        }
        // Segments starting at or before the slot before `use_` start before
        // `use_`: the slots are consecutive, and the previous one is not built.
        let mut i = self.segments.partition_point(|x| x.start < *use_);
        if i == 0 {
            return None;
        }
        i -= 1;
        if self.segments[i].end <= *start_idx {
            return None;
        }
        if self.segments[i].end < *use_ {
            self.extend_segment_end_to(i, use_);
        }
        self.segments[i].valno.clone()
    }

    /// `extendInBlock(Undefs, StartIdx, Use)` (LiveInterval.cpp#L125; public at
    /// #L571).
    pub fn extend_in_block_undefs(
        &mut self,
        undefs: &[SlotIndex],
        start_idx: &SlotIndex,
        use_: &SlotIndex,
    ) -> (Option<Vn>, bool) {
        if self.segments.is_empty() {
            return (None, false);
        }
        let before_use = use_.get_prev_slot();
        let mut i = self.find_insert_pos(&before_use);
        if i == 0 {
            return (None, self.is_undef_in(undefs, start_idx, &before_use));
        }
        i -= 1;
        if self.segments[i].end <= *start_idx {
            return (None, self.is_undef_in(undefs, start_idx, &before_use));
        }
        if self.segments[i].end < *use_ {
            if self.is_undef_in(undefs, &self.segments[i].end, &before_use) {
                return (None, true);
            }
            self.extend_segment_end_to(i, use_);
        }
        (self.segments[i].valno.clone(), false)
    }

    /// `isUndefIn`: an explicit "undef" between `begin` and `end`.
    pub fn is_undef_in(
        &self,
        undefs: &[SlotIndex],
        begin: &SlotIndex,
        end: &SlotIndex,
    ) -> bool {
        undefs.iter().any(|idx| *begin <= *idx && *idx < *end)
    }

    /// `extendSegmentEndTo` (LiveInterval.cpp#L158): extends segment `i` to end
    /// at `new_end`, merging and eliminating the segments this overlaps.
    fn extend_segment_end_to(
        &mut self,
        i: usize,
        new_end: &SlotIndex,
    ) {
        assert!(i != self.segments.len(), "Not a valid segment!");
        let val_no = self.segments[i].valno.clone();
        // Search for the first segment that we can't merge with.
        let mut merge_to = i + 1;
        while merge_to != self.segments.len() && *new_end >= self.segments[merge_to].end {
            assert!(same_vn(&self.segments[merge_to].valno, &val_no), "Cannot merge with differing values!");
            merge_to += 1;
        }
        // If NewEnd was in the middle of a segment, make sure to get its
        // endpoint.
        self.segments[i].end = std::cmp::max(new_end.clone(), self.segments[merge_to - 1].end.clone());
        // If the newly formed segment now touches the segment after it and if
        // they have the same value number, merge the two segments into
        // one segment.
        if merge_to != self.segments.len()
            && self.segments[merge_to].start <= self.segments[i].end
            && same_vn(&self.segments[merge_to].valno, &val_no)
        {
            self.segments[i].end = self.segments[merge_to].end.clone();
            merge_to += 1;
        }
        // Erase any dead segments.
        self.segments.drain(i + 1..merge_to);
    }

    /// `extendSegmentStartTo` (LiveInterval.cpp#L186): extends segment `i` to
    /// start at `new_start`, returns the index of the segment that results.
    ///
    /// C++ BUG NOT COPIED: where the loop reaches the first segment, C++ erases
    /// `[begin, I)` and then returns `I`, an iterator that erase invalidated
    /// (it points past the segment that moved to the front). The caller
    /// then reads `I->end`. Here the index of the segment after the erase
    /// is returned.
    fn extend_segment_start_to(
        &mut self,
        i: usize,
        new_start: &SlotIndex,
    ) -> usize {
        assert!(i != self.segments.len(), "Not a valid segment!");
        let val_no = self.segments[i].valno.clone();
        let s_end = self.segments[i].end.clone();
        // Search for the first segment that we can't merge with.
        let mut merge_to = i;
        loop {
            if merge_to == 0 {
                self.segments[i].start = new_start.clone();
                self.segments.drain(merge_to..i);
                return merge_to;
            }
            assert!(same_vn(&self.segments[merge_to].valno, &val_no), "Cannot merge with differing values!");
            merge_to -= 1;
            if *new_start > self.segments[merge_to].start {
                break;
            }
        }
        // If we start in the middle of another segment, just delete a range and
        // extend that segment.
        if self.segments[merge_to].end >= *new_start && same_vn(&self.segments[merge_to].valno, &val_no) {
            self.segments[merge_to].end = s_end;
        } else {
            // Otherwise, extend the segment right after.
            merge_to += 1;
            self.segments[merge_to].start = new_start.clone();
            self.segments[merge_to].end = s_end;
        }
        self.segments.drain(merge_to + 1..=i);
        merge_to
    }

    /// `addSegment` (LiveInterval.cpp#L219, public #L533): adds `s`, merging
    /// segments as appropriate; returns the index of the inserted segment.
    pub fn add_segment(
        &mut self,
        s: Segment,
    ) -> usize {
        let (start, end) = (s.start.clone(), s.end.clone());
        let mut i = self.find_insert_pos(&s.start);
        // If the inserted segment starts in the middle or right at the end of
        // another segment, just extend that segment to contain the segment of
        // S.
        if i != 0 {
            let b = i - 1;
            if same_vn(&s.valno, &self.segments[b].valno) {
                if self.segments[b].start <= start && self.segments[b].end >= start {
                    self.extend_segment_end_to(b, &end);
                    return b;
                }
            } else {
                // Check to make sure that we are not overlapping two live
                // segments with different valno's.
                assert!(self.segments[b].end <= start, "Cannot overlap two segments with differing ValID's");
            }
        }
        // Otherwise, if this segment ends in the middle of, or right next to,
        // another segment, merge it into that segment.
        if i != self.segments.len() {
            if same_vn(&s.valno, &self.segments[i].valno) {
                if self.segments[i].start <= end {
                    i = self.extend_segment_start_to(i, &start);
                    // If S is a complete superset of a segment, we may need to
                    // grow its endpoint as well.
                    if end > self.segments[i].end {
                        self.extend_segment_end_to(i, &end);
                    }
                    return i;
                }
            } else {
                // Check to make sure that we are not overlapping two live
                // segments with different valno's.
                assert!(self.segments[i].start >= end, "Cannot overlap two segments with differing ValID's");
            }
        }
        // Otherwise, this is just a new segment that doesn't interact with
        // anything.
        self.segments.insert(i, s);
        i
    }

    /// `mergeAdjacentSegments` (LiveInterval.cpp#L543): merges segment `i` with
    /// its neighbours that have the same value number and touch it.
    pub fn merge_adjacent_segments(
        &mut self,
        mut i: usize,
    ) -> usize {
        assert!(i != self.segments.len(), "Cannot merge end iterator");
        if i != 0 {
            let prev = i - 1;
            if same_vn(&self.segments[prev].valno, &self.segments[i].valno)
                && self.segments[prev].end == self.segments[i].start
            {
                self.segments[prev].end = self.segments[i].end.clone();
                self.segments.remove(i);
                i = prev;
            }
        }
        let next = i + 1;
        if next != self.segments.len()
            && same_vn(&self.segments[i].valno, &self.segments[next].valno)
            && self.segments[i].end == self.segments[next].start
        {
            self.segments[i].end = self.segments[next].end.clone();
            self.segments.remove(next);
        }
        i
    }

    /// `append` (LiveInterval.cpp#L565): a segment that belongs at the back.
    pub fn append(
        &mut self,
        s: Segment,
    ) {
        assert!(self.segments.is_empty() || self.segments.last().expect("a segment").end <= s.start);
        self.segments.push(s);
    }

    /// `removeSegment(Start, End, RemoveDeadValNo)` (LiveInterval.cpp#L588).
    pub fn remove_segment(
        &mut self,
        start: &SlotIndex,
        end: &SlotIndex,
        remove_dead_val_no: bool,
    ) {
        // Find the Segment containing this span.
        let i = self.find(start);
        // No Segment found, so nothing to do.
        if i == self.segments.len() {
            return;
        }
        assert!(self.segments[i].contains_interval(start, end), "Segment is not entirely in range!");
        // If the span we are removing is at the start of the Segment, adjust
        // it.
        let val_no = self.segments[i].valno.clone();
        if self.segments[i].start == *start {
            if self.segments[i].end == *end {
                self.segments.remove(i); // Removed the whole Segment.
                if remove_dead_val_no {
                    self.remove_val_no_if_dead(&val_no.expect("a value"));
                }
            } else {
                self.segments[i].start = end.clone();
            }
            return;
        }
        // Otherwise if the span we are removing is at the end of the Segment,
        // adjust the other way.
        if self.segments[i].end == *end {
            self.segments[i].end = start.clone();
            return;
        }
        // Otherwise, we are splitting the Segment into two pieces.
        let old_end = self.segments[i].end.clone();
        self.segments[i].end = start.clone(); // Trim the old segment.
        // Insert the new one.
        self.segments.insert(i + 1, Segment::new(end.clone(), old_end, val_no));
    }

    /// `removeSegment(iterator, RemoveDeadValNo)` (LiveInterval.cpp#L628):
    /// returns the index of the segment after the removed one.
    pub fn remove_segment_at(
        &mut self,
        i: usize,
        remove_dead_val_no: bool,
    ) -> usize {
        let val_no = self.segments[i].valno.clone();
        self.segments.remove(i);
        if remove_dead_val_no {
            self.remove_val_no_if_dead(&val_no.expect("a value"));
        }
        i
    }

    /// `removeValNoIfDead` (LiveInterval.cpp#L636).
    pub fn remove_val_no_if_dead(
        &mut self,
        val_no: &Vn,
    ) {
        if !self.segments.iter().any(|s| same_vn(&s.valno, &Some(val_no.clone()))) {
            self.mark_val_no_for_deletion(val_no);
        }
    }

    /// `removeValNo` (LiveInterval.cpp#L643): removes all segments of `val_no`
    /// and the value number.
    pub fn remove_val_no(
        &mut self,
        val_no: &Vn,
    ) {
        if self.empty() {
            return;
        }
        self.segments.retain(|s| !same_vn(&s.valno, &Some(val_no.clone())));
        // Now that ValNo is dead, remove it.
        self.mark_val_no_for_deletion(val_no);
    }

    /// `join` (LiveInterval.cpp#L651): joins `other` into this range with the
    /// value-number mappings given. `new_vn_info[i]` is None where C++ has
    /// null.
    pub fn join(
        &mut self,
        other: &mut LiveRange,
        lhs_val_no_assignments: &[i32],
        rhs_val_no_assignments: &[i32],
        new_vn_info: &mut Vec<Option<Vn>>,
    ) {
        if checking() {
            assert!(self.verify());
            assert!(other.verify());
        }
        // Determine if any of our values are mapped.  This is uncommon, so we
        // want to avoid the range scan if not.
        let mut must_map_cur_val_nos = false;
        let num_vals = self.get_num_val_nums() as usize;
        let num_new_vals = new_vn_info.len();
        for i in 0..num_vals {
            let lhs_val_id = lhs_val_no_assignments[i] as usize;
            if i != lhs_val_id || new_vn_info[lhs_val_id].as_ref().is_some_and(|n| !Rc::ptr_eq(n, &self.valnos[i])) {
                must_map_cur_val_nos = true;
                break;
            }
        }
        // If we have to apply a mapping to our base range assignment, rewrite
        // it now.
        if must_map_cur_val_nos && !self.empty() {
            // Map the first live range.
            let mut out = 0usize;
            let first = lhs_val_no_assignments[self.segments[0].vn().id() as usize] as usize;
            self.segments[0].valno = new_vn_info[first].clone();
            for i in 1..self.segments.len() {
                let next_val_no =
                    new_vn_info[lhs_val_no_assignments[self.segments[i].vn().id() as usize] as usize].clone();
                assert!(next_val_no.is_some(), "Huh?");
                // If this live range has the same value # as its immediate
                // predecessor, and if they are neighbors,
                // remove one Segment.  This happens when we
                // have [0,4:0)[4,7:1) and map 0/1 onto the same value #.
                if same_vn(&self.segments[out].valno, &next_val_no) && self.segments[out].end == self.segments[i].start
                {
                    self.segments[out].end = self.segments[i].end.clone();
                } else {
                    // Didn't merge. Move OutIt to the next segment,
                    out += 1;
                    self.segments[out].valno = next_val_no;
                    if out != i {
                        self.segments[out].start = self.segments[i].start.clone();
                        self.segments[out].end = self.segments[i].end.clone();
                    }
                }
            }
            // If we merge some segments, chop off the end.
            out += 1;
            self.segments.truncate(out);
        }
        // Rewrite Other values before changing the VNInfo ids.
        for s in &mut other.segments {
            s.valno = new_vn_info[rhs_val_no_assignments[s.vn().id() as usize] as usize].clone();
        }
        // Update val# info. Renumber them and make sure they all belong to this
        // LiveRange now. Also remove dead val#'s.
        let mut num_val_nos = 0usize;
        for i in 0..num_new_vals {
            if let Some(vni) = &new_vn_info[i] {
                if num_val_nos >= num_vals {
                    self.valnos.push(Rc::clone(vni));
                } else {
                    self.valnos[num_val_nos] = Rc::clone(vni);
                }
                vni.set_id(num_val_nos as u32); // Renumber val#.
                num_val_nos += 1;
            }
        }
        if num_new_vals < num_vals {
            self.valnos.truncate(num_new_vals); // shrinkify
        }
        // Okay, now insert the RHS live segments into the LHS.
        let mut updater = LiveRangeUpdater::new(Some(self));
        for s in other.segments.clone() {
            updater.add(s);
        }
    }

    /// `MergeSegmentsInAsValue` (LiveInterval.cpp#L735): all segments of `rhs`
    /// into this range as the value number `lhs_val_no`.
    pub fn merge_segments_in_as_value(
        &mut self,
        rhs: &LiveRange,
        lhs_val_no: &Vn,
    ) {
        let mut updater = LiveRangeUpdater::new(Some(self));
        for s in &rhs.segments {
            updater.add(Segment::new(s.start.clone(), s.end.clone(), Some(lhs_val_no.clone())));
        }
    }

    /// `MergeValueInAsValue` (LiveInterval.cpp#L747): the segments of
    /// `rhs_val_no` in `rhs` into this range as `lhs_val_no`.
    pub fn merge_value_in_as_value(
        &mut self,
        rhs: &LiveRange,
        rhs_val_no: &Vn,
        lhs_val_no: &Vn,
    ) {
        let mut updater = LiveRangeUpdater::new(Some(self));
        for s in &rhs.segments {
            if same_vn(&s.valno, &Some(rhs_val_no.clone())) {
                updater.add(Segment::new(s.start.clone(), s.end.clone(), Some(lhs_val_no.clone())));
            }
        }
    }

    /// `MergeValueNumberInto` (LiveInterval.cpp#L760): `v1` and `v2` are
    /// equivalent; eliminates the larger number into the smaller.
    pub fn merge_value_number_into(
        &mut self,
        mut v1: Vn,
        mut v2: Vn,
    ) -> Vn {
        assert!(!Rc::ptr_eq(&v1, &v2), "Identical value#'s are always equivalent!");
        // Make sure V2 is smaller than V1.
        if v1.id() < v2.id() {
            v1.copy_from(&v2);
            std::mem::swap(&mut v1, &mut v2);
        }
        // Merge V1 segments into V2.
        let mut i = 0usize;
        while i != self.segments.len() {
            let s = i;
            i += 1;
            if !same_vn(&self.segments[s].valno, &Some(v1.clone())) {
                continue; // Not a V1 Segment.
            }
            // After changing this segment to V2, it may touch an adjacent V2
            // segment. Merge with either neighbor before
            // continuing.
            self.segments[s].valno = Some(v2.clone());
            i = self.merge_adjacent_segments(s) + 1;
        }
        // Now that V1 is dead, remove it.
        self.mark_val_no_for_deletion(&v1);
        v2
    }

    /// `Query` (LiveInterval.h#L540): liveness at the instruction of `idx`.
    pub fn query(
        &self,
        idx: &SlotIndex,
    ) -> LiveQueryResult {
        // Find the segment that enters the instruction.
        let mut i = self.find(&idx.get_base_index());
        let e = self.segments.len();
        if i == e {
            return LiveQueryResult::new(None, None, SlotIndex::default(), false);
        }
        // Is this an instruction live-in segment?
        // If Idx is the start index of a basic block, include live-in segments
        // that start at Idx.getBaseIndex().
        let mut early_val: Option<Vn> = None;
        let mut late_val: Option<Vn> = None;
        let mut end_point = SlotIndex::default();
        let mut kill = false;
        if self.segments[i].start <= idx.get_base_index() {
            early_val = self.segments[i].valno.clone();
            end_point = self.segments[i].end.clone();
            // Move to the potentially live-out segment.
            if SlotIndex::is_same_instr(idx, &self.segments[i].end) {
                kill = true;
                i += 1;
                if i == e {
                    return LiveQueryResult::new(early_val, late_val, end_point, kill);
                }
            }
            // Special case: A PHIDef value can have its def in the middle of a
            // segment if the value happens to be live out of the layout
            // predecessor. Such a value is not live-in.
            if early_val.as_ref().expect("a value").def() == idx.get_base_index() {
                early_val = None;
            }
        }
        // I now points to the segment that may be live-through, or defined by
        // this instr. Ignore segments starting after the current instr.
        if !SlotIndex::is_earlier_instr(idx, &self.segments[i].start) {
            late_val = self.segments[i].valno.clone();
            end_point = self.segments[i].end.clone();
        }
        LiveQueryResult::new(early_val, late_val, end_point, kill)
    }

    /// `isZeroLength`: no live segment spans an instruction.
    pub fn is_zero_length(
        &self,
        indexes: &SlotIndexes,
    ) -> bool {
        for s in &self.segments {
            if indexes.get_next_non_null_index(&s.start).get_base_index() < s.end.get_base_index() {
                return false;
            }
        }
        true
    }

    /// `isLiveAtIndexes` (LiveInterval.cpp#L801): any of the (ascending)
    /// `slots` is inside a segment.
    pub fn is_live_at_indexes(
        &self,
        slots: &[SlotIndex],
    ) -> bool {
        // If there are no regmask slots, we have nothing to search.
        let Some(first) = slots.first() else { return false };
        // Start our search at the first segment that ends after the first slot.
        let mut segment_i = self.find(first);
        // If there are no segments that end after the first slot, we're done.
        if segment_i == self.segments.len() {
            return false;
        }
        // Look for each slot in the live range.
        for slot in slots {
            // Go to the next segment that ends after the current slot.
            // The slot may be within a hole in the range.
            segment_i = self.advance_to_or_end(segment_i, slot);
            if segment_i == self.segments.len() {
                return false;
            }
            // If this segment contains the slot, we're done.
            if self.segments[segment_i].contains(slot) {
                return true;
            }
        }
        false
    }

    /// `verify` (LiveInterval.cpp#L1062): the invariants of a range.
    pub fn verify(&self) -> bool {
        VERIFIES.with(|count| count.set(count.get() + 1));
        for (at, s) in self.segments.iter().enumerate() {
            if !s.start.is_valid() || !s.end.is_valid() || s.start >= s.end {
                return false;
            }
            let Some(vn) = &s.valno else { return false };
            if vn.id() as usize >= self.valnos.len() || !Rc::ptr_eq(vn, &self.valnos[vn.id() as usize]) {
                return false;
            }
            if let Some(next) = self.segments.get(at + 1) {
                if s.end > next.start {
                    return false;
                }
                if s.end == next.start && same_vn(&s.valno, &next.valno) {
                    return false;
                }
            }
        }
        true
    }
}

impl PartialOrd for LiveRange {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.begin_index().cmp(&other.begin_index()))
    }
}

impl PartialEq for LiveRange {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.segments == other.segments
    }
}

impl fmt::Display for LiveRange {
    /// `LiveRange::print` (LiveInterval.cpp#L1056).
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if self.empty() {
            write!(f, "EMPTY")?;
        } else {
            for s in &self.segments {
                write!(f, "{s}")?;
            }
        }
        // Print value number info.
        if self.get_num_val_nums() != 0 {
            write!(f, " ")?;
            for (vnum, vni) in self.valnos.iter().enumerate() {
                if vnum != 0 {
                    write!(f, " ")?;
                }
                write!(f, "{vni}")?;
            }
        }
        Ok(())
    }
}

/// `LiveInterval` (LiveInterval.h#L707): the liveness of a virtual register
/// with its spill weight.
#[derive(Clone)]
pub struct LiveInterval {
    pub range: LiveRange,
    reg: u32,
    weight: f32,
}

impl std::ops::Deref for LiveInterval {
    type Target = LiveRange;
    fn deref(&self) -> &LiveRange {
        &self.range
    }
}

impl std::ops::DerefMut for LiveInterval {
    fn deref_mut(&mut self) -> &mut LiveRange {
        &mut self.range
    }
}

impl LiveInterval {
    pub fn new(
        reg: u32,
        weight: f32,
    ) -> Self {
        Self { range: LiveRange::new(), reg, weight }
    }

    pub fn reg(&self) -> u32 {
        self.reg
    }

    pub fn weight(&self) -> f32 {
        self.weight
    }

    pub fn increment_weight(
        &mut self,
        inc: f32,
    ) {
        self.weight += inc;
    }

    pub fn set_weight(
        &mut self,
        value: f32,
    ) {
        self.weight = value;
    }

    /// `isSpillable`: `huge_valf` marks an interval that cannot be spilled.
    pub fn is_spillable(&self) -> bool {
        self.weight != f32::INFINITY
    }

    pub fn mark_not_spillable(&mut self) {
        self.weight = f32::INFINITY;
    }

    /// `getSize` (LiveInterval.cpp#L956): the sum of the segment lengths.
    pub fn get_size(&self) -> u32 {
        self.range.segments.iter().map(|s| s.start.distance(&s.end) as u32).sum()
    }
}

impl PartialOrd for LiveInterval {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some((self.begin_index(), self.reg).cmp(&(other.begin_index(), other.reg)))
    }
}

impl PartialEq for LiveInterval {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.reg == other.reg && self.range == other.range
    }
}

impl fmt::Display for LiveInterval {
    /// `LiveInterval::print` (LiveInterval.cpp#L1089).
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "%{} {}  weight:{}", self.reg, self.range, self.weight)
    }
}

/// `LiveRangeUpdater` (LiveInterval.h#L895): bulk updates of a live range that
/// defer the shifting of the segments after the insertion point. The range is
/// in an invalid state until `flush`.
pub struct LiveRangeUpdater<'a> {
    lr: Option<&'a mut LiveRange>,
    last_start: SlotIndex,
    write_i: usize,
    read_i: usize,
    spills: Vec<Segment>,
}

impl<'a> LiveRangeUpdater<'a> {
    pub fn new(lr: Option<&'a mut LiveRange>) -> Self {
        Self { lr, last_start: SlotIndex::default(), write_i: 0, read_i: 0, spills: Vec::new() }
    }

    fn seg(
        &self,
        i: usize,
    ) -> &Segment {
        &self.lr.as_deref().expect("Cannot add to a null destination").segments[i]
    }

    fn lr(&mut self) -> &mut LiveRange {
        self.lr.as_deref_mut().expect("Cannot add to a null destination")
    }

    pub fn is_dirty(&self) -> bool {
        self.last_start.is_valid()
    }

    /// `coalescable` (LiveInterval.cpp#L1182): whether `a` and `b` should be
    /// coalesced.
    fn coalescable(
        a: &Segment,
        b: &Segment,
    ) -> bool {
        assert!(a.start <= b.start, "Unordered live segments.");
        if a.end == b.start {
            return same_vn(&a.valno, &b.valno);
        }
        if a.end < b.start {
            return false;
        }
        assert!(same_vn(&a.valno, &b.valno), "Cannot overlap different values");
        true
    }

    /// `LiveRangeUpdater::add` (LiveInterval.cpp#L1194).
    pub fn add(
        &mut self,
        mut seg: Segment,
    ) {
        // Flush the state if Start moves backwards.
        if !self.last_start.is_valid() || self.last_start > seg.start {
            if self.is_dirty() {
                self.flush();
            }
            // This brings us to an uninitialized state. Reinitialize.
            assert!(self.spills.is_empty(), "Leftover spilled segments");
            self.write_i = 0;
            self.read_i = 0;
        }
        // Remember start for next time.
        self.last_start = seg.start.clone();
        // Advance ReadI until it ends after Seg.start.
        let e = self.lr().segments.len();
        if self.read_i != e && self.seg(self.read_i).end <= seg.start {
            // First try to close the gap between WriteI and ReadI with spills.
            if self.read_i != self.write_i {
                self.merge_spills();
            }
            // Then advance ReadI.
            if self.read_i == self.write_i {
                let at = self.lr().find(&seg.start);
                self.read_i = at;
                self.write_i = at;
            } else {
                while self.read_i != e && self.seg(self.read_i).end <= seg.start {
                    let (r, w) = (self.read_i, self.write_i);
                    let moved = self.lr().segments[r].clone();
                    self.lr().segments[w] = moved;
                    self.write_i += 1;
                    self.read_i += 1;
                }
            }
        }
        assert!(self.read_i == e || self.seg(self.read_i).end > seg.start);
        // Check if the ReadI segment begins early.
        if self.read_i != e && self.seg(self.read_i).start <= seg.start {
            let r = self.read_i;
            assert!(same_vn(&self.seg(r).valno, &seg.valno), "Cannot overlap different values");
            // Bail if Seg is completely contained in ReadI.
            if self.seg(r).end >= seg.end {
                return;
            }
            // Coalesce into Seg.
            seg.start = self.seg(r).start.clone();
            self.read_i += 1;
        }
        // Coalesce as much as possible from ReadI into Seg.
        while self.read_i != e && Self::coalescable(&seg, self.seg(self.read_i)) {
            let r = self.read_i;
            seg.end = std::cmp::max(seg.end.clone(), self.seg(r).end.clone());
            self.read_i += 1;
        }
        // Try coalescing Spills.back() into Seg.
        if self.spills.last().is_some_and(|back| Self::coalescable(back, &seg)) {
            let back = self.spills.pop().expect("a spill");
            seg.start = back.start;
            seg.end = std::cmp::max(back.end, seg.end.clone());
        }
        // Try coalescing Seg into WriteI[-1].
        if self.write_i != 0 {
            let w = self.write_i - 1;
            if Self::coalescable(&self.lr().segments[w], &seg) {
                let end = std::cmp::max(self.seg(w).end.clone(), seg.end.clone());
                self.lr().segments[w].end = end;
                return;
            }
        }
        // Seg doesn't coalesce with anything, and needs to be inserted
        // somewhere.
        if self.write_i != self.read_i {
            let w = self.write_i;
            self.lr().segments[w] = seg;
            self.write_i += 1;
            return;
        }
        // Finally, append to LR or Spills.
        if self.write_i == e {
            self.lr().segments.push(seg);
            let end = self.lr().segments.len();
            self.write_i = end;
            self.read_i = end;
        } else {
            self.spills.push(seg);
        }
    }

    /// `mergeSpills` (LiveInterval.cpp#L1278): merges as many spilled segments
    /// as possible into the gap between WriteI and ReadI.
    fn merge_spills(&mut self) {
        // Perform a backwards merge of Spills and [SpillI;WriteI).
        let gap_size = self.read_i - self.write_i;
        let num_moved = std::cmp::min(self.spills.len(), gap_size);
        let mut src = self.write_i;
        let mut dst = src + num_moved;
        let mut spill_src = self.spills.len();
        // This is the new WriteI position after merging spills.
        self.write_i = dst;
        // Now merge Src and Spills backwards.
        while src != dst {
            dst -= 1;
            if src != 0 && self.seg(src - 1).start > self.spills[spill_src - 1].start {
                src -= 1;
                let moved = self.lr().segments[src].clone();
                self.lr().segments[dst] = moved;
            } else {
                spill_src -= 1;
                let moved = self.spills[spill_src].clone();
                self.lr().segments[dst] = moved;
            }
        }
        assert_eq!(num_moved, self.spills.len() - spill_src);
        self.spills.truncate(spill_src);
    }

    /// `flush` (LiveInterval.cpp#L1301): the updater's state into the range so
    /// it is valid and holds all added segments.
    pub fn flush(&mut self) {
        if !self.is_dirty() {
            return;
        }
        // Clear the dirty state.
        self.last_start = SlotIndex::default();
        // Nothing to merge?
        if self.spills.is_empty() {
            let (w, r) = (self.write_i, self.read_i);
            self.lr().segments.drain(w..r);
            if checking() {
                assert!(self.lr().verify());
            }
            return;
        }
        // Resize the WriteI - ReadI gap to match Spills.
        let gap_size = self.read_i - self.write_i;
        let spills = self.spills.len();
        if gap_size < spills {
            // The gap is too small. Make some room.
            let r = self.read_i;
            let filler = std::iter::repeat_with(Segment::default).take(spills - gap_size);
            self.lr().segments.splice(r..r, filler);
        } else {
            // Shrink the gap if necessary.
            let (from, to) = (self.write_i + spills, self.read_i);
            self.lr().segments.drain(from..to);
        }
        self.read_i = self.write_i + spills;
        self.merge_spills();
        if checking() {
            assert!(self.lr().verify());
        }
    }
}

impl Drop for LiveRangeUpdater<'_> {
    fn drop(&mut self) {
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    /// 200 instructions in one block: base index of instruction n is 16 (n +
    /// 1).
    fn indexes() -> (SlotIndexes, Vec<SlotIndex>) {
        let insns: Vec<Arc<Insn>> = (0..200).map(|_| Arc::new(Insn::new(0, None, None, vec![], vec![]))).collect();
        let body = LirBody::new(
            "ranges",
            0,
            vec![Block::from(LirBlock::new(0, insns.clone()))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let indexes = SlotIndexes::new(&body);
        let at = insns.iter().map(|one| indexes.get_instruction_index(one.id())).collect();
        (indexes, at)
    }

    fn range(
        spec: &[(usize, usize, u32)],
        at: &[SlotIndex],
    ) -> LiveRange {
        let mut lr = LiveRange::new();
        let mut made: Vec<Vn> = Vec::new();
        for &(from, to, v) in spec {
            while made.len() <= v as usize {
                made.push(lr.get_next_value(at[from].get_reg_slot(false)));
            }
            lr.add_segment(Segment::new(
                at[from].get_reg_slot(false),
                at[to].get_reg_slot(false),
                Some(made[v as usize].clone()),
            ));
        }
        lr
    }

    fn shape(lr: &LiveRange) -> Vec<(u32, u32, u32)> {
        lr.segments
            .iter()
            .map(|s| (s.start.get_index(), s.end.get_index(), s.valno.as_ref().expect("a value").id()))
            .collect()
    }

    /// `addSegment` joins a segment that touches or lies inside one of the same
    /// value and keeps different values apart.
    #[test]
    fn test_adding_segments_merges_those_of_one_value_and_keeps_values_apart() {
        let (_, at) = indexes();
        let lr = range(&[(1, 3, 0), (3, 5, 0), (7, 9, 0), (2, 8, 0)], &at);
        assert_eq!(lr.segments.len(), 1, "{lr}");
        assert_eq!(lr.segments[0].start, at[1].get_reg_slot(false));
        assert_eq!(lr.segments[0].end, at[9].get_reg_slot(false));
        let two = range(&[(1, 3, 0), (3, 5, 1)], &at);
        assert_eq!(two.segments.len(), 2, "touching segments of different values stay apart: {two}");
        assert!(two.verify());
    }

    /// A segment that is a complete superset of earlier ones of its value
    /// swallows them: the start of the result was an index past the end of
    /// the vector in C++ (see the module doc).
    #[test]
    fn test_a_segment_covering_earlier_segments_of_its_value_replaces_them() {
        let (_, at) = indexes();
        let lr = range(&[(10, 12, 0), (14, 16, 0), (18, 20, 0), (5, 25, 0)], &at);
        assert_eq!(lr.segments.len(), 1, "{lr}");
        assert_eq!((lr.begin_index(), lr.end_index()), (at[5].get_reg_slot(false), at[25].get_reg_slot(false)));
        assert!(lr.verify());
    }

    /// `extend_in_block` finds the segment before `use_` without building the
    /// slot before it (62 Ir a call, 1.8 M of d_faces): at every slot of an
    /// instruction it answers as the search for the previous slot did.
    #[test]
    fn test_extend_in_block_answers_at_every_slot_as_the_previous_slot_search() {
        let (_, at) = indexes();
        for slot in [at[7].get_base_index(), at[7].get_reg_slot(true), at[7].get_reg_slot(false), at[7].get_dead_slot()]
        {
            let mut lr = range(&[(3, 6, 0), (9, 11, 1)], &at);
            let reference = lr.clone();
            let i = reference.segments.partition_point(|x| x.start <= slot.get_prev_slot());
            let want = (i != 0 && reference.segments[i - 1].end > at[0].get_base_index())
                .then(|| reference.segments[i - 1].valno.clone());
            let got = lr.extend_in_block(&at[0].get_base_index(), &slot);
            assert_eq!(got.is_some(), want.is_some(), "{slot:?}");
            assert_eq!(got.map(|v| v.id()), want.flatten().map(|v| v.id()));
        }
    }

    #[test]
    fn test_find_live_at_and_the_value_at_a_point() {
        let (_, at) = indexes();
        let lr = range(&[(1, 3, 0), (5, 9, 1)], &at);
        let (a, b) = (lr.get_val_num_info(0), lr.get_val_num_info(1));
        assert!(lr.live_at(&at[1].get_reg_slot(false)) && !lr.live_at(&at[3].get_reg_slot(false)));
        assert!(!lr.live_at(&at[4].get_reg_slot(false)), "a hole");
        assert!(Rc::ptr_eq(&lr.get_vn_info_at(&at[2].get_reg_slot(false)).expect("a"), &a));
        assert!(lr.get_vn_info_at(&at[4].get_reg_slot(false)).is_none());
        assert!(Rc::ptr_eq(&lr.get_vn_info_before(&at[9].get_reg_slot(false)).expect("b"), &b));
        assert!(lr.expired_at(&at[9].get_reg_slot(false)) && !lr.expired_at(&at[8].get_reg_slot(false)));
    }

    #[test]
    fn test_create_dead_def_makes_a_value_defined_at_a_point_and_returns_the_existing_one() {
        let (_, at) = indexes();
        let mut lr = LiveRange::new();
        let first = lr.create_dead_def(at[4].get_reg_slot(false));
        let again = lr.create_dead_def(at[4].get_reg_slot(false));
        assert!(Rc::ptr_eq(&first, &again));
        assert_eq!(lr.segments[0].end, at[4].get_dead_slot(), "a dead def lives to its dead slot");
        let earlier = lr.create_dead_def(at[2].get_reg_slot(false));
        assert!(lr.verify() && lr.segments[0].start == at[2].get_reg_slot(false) && !Rc::ptr_eq(&earlier, &first));
        // normal and early-clobber defs of one instruction become the
        // early-clobber one
        let ec = lr.create_dead_def(at[4].get_reg_slot(true));
        assert!(Rc::ptr_eq(&ec, &first) && first.def() == at[4].get_reg_slot(true));
    }

    #[test]
    fn test_remove_segment_trims_splits_and_removes() {
        let (_, at) = indexes();
        let r = |n: usize| at[n].get_reg_slot(false);
        let mut lr = range(&[(1, 10, 0)], &at);
        lr.remove_segment(&r(4), &r(6), false);
        assert_eq!(lr.segments.len(), 2);
        lr.remove_segment(&r(1), &r(3), false);
        assert_eq!(lr.segments[0].start, r(3));
        lr.remove_segment(&r(8), &r(10), false);
        assert_eq!(lr.segments[1].end, r(8));
        lr.remove_segment(&r(3), &r(4), true);
        lr.remove_segment(&r(6), &r(8), true);
        assert!(lr.empty() && lr.valnos.is_empty(), "the value is gone with its last segment: {lr}");
    }

    /// `overlaps` of two ranges against the pairwise answer.
    #[test]
    fn test_overlaps_is_the_pairwise_answer() {
        let (_, at) = indexes();
        let mut seed = 12345u64;
        let mut next = move |n: usize| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as usize) % n
        };
        let random = |next: &mut dyn FnMut(usize) -> usize| {
            let mut spec = Vec::new();
            let mut p = next(5) + 1;
            for _ in 0..next(6) + 1 {
                let len = next(6) + 1;
                spec.push((p, p + len, 0));
                p += len + next(6) + 1;
            }
            spec
        };
        let mut hits = 0;
        for _ in 0..300 {
            let (a, b) = (random(&mut next), random(&mut next));
            let (ra, rb) = (range(&a, &at), range(&b, &at));
            let want = a.iter().any(|x| b.iter().any(|y| x.0 < y.1 && y.0 < x.1));
            assert_eq!(ra.overlaps(&rb), want, "{ra} vs {rb}");
            assert_eq!(rb.overlaps(&ra), want);
            hits += usize::from(want);
        }
        assert!(hits > 20 && hits < 280, "premise: some pairs overlap and some do not ({hits})");
    }

    /// `LiveRangeUpdater` adding segments in order, out of order and with
    /// spills gives what `addSegment` of each gives.
    #[test]
    fn test_the_updater_gives_what_adding_each_segment_gives() {
        let (_, at) = indexes();
        let mut seed = 99u64;
        let mut next = move |n: usize| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as usize) % n
        };
        for round in 0..200 {
            // one value, disjoint segments, some touching
            let mut spec: Vec<(usize, usize, u32)> = Vec::new();
            let mut p = 1;
            for _ in 0..next(12) + 1 {
                let len = next(5) + 1;
                spec.push((p, p + len, 0));
                p += len + next(3);
            }
            // a shuffle of the order
            let mut order: Vec<usize> = (0..spec.len()).collect();
            for i in (1..order.len()).rev() {
                order.swap(i, next(i + 1));
            }
            let sorted = round % 2 == 0;
            let order: Vec<usize> = if sorted { (0..spec.len()).collect() } else { order };
            let mut by_add = LiveRange::new();
            let mut by_updater = LiveRange::new();
            let (v1, v2) = (
                by_add.get_next_value(at[1].get_reg_slot(false)),
                by_updater.get_next_value(at[1].get_reg_slot(false)),
            );
            // a base the updater adds into
            by_add.add_segment(Segment::new(at[60].get_reg_slot(false), at[64].get_reg_slot(false), Some(v1.clone())));
            by_updater.add_segment(Segment::new(
                at[60].get_reg_slot(false),
                at[64].get_reg_slot(false),
                Some(v2.clone()),
            ));
            for &i in &order {
                let (from, to, _) = spec[i];
                by_add.add_segment(Segment::new(
                    at[from].get_reg_slot(false),
                    at[to].get_reg_slot(false),
                    Some(v1.clone()),
                ));
            }
            {
                let mut updater = LiveRangeUpdater::new(Some(&mut by_updater));
                for &i in &order {
                    let (from, to, _) = spec[i];
                    updater.add(Segment::new(
                        at[from].get_reg_slot(false),
                        at[to].get_reg_slot(false),
                        Some(v2.clone()),
                    ));
                }
            }
            assert!(by_updater.verify(), "round {round}: {by_updater}");
            assert_eq!(shape(&by_add), shape(&by_updater), "round {round}, order {order:?}, spec {spec:?}");
        }
    }

    /// `MergeValueNumberInto` folds the larger number into the smaller and
    /// merges neighbours.
    #[test]
    fn test_merging_value_numbers_joins_the_segments_that_now_touch() {
        let (_, at) = indexes();
        let mut lr = range(&[(1, 4, 0), (4, 7, 1)], &at);
        let (v0, v1) = (lr.get_val_num_info(0), lr.get_val_num_info(1));
        let kept = lr.merge_value_number_into(v1, v0.clone());
        assert!(Rc::ptr_eq(&kept, &v0));
        assert_eq!(lr.segments.len(), 1, "{lr}");
        assert_eq!(lr.get_num_val_nums(), 1);
        assert!(lr.verify());
    }

    /// `join` with the identity mapping and a mapped one.
    #[test]
    fn test_join_applies_the_value_number_mappings() {
        let (_, at) = indexes();
        let mut lhs = range(&[(1, 4, 0)], &at);
        let mut rhs = range(&[(4, 8, 0), (10, 12, 1)], &at);
        // rhs value 0 continues lhs value 0 (same new number), rhs value 1 is a
        // new value
        let new_vn: Vec<Option<Vn>> = vec![Some(lhs.get_val_num_info(0)), Some(rhs.get_val_num_info(1))];
        let mut new_vn = new_vn;
        lhs.join(&mut rhs, &[0], &[0, 1], &mut new_vn);
        assert_eq!(
            shape(&lhs),
            vec![
                (at[1].get_reg_slot(false).get_index(), at[8].get_reg_slot(false).get_index(), 0),
                (at[10].get_reg_slot(false).get_index(), at[12].get_reg_slot(false).get_index(), 1)
            ]
        );
        assert_eq!(lhs.get_num_val_nums(), 2);
        assert!(lhs.verify());
    }

    #[test]
    fn test_query_reports_the_value_in_and_out_of_an_instruction() {
        let (_, at) = indexes();
        // value 0 defined at 2, killed at 5; value 1 defined at 5 (a
        // redefinition), read at 8
        let lr = range(&[(2, 5, 0), (5, 8, 1)], &at);
        let q = lr.query(&at[3]);
        assert!(q.value_in().is_some() && q.value_out().is_some() && !q.is_kill());
        let q = lr.query(&at[5]);
        assert!(q.is_kill(), "value 0 ends at the instruction");
        assert_eq!(q.value_defined().map(|v| v.id()), Some(1));
        assert_eq!(q.value_in().map(|v| v.id()), Some(0));
        assert!(lr.query(&at[20]).value_in().is_none());
    }

    #[test]
    fn test_covers_and_live_at_indexes() {
        let (_, at) = indexes();
        let r = |n: usize| at[n].get_reg_slot(false);
        let big = range(&[(1, 6, 0), (6, 12, 1)], &at);
        let small = range(&[(3, 8, 0)], &at);
        assert!(big.covers(&small), "adjacent segments cover");
        assert!(!small.covers(&big));
        let holes = range(&[(1, 3, 0), (5, 7, 0)], &at);
        assert!(!holes.covers(&range(&[(2, 6, 0)], &at)));
        assert!(holes.is_live_at_indexes(&[r(0), r(2)]) && !holes.is_live_at_indexes(&[r(3), r(4)]));
    }

    #[test]
    fn test_renumber_values_numbers_by_first_appearance_and_drops_unused() {
        let (_, at) = indexes();
        let mut lr = range(&[(1, 3, 2), (5, 7, 0)], &at);
        assert_eq!(lr.get_num_val_nums(), 3, "value 1 is unused");
        lr.renumber_values();
        assert_eq!(lr.get_num_val_nums(), 2);
        assert_eq!(shape(&lr).iter().map(|s| s.2).collect::<Vec<_>>(), vec![0, 1]);
        assert!(lr.verify());
    }

    #[test]
    fn test_a_live_interval_orders_by_start_then_register_and_prints() {
        let (_, at) = indexes();
        let mut a = LiveInterval::new(5, 1.5);
        let mut b = LiveInterval::new(2, 0.5);
        for li in [&mut a, &mut b] {
            let v = li.get_next_value(at[3].get_reg_slot(false));
            li.add_segment(Segment::new(at[3].get_reg_slot(false), at[7].get_reg_slot(false), Some(v)));
        }
        assert!(b < a, "the same start orders by register");
        assert!(a.is_spillable());
        a.mark_not_spillable();
        assert!(!a.is_spillable());
        assert_eq!(b.get_size(), 4 * 16);
        assert_eq!(b.to_string(), "%2 [64r,128r:0) 0@64r  weight:0.5");
    }
}
