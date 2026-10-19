//! `SplitKit`: the analysis of a live range for splitting and the editor that
//! splits it. Translated from llvm/lib/CodeGen/SplitKit.{h,cpp} at
//! d1106deb71cc. This file holds `InsertPointAnalysis` and `SplitAnalysis`;
//! `SplitEditor` follows.
//!
//! # How it works
//! `analyze` lists the slots where the range is used (its defs and its use
//! operands, one per instruction), then walks the range and the blocks
//! together to say, for each block it is live in, the first and last use, and
//! whether it is live in and out (`BlockInfo`). A block it crosses without a
//! use is a through block. A block with a gap in the range is listed twice, as
//! its live-in snippet and its live-out snippet.
//!
//! ```text
//!   block      b0        b1          b2
//!   range    [..use..)  [ no use ]  [..use..]
//!   result   use block  through     use block
//! ```
//!
//! DEVIATE D-SK1: `computeLastInsertPoint` has no landing pad or `inlineasm_br`
//! successor case: the last insert point of a block is its first terminator
//! (or the block end). The fact: LIR has no exception edges and no
//! `INLINEASM_BR`, so the C++ returns `LIP.first` on that path as well.
//!
//! DEVIATE D-SK2: what LIR does not state uniformly is asked of [`SplitEnv`]:
//! a block's first terminator (`getFirstTerminator`), whether a block is a
//! loop latch (`MachineLoopInfo::getLoopFor` and `isLoopLatch`) and whether an
//! instruction is a copy (`TII.isCopyInstr`).
//!
//! # Example
//! A value defined in `b0`, used in `b0` and `b2` and live across `b1`: use
//! blocks `b0` (live out, first def) and `b2` (live in), through block `b1`.
//!
//! # Used by
//! `RAGreedy::trySplit` and its region, block and local splits.

use std::rc::Rc;
use std::sync::Arc;

use super::virtregmap::VirtRegMap;
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::liverange::LiveInterval;
use crate::liveness::slotindex::{InstrKey, SlotIndex};
use crate::model::lir::Insn;

/// What LIR does not state the way MIR does (D-SK2).
pub trait SplitEnv {
    /// `MI.isTerminator()`.
    fn is_terminator(
        &self,
        insn: &Insn,
    ) -> bool;

    /// `L = Loops.getLoopFor(MBB); L && L->isLoopLatch(MBB)`.
    fn is_loop_latch(
        &self,
        block: usize,
    ) -> bool;

    /// `Loops.getLoopFor(MBB)`: a number naming the innermost loop of `block`.
    fn loop_of(
        &self,
        block: usize,
    ) -> Option<usize>;

    /// `L->getLoopDepth()`.
    fn loop_depth(
        &self,
        l: usize,
    ) -> u32;

    /// `L->getHeader()`.
    fn loop_header(
        &self,
        l: usize,
    ) -> usize;

    /// `new` was made from `from` by a split (`createFrom`): what the target
    /// knows by register number (its width) goes with it.
    fn note_split_from(
        &self,
        lis: &LiveIntervals,
        new: u32,
        from: u32,
    );

    /// `TII.isTriviallyReMaterializable(*MRI.getUniqueVRegDef(reg))`: the
    /// value can be made again where it is read.
    fn is_rematerializable(
        &self,
        reg: u32,
    ) -> bool;

    /// `recomputeRegClass` of the registers an edit made, once their
    /// instructions are in place.
    fn reclass(
        &self,
        _lis: &LiveIntervals,
        _regs: &[u32],
    ) {
    }

    /// A definition that is only a definition, so nothing is lost by erasing it
    /// when nothing reads what it writes (`MI->isSafeToMove` and no side
    /// effects, of `eliminateDeadDefs`).
    fn is_pure_def(
        &self,
        _insn: &Insn,
    ) -> bool {
        false
    }

    /// `TII.isCopyInstr(MI) || MI.isSubregToReg()`.
    fn is_copy_like(
        &self,
        insn: &Insn,
    ) -> bool;

    /// `LiveRangeEdit::canRematerializeAt` and `rematerializeAt`: `def`, the
    /// definition of `parent`, made again as a definition of `to`, beside
    /// `beside` (as `build_copy`), or None when it cannot be (not
    /// rematerializable, or it reads a register).
    fn rematerialize(
        &self,
        _def: &Insn,
        _to: u32,
        _parent: u32,
        _beside: Option<&Insn>,
    ) -> Option<Arc<Insn>> {
        None
    }

    /// `BuildMI(MBB, I, DebugLoc(), TII.get(TII.getLiveRangeSplitOpcode(
    /// FromReg, MF)), ToReg).addReg(FromReg)`: a move of `from` into `to`.
    /// The new instruction goes beside `beside` (the one it is put before, or
    /// the last of its block) and claims none of its bytes.
    fn build_copy(
        &self,
        lis: &LiveIntervals,
        to: u32,
        from: u32,
        beside: Option<&Insn>,
    ) -> Arc<Insn>;
}

/// `-enable-split-loopiv-heuristic`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L46>).
const ENABLE_LOOP_IV_HEURISTIC: bool = true;

/// `InsertPointAnalysis`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.h#L50>).
pub struct InsertPointAnalysis {
    /// Per block: the last insert point, once computed (the pair's second is
    /// always invalid, D-SK1).
    last_insert_point: Vec<SlotIndex>,
}

impl InsertPointAnalysis {
    pub fn new(num_blocks: usize) -> Self {
        Self { last_insert_point: vec![SlotIndex::default(); num_blocks] }
    }

    /// `getLastInsertPoint` with `computeLastInsertPoint`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L62>):
    /// the first terminator, or the block end.
    pub fn get_last_insert_point(
        &mut self,
        lis: &LiveIntervals,
        env: &dyn SplitEnv,
        mbb: usize,
    ) -> SlotIndex {
        if self.last_insert_point[mbb].is_valid() {
            return self.last_insert_point[mbb];
        }
        // `MBB.getFirstTerminator()`.
        let at = match lis.block_insns(mbb).iter().find(|insn| env.is_terminator(insn)) {
            None => lis.indexes.get_mbb_end_idx(mbb),
            Some(term) => lis.indexes.get_instruction_index(term.id()),
        };
        self.last_insert_point[mbb] = at;
        at
    }

    /// `getFirstInsertPoint`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.h#L82>):
    /// `SkipPHIsLabelsAndDebug`; a LIR block starts with no phi or label, but
    /// the entry's arrival instruction is no code and nothing goes before it.
    pub fn get_first_insert_point(
        &self,
        lis: &LiveIntervals,
        mbb: usize,
    ) -> SlotIndex {
        match first_insert_instr(lis, mbb) {
            Some(first) => lis.indexes.get_instruction_index(first),
            None => lis.indexes.get_mbb_start_idx(mbb),
        }
    }
}

/// The first instruction of `mbb` that code may go before (`MBB.
/// SkipPHIsLabelsAndDebug(MBB.begin())`).
pub fn first_insert_instr(
    lis: &LiveIntervals,
    mbb: usize,
) -> Option<InstrKey> {
    lis.block_insns(mbb).iter().find(|insn| !insn.arrival()).map(|insn| insn.id())
}

/// `SplitAnalysis::BlockInfo`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.h#L121>).
#[derive(Clone, Copy, Debug, Default)]
pub struct BlockInfo {
    pub mbb: usize,
    /// First instr accessing current reg.
    pub first_instr: SlotIndex,
    /// Last instr accessing current reg.
    pub last_instr: SlotIndex,
    /// First non-phi valno->def, or invalid.
    pub first_def: SlotIndex,
    /// Current reg is live in.
    pub live_in: bool,
    /// Current reg is live out.
    pub live_out: bool,
}

impl BlockInfo {
    /// `isOneInstr`.
    pub fn is_one_instr(&self) -> bool {
        SlotIndex::is_same_instr(&self.first_instr, &self.last_instr)
    }
}

/// `SplitAnalysis`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.h#L96>).
pub struct SplitAnalysis {
    /// Current live interval.
    cur_li: Option<Rc<LiveInterval>>,
    ipa: InsertPointAnalysis,
    /// Sorted slot indexes of using instructions.
    use_slots: Vec<SlotIndex>,
    /// Additional information about the blocks where the range is live.
    use_blocks: Vec<BlockInfo>,
    num_gap_blocks: u32,
    /// Blocks the range is live through without a use.
    through_blocks: Vec<bool>,
    num_through_blocks: u32,
    looks_like_loop_iv: bool,
    num_blocks: usize,
}

impl SplitAnalysis {
    pub fn new(num_blocks: usize) -> Self {
        Self {
            cur_li: None,
            ipa: InsertPointAnalysis::new(num_blocks),
            use_slots: Vec::new(),
            use_blocks: Vec::new(),
            num_gap_blocks: 0,
            through_blocks: Vec::new(),
            num_through_blocks: 0,
            looks_like_loop_iv: false,
            num_blocks,
        }
    }

    /// `clear`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L159>).
    pub fn clear(&mut self) {
        self.use_slots.clear();
        self.use_blocks.clear();
        self.through_blocks.clear();
        self.cur_li = None;
    }

    /// `analyze`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L347>).
    pub fn analyze(
        &mut self,
        lis: &LiveIntervals,
        env: &dyn SplitEnv,
        li: Rc<LiveInterval>,
    ) {
        self.clear();
        self.cur_li = Some(li);
        self.analyze_uses(lis, env);
    }

    /// `analyzeUses`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L167>):
    /// count instructions, basic blocks, and loops using the range.
    fn analyze_uses(
        &mut self,
        lis: &LiveIntervals,
        env: &dyn SplitEnv,
    ) {
        assert!(self.use_slots.is_empty(), "Call clear first");
        let cur = Rc::clone(self.cur_li.as_ref().expect("analyze sets it"));

        // First get all the defs from the interval values. This provides the
        // correct slots for early clobbers.
        for vni in &cur.valnos {
            if !vni.is_phi_def() && !vni.is_unused() {
                self.use_slots.push(vni.def());
            }
        }

        // Get use slots from the use-def chain.
        for op in lis.operands(cur.reg()) {
            if !op.is_def {
                self.use_slots.push(lis.indexes.get_instruction_index(op.instr).get_reg_slot(false));
            }
        }

        self.use_slots.sort();

        // Remove duplicates, keeping the smaller slot for each instruction.
        // That is what we want for early clobbers.
        self.use_slots.dedup_by(|b, a| SlotIndex::is_same_instr(a, b));

        // Compute per-live block info.
        self.calc_live_block_info(lis, env, &cur);
    }

    /// `calcLiveBlockInfo`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L199>):
    /// fill the use blocks with information about the blocks where the range
    /// is live.
    fn calc_live_block_info(
        &mut self,
        lis: &LiveIntervals,
        env: &dyn SplitEnv,
        cur: &LiveInterval,
    ) {
        self.through_blocks = vec![false; self.num_blocks];
        self.num_through_blocks = 0;
        self.num_gap_blocks = 0;
        if cur.empty() {
            return;
        }

        let segs = &cur.segments;
        let (mut lvi, lve) = (0usize, segs.len());
        let (mut use_i, use_e) = (0usize, self.use_slots.len());

        // Loop over basic blocks where the range is live.
        let mut mfi = lis.indexes.get_mbb_from_index(&segs[lvi].start);
        loop {
            let mut bi = BlockInfo { mbb: mfi, ..BlockInfo::default() };
            let (start, stop) = *lis.indexes.get_mbb_range(bi.mbb);

            // If the block contains no uses, the range must be live through. At
            // one point, RegisterCoalescer could create dangling ranges that
            // ended mid-block.
            if use_i == use_e || self.use_slots[use_i] >= stop {
                self.num_through_blocks += 1;
                self.through_blocks[bi.mbb] = true;
                // The range shouldn't end mid-block if there are no uses.
                assert!(segs[lvi].end >= stop, "range ends mid block with no uses");
            } else {
                // This block has uses. Find the first and last uses in the
                // block.
                bi.first_instr = self.use_slots[use_i];
                assert!(bi.first_instr >= start);
                loop {
                    use_i += 1;
                    if !(use_i != use_e && self.use_slots[use_i] < stop) {
                        break;
                    }
                }
                bi.last_instr = self.use_slots[use_i - 1];
                assert!(bi.last_instr < stop);

                // `lvi` is the first live segment overlapping the block.
                bi.live_in = segs[lvi].start <= start;

                // When not live in, the first use should be a def.
                if !bi.live_in {
                    assert!(segs[lvi].start == def_of(&segs[lvi]), "Dangling Segment start");
                    assert!(segs[lvi].start == bi.first_instr, "First instr should be a def");
                    bi.first_def = bi.first_instr;
                }

                // Look for gaps in the live range.
                bi.live_out = true;
                while segs[lvi].end < stop {
                    let last_stop = segs[lvi].end;
                    lvi += 1;
                    if lvi == lve || segs[lvi].start >= stop {
                        bi.live_out = false;
                        bi.last_instr = last_stop;
                        break;
                    }

                    if last_stop < segs[lvi].start {
                        // There is a gap in the live range. Create duplicate
                        // entries for the live-in snippet and the live-out
                        // snippet.
                        self.num_gap_blocks += 1;

                        // Push the live-in part.
                        bi.live_out = false;
                        self.use_blocks.push(BlockInfo { last_instr: last_stop, ..bi });

                        // Set up `bi` for the live-out part.
                        bi.live_in = false;
                        bi.live_out = true;
                        bi.first_instr = segs[lvi].start;
                        bi.first_def = bi.first_instr;
                    }

                    // A segment that starts in the middle of the block must be
                    // a def.
                    assert!(segs[lvi].start == def_of(&segs[lvi]), "Dangling Segment start");
                    if !bi.first_def.is_valid() {
                        bi.first_def = segs[lvi].start;
                    }
                }

                self.use_blocks.push(bi);

                // `lvi` is now at the end or `end >= stop`.
                if lvi == lve {
                    break;
                }
            }

            // Live segment ends exactly at `stop`. Move to the next segment.
            if segs[lvi].end == stop {
                lvi += 1;
                if lvi == lve {
                    break;
                }
            }

            // Pick the next basic block.
            if segs[lvi].start < stop {
                mfi += 1;
            } else {
                mfi = lis.indexes.get_mbb_from_index(&segs[lvi].start);
            }
        }

        self.looks_like_loop_iv = ENABLE_LOOP_IV_HEURISTIC
            && self.use_blocks.len() == 2
            && self
                .use_blocks
                .iter()
                .any(|bi| bi.live_in && bi.live_out && bi.first_def.is_valid() && env.is_loop_latch(bi.mbb));

        assert!(self.get_num_live_blocks() == self.count_live_blocks(lis, cur), "Bad block count");
    }

    /// `countLiveBlocks`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L309>).
    pub fn count_live_blocks(
        &self,
        lis: &LiveIntervals,
        cli: &LiveInterval,
    ) -> u32 {
        if cli.empty() {
            return 0;
        }
        let mut lvi = 0usize;
        let mut count = 0;

        // Loop over basic blocks where the range is live.
        let mut mfi = lis.indexes.get_mbb_from_index(&cli.segments[0].start);
        let mut stop = lis.indexes.get_mbb_end_idx(mfi);
        loop {
            count += 1;
            lvi = cli.advance_to(lvi, &stop);
            if lvi == cli.segments.len() {
                return count;
            }
            loop {
                mfi += 1;
                stop = lis.indexes.get_mbb_end_idx(mfi);
                if stop > cli.segments[lvi].start {
                    break;
                }
            }
        }
    }

    /// `isOriginalEndpoint`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L333>).
    pub fn is_original_endpoint(
        &self,
        lis: &LiveIntervals,
        vrm: &VirtRegMap,
        idx: SlotIndex,
    ) -> bool {
        let orig_reg = vrm.get_original(self.get_parent().reg());
        let orig = lis.get_interval(orig_reg);
        assert!(!orig.empty(), "Splitting empty interval?");
        let i = orig.find(&idx);

        // Range containing idx should begin at idx.
        if i != orig.segments.len() && orig.segments[i].start <= idx {
            return orig.segments[i].start == idx;
        }

        // Range does not contain idx, previous must end at idx.
        i != 0 && orig.segments[i - 1].end == idx
    }

    /// `shouldSplitSingleBlock`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1630>).
    pub fn should_split_single_block(
        &self,
        lis: &LiveIntervals,
        vrm: &VirtRegMap,
        env: &dyn SplitEnv,
        bi: &BlockInfo,
        single_instrs: bool,
    ) -> bool {
        // Always split for multiple instructions.
        if !bi.is_one_instr() {
            return true;
        }
        // Don't split for single instructions unless explicitly requested.
        if !single_instrs {
            return false;
        }
        // Splitting a live-through range always makes progress.
        if bi.live_in && bi.live_out {
            return true;
        }
        // No point in isolating a copy. It has no register class constraints.
        let mi = lis.indexes.get_instruction_from_index(&bi.first_instr).expect("an instruction");
        if env.is_copy_like(lis.instr(mi)) {
            return false;
        }
        // Finally, don't isolate an end point that was created by earlier
        // splits.
        self.is_original_endpoint(lis, vrm, bi.first_instr)
    }

    /// `getParent`.
    pub fn get_parent(&self) -> &LiveInterval {
        self.cur_li.as_ref().expect("analyzed")
    }

    /// `getUseSlots`.
    pub fn get_use_slots(&self) -> &[SlotIndex] {
        &self.use_slots
    }

    /// `getUseBlocks`.
    pub fn get_use_blocks(&self) -> &[BlockInfo] {
        &self.use_blocks
    }

    /// `getNumThroughBlocks`.
    pub fn get_num_through_blocks(&self) -> u32 {
        self.num_through_blocks
    }

    /// `isThroughBlock`.
    pub fn is_through_block(
        &self,
        mbb: usize,
    ) -> bool {
        self.through_blocks[mbb]
    }

    /// `getThroughBlocks`.
    pub fn get_through_blocks(&self) -> &[bool] {
        &self.through_blocks
    }

    /// `getNumLiveBlocks`.
    pub fn get_num_live_blocks(&self) -> u32 {
        self.use_blocks.len() as u32 - self.num_gap_blocks + self.num_through_blocks
    }

    /// `looksLikeLoopIV`.
    pub fn looks_like_loop_iv(&self) -> bool {
        self.looks_like_loop_iv
    }

    /// `getLastSplitPoint`.
    pub fn get_last_split_point(
        &mut self,
        lis: &LiveIntervals,
        env: &dyn SplitEnv,
        num: usize,
    ) -> SlotIndex {
        self.ipa.get_last_insert_point(lis, env, num)
    }

    /// `getFirstSplitPoint`.
    pub fn get_first_split_point(
        &self,
        lis: &LiveIntervals,
        num: usize,
    ) -> SlotIndex {
        self.ipa.get_first_insert_point(lis, num)
    }
}

/// `LVI->valno->def`.
fn def_of(seg: &crate::liveness::liverange::Segment) -> SlotIndex {
    seg.valno.as_ref().expect("a segment has a value").def()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    struct Env {
        terminators: Vec<(usize, InstrKey)>,
        latches: Vec<usize>,
    }

    impl SplitEnv for Env {
        fn is_terminator(
            &self,
            insn: &Insn,
        ) -> bool {
            self.terminators.iter().any(|(_, k)| *k == insn.id())
        }

        fn is_loop_latch(
            &self,
            block: usize,
        ) -> bool {
            self.latches.contains(&block)
        }

        fn note_split_from(
            &self,
            _lis: &LiveIntervals,
            _new: u32,
            _from: u32,
        ) {
        }

        fn is_rematerializable(
            &self,
            _reg: u32,
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
            _insn: &Insn,
        ) -> bool {
            false
        }

        fn build_copy(
            &self,
            _lis: &LiveIntervals,
            _to: u32,
            _from: u32,
            _beside: Option<&Insn>,
        ) -> Arc<Insn> {
            unreachable!("the analysis builds no copy")
        }
    }

    fn insn(
        defines: &[u32],
        uses: &[u32],
    ) -> Arc<Insn> {
        Arc::new(Insn::new(0, None, None, defines.to_vec(), uses.to_vec()))
    }

    /// A chain of blocks `b0 -> b1 -> ...`, each a list of (defines, uses).
    fn chain(blocks: &[&[(&[u32], &[u32])]]) -> (LiveIntervals, Vec<Vec<Arc<Insn>>>) {
        let insns: Vec<Vec<Arc<Insn>>> = blocks.iter().map(|b| b.iter().map(|(d, u)| insn(d, u)).collect()).collect();
        let lir = insns
            .iter()
            .enumerate()
            .map(|(n, i)| {
                let mut b = LirBlock::new(n as i64, i.clone());
                if n + 1 < blocks.len() {
                    b.succ = vec![n as i64 + 1].into();
                }
                Block::from(b)
            })
            .collect();
        let body = LirBody::new("split", 0, lir, IndexMap::default(), IndexMap::default());
        (LiveIntervals::new(&body), insns)
    }

    fn analyzed(
        lis: &mut LiveIntervals,
        env: &Env,
        reg: u32,
        blocks: usize,
    ) -> SplitAnalysis {
        let li = lis.interval_rc(reg);
        let mut sa = SplitAnalysis::new(blocks);
        sa.analyze(lis, env, li);
        sa
    }

    fn quiet() -> Env {
        Env { terminators: vec![], latches: vec![] }
    }

    #[test]
    fn a_range_with_uses_at_both_ends_has_a_through_block_between() {
        let (mut lis, insns) = chain(&[&[(&[1], &[]), (&[], &[1])], &[(&[2], &[])], &[(&[], &[1])]]);
        let sa = analyzed(&mut lis, &quiet(), 1, 3);
        let ub = sa.get_use_blocks();
        assert_eq!(ub.len(), 2);
        assert_eq!((ub[0].mbb, ub[0].live_in, ub[0].live_out), (0, false, true));
        assert!(ub[0].first_def.is_valid());
        assert_eq!((ub[1].mbb, ub[1].live_in, ub[1].live_out), (2, true, false));
        assert!(!ub[1].first_def.is_valid());
        assert!(sa.is_through_block(1) && !sa.is_through_block(0));
        assert_eq!((sa.get_num_through_blocks(), sa.get_num_live_blocks()), (1, 3));
        let first_use = lis.indexes.get_instruction_index(insns[0][1].id()).get_reg_slot(false);
        assert_eq!(ub[0].last_instr, first_use);
    }

    /// An instruction naming the value twice is one use slot; counting it
    /// twice would make a single-instruction block look like two instructions.
    #[test]
    fn one_instruction_naming_the_value_twice_is_one_use() {
        let (mut lis, _) = chain(&[&[(&[1], &[]), (&[2], &[1, 1])]]);
        let sa = analyzed(&mut lis, &quiet(), 1, 1);
        assert_eq!(sa.get_use_slots().len(), 2);
        assert!(!sa.get_use_blocks()[0].is_one_instr());
    }

    #[test]
    fn a_hole_in_the_range_lists_the_block_twice() {
        let (mut lis, _) = chain(&[&[(&[1], &[]), (&[], &[1]), (&[3], &[]), (&[1], &[]), (&[], &[1])]]);
        let sa = analyzed(&mut lis, &quiet(), 1, 1);
        let ub = sa.get_use_blocks();
        assert_eq!(ub.len(), 2);
        assert!(!ub[0].live_out && !ub[0].live_in && ub[0].first_def.is_valid());
        assert!(!ub[1].live_in && !ub[1].live_out);
        assert!(ub[0].last_instr < ub[1].first_instr);
        // The two snippets are one block: the count of live blocks holds it.
        assert_eq!(sa.get_num_live_blocks(), 1);
    }

    #[test]
    fn last_split_point_is_the_first_terminator_or_the_block_end() {
        let (mut lis, insns) = chain(&[&[(&[1], &[]), (&[], &[1]), (&[], &[])], &[(&[], &[1])]]);
        let env = Env { terminators: vec![(0, insns[0][2].id())], latches: vec![] };
        let mut sa = analyzed(&mut lis, &env, 1, 2);
        assert_eq!(sa.get_last_split_point(&lis, &env, 0), lis.indexes.get_instruction_index(insns[0][2].id()));
        assert_eq!(sa.get_last_split_point(&lis, &env, 1), lis.indexes.get_mbb_end_idx(1));
        assert_eq!(sa.get_first_split_point(&lis, 0), lis.indexes.get_instruction_index(insns[0][0].id()));
    }

    #[test]
    fn single_instruction_blocks_split_only_when_asked() {
        let (mut lis, _) = chain(&[&[(&[1], &[]), (&[], &[1])], &[(&[], &[1])]]);
        let env = quiet();
        let sa = analyzed(&mut lis, &env, 1, 2);
        let vrm = VirtRegMap::new();
        let bi = sa.get_use_blocks()[1];
        assert!(bi.is_one_instr());
        assert!(!sa.should_split_single_block(&lis, &vrm, &env, &bi, false));
        // Not live out, live in, a single instruction: only an original
        // endpoint is worth isolating.
        assert!(sa.should_split_single_block(&lis, &vrm, &env, &bi, true));
        assert!(sa.should_split_single_block(&lis, &vrm, &env, &sa.get_use_blocks()[0], false));
    }
}
