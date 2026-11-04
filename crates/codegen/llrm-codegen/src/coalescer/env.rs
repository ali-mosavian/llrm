//! What the translated coalescer asks of the target, and the pass over a body.
//!
//! # How it works
//! `TargetEnv` answers `CoalescerEnv::can_share` from the old coalescer's own
//! facts: two values may share a register when the registers each may take
//! (their class palettes) have one in common and they are not pinned to
//! different registers. A join narrows the surviving value's palette to the
//! registers both had, so the next join is judged against the merged value.
//! `joined` numbers the body, builds its live intervals,
//! runs the driver and returns the body with the copies joined away.
//! `LLRM_CHECK_LIVEINTERVALS=1` checks every interval the joins left against
//! one made afresh (points live, touching segments as one).
//!
//! History, not a deviation: the old coalescer also refused a join whose merged
//! interval crossed a call that destroys every register the values may take,
//! and joined only where Briggs or George said the colouring stayed easy. LLVM
//! does neither and relies on the allocator to split; so does this (rows: the
//! translation wins `d_faces` -2.4% clocks and -6.5% code at -O3 and loses
//! some recursive m16 files, the allocator's splitting being the difference).
//!
//! # Example
//! `v2 := mov v1; ...; read v2` with palettes that share a register: one value.
//!
//! # Used by
//! `coalescer::pass`.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::{RegId, Regs};

use super::driver;
use super::pair::CoalescerEnv;
use crate::liveness::liveintervals::{Change, LiveIntervals};
use crate::liveness::slotindex::InstrKey;
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::IndexMap;

/// The registers each value may take, and the ones that are fixed.
pub struct TargetEnv {
    pub palettes: RefCell<IndexMap<u32, BTreeSet<RegId>>>,
    pub pins: IndexMap<u32, RegId>,
    pub size: bool,
    /// A join leaves a pinned value standing ([`CoalescerEnv::keeps`]): for a
    /// caller whose pins are values of their own, not constraints on an
    /// instruction.
    pub keep_pinned: bool,
    /// `JoinGlobalCopies` (`CoalescerEnv::joins_global_copies`).
    pub joins_global_copies: bool,
    /// `CoalescerEnv::joins_pinned_copies`.
    pub joins_pinned_copies: bool,
    /// DEVIATE C13 (`CoalescerEnv::joins_sub_registers`).
    pub joins_sub_registers: bool,
    /// Whether a copy into or out of a lane above the low one is joined
    /// ([`CoalescerEnv::can_share_lane`]). Off: such a copy stays.
    pub joins_lanes: bool,
    /// For each pinned value, the pinned values on overlapping registers.
    pub aliases: IndexMap<u32, Vec<u32>>,
    /// `TargetRegisterInfo::shouldCoalesce`: whether the join goes ahead.
    pub should_join: Option<Box<dyn Fn(&Join) -> bool>>,
    /// The frequency of each block relative to the entry block
    /// (`MBFI->getBlockFreqRelativeToEntryBlock`); a block it does not list
    /// runs once.
    pub freq: Vec<f32>,
    /// The target's registers, for a join of a value into the wide value it is
    /// the low part of ([`CoalescerEnv::can_share_sub`]); none where the
    /// target builds no copy between values of two widths.
    pub regs: Option<Regs>,
    /// What one instruction asks of the registers of a value it names: an id
    /// the target interns (the operand slots that name it), or none where the
    /// instruction costs the same in every register. Priced by
    /// [`Join`]'s `kinds`, not by walking the instructions.
    pub kind_of: Option<Box<dyn Fn(&Insn, u32) -> Option<u32>>>,
    /// Each value's [`UseKinds`], built the first time it is priced and merged
    /// when a join absorbs another (DEVIATE C14).
    pub kinds: RefCell<IndexMap<u32, Kept>>,
    /// `LLRM_CHECK_USE_SUMMARY`: each [`JoinSide`] also lists its instructions,
    /// for the target to check the summary against.
    pub check_kinds: bool,
    /// `TII->isAsCheapAsAMove(DefMI) && TII->isReMaterializable(DefMI)`: the
    /// target's own test of a definition worth making again. None: a move of
    /// an immediate or an address as x86 names them.
    pub remat: Option<Box<dyn Fn(&Insn) -> bool>>,
    /// The target's price of that remat against the copy it replaces: the
    /// definition, the registers the copy's source and destination may take,
    /// and whether the build is for size. None: always worth it.
    pub remat_wins: Option<RematWins>,
}

/// The instructions that name a value, summed by what they ask of its
/// registers: the target's kind id and the frequency of their blocks.
pub type UseKinds = IndexMap<u32, f32>;

/// [`UseKinds`] with how many instructions each kind sums: a kind goes when its
/// count reaches 0, not when its sum does (f32 sums of hot blocks keep a
/// residue of 1e-3 after many puts and takes).
pub type Kept = IndexMap<u32, Tally>;

#[derive(Clone, Copy, Debug)]
pub struct Tally {
    pub count: u32,
    pub sum: f32,
}

fn sums(kept: &Kept) -> UseKinds {
    kept.iter().map(|(&kind, tally)| (kind, tally.sum)).collect()
}

impl TargetEnv {
    /// `value`'s [`UseKinds`]: kept, or made once from its instructions.
    fn use_kinds(
        &self,
        lis: &LiveIntervals,
        value: u32,
    ) -> Kept {
        if let Some(kept) = self.kinds.borrow().get(&value) {
            if self.check_kinds {
                let now = self.scan_kinds(lis, value);
                let same = now.len() == kept.len()
                    && now.iter().all(|(kind, now)| {
                        kept.get(kind)
                            .is_some_and(|k| k.count == now.count && (k.sum - now.sum).abs() <= 1e-3 * (1.0 + now.sum))
                    });
                assert!(same, "value {value}: the kept uses {kept:?} are not the instructions' {now:?}");
            }
            return kept.clone();
        }
        let kinds = self.scan_kinds(lis, value);
        self.kinds.borrow_mut().insert(value, kinds.clone());
        kinds
    }

    /// What `insn` asks of the registers of `value`, where the target prices
    /// it.
    fn kind(
        &self,
        insn: &Insn,
        value: u32,
    ) -> Option<u32> {
        self.kind_of.as_ref().and_then(|kind_of| kind_of(insn, value))
    }

    /// Applies the instructions put in and taken out since the last pricing to
    /// the kept lists: each value an instruction names gains or loses what the
    /// instruction asks of it. A value with no kept list is made afresh, from
    /// the instructions as they are now.
    fn catch_up(
        &self,
        lis: &LiveIntervals,
    ) {
        let changes = lis.take_changes();
        if changes.is_empty() {
            return;
        }
        let mut all = self.kinds.borrow_mut();
        for change in changes {
            let (block, insn, sign) = match &change {
                Change::Put(block, insn) => (*block, insn, 1.0),
                Change::Took(block, insn) => (*block, insn, -1.0),
            };
            let freq = self.freq.get(block).copied().unwrap_or(1.0);
            let named = insn.defines.iter().chain(&insn.uses).chain(&insn.undefs);
            let mut seen = crate::support::hash::HashSet::default();
            for &value in named {
                if !seen.insert(value) {
                    continue;
                }
                let Some(kinds) = all.get_mut(&value) else { continue };
                let Some(kind) = self.kind(insn, value) else { continue };
                if sign > 0.0 {
                    let tally = kinds.entry(kind).or_insert(Tally { count: 0, sum: 0.0 });
                    tally.count += 1;
                    tally.sum += freq;
                } else {
                    self.take_away(kinds, kind, freq);
                }
            }
        }
    }

    /// `value`'s [`UseKinds`] from the instructions that name it.
    fn scan_kinds(
        &self,
        lis: &LiveIntervals,
        value: u32,
    ) -> Kept {
        let mut seen = crate::support::hash::HashSet::default();
        let mut kinds = Kept::default();
        for op in lis.operands(value) {
            if !seen.insert(op.instr) {
                continue;
            }
            if let Some(kind) = self.kind(lis.instr(op.instr), value) {
                let block = lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(op.instr));
                let tally = kinds.entry(kind).or_insert(Tally { count: 0, sum: 0.0 });
                tally.count += 1;
                tally.sum += self.freq.get(block).copied().unwrap_or(1.0);
            }
        }
        kinds
    }

    /// `kinds` less one instruction of `kind` that runs `freq` times.
    fn take_away(
        &self,
        kinds: &mut Kept,
        kind: u32,
        freq: f32,
    ) {
        if let Some(tally) = kinds.get_mut(&kind) {
            tally.count -= 1;
            tally.sum -= freq;
            if tally.count == 0 {
                kinds.swap_remove(&kind);
            }
        }
    }

    /// The registers `wide` may take whose low part `narrow` may take
    /// (`getMatchingSuperRegClass`): a register of `narrow`'s palette that is
    /// the bottom bytes of the same root register, or the register itself
    /// where the target names one id for every width of it.
    fn hosts(
        &self,
        wide: &BTreeSet<RegId>,
        narrow: &BTreeSet<RegId>,
    ) -> BTreeSet<RegId> {
        self.hosts_at(wide, narrow, 0)
    }

    /// `hosts` for the byte `offset` of the wide register: the narrow one is
    /// that many bytes above the wide one's lowest.
    fn hosts_at(
        &self,
        wide: &BTreeSet<RegId>,
        narrow: &BTreeSet<RegId>,
        offset: u32,
    ) -> BTreeSet<RegId> {
        let Some(regs) = &self.regs else { return BTreeSet::new() };
        let low = |register: RegId| regs.lanes(register);
        wide.iter()
            .copied()
            .filter(|&big| {
                narrow
                    .iter()
                    .any(
                        |&small| regs.root(small) == regs.root(big)
                            && low(small) & !low(big) == 0
                            && low(small).trailing_zeros() == low(big).trailing_zeros() + offset,
                    )
            })
            .collect()
    }
}

/// See [`TargetEnv::remat_wins`].
pub type RematWins = Box<dyn Fn(&Insn, &BTreeSet<RegId>, &BTreeSet<RegId>, bool) -> bool>;

/// One side of a copy about to be joined.
pub struct JoinSide {
    pub value: u32,
    /// The registers it may take.
    pub palette: BTreeSet<RegId>,
    /// The instructions that name it, the copy apart, by kind: what the join
    /// would leave a narrower class to serve.
    pub kinds: UseKinds,
    /// The same instructions one by one with the frequency of their block;
    /// only under `LLRM_CHECK_USE_SUMMARY`, empty otherwise.
    pub uses: Vec<(Arc<Insn>, f32)>,
}

/// What `shouldCoalesce` is asked: the two sides of the copy and the
/// registers the joined value may take (the palettes' common ones).
pub struct Join {
    pub one: JoinSide,
    pub other: JoinSide,
    pub joined: BTreeSet<RegId>,
    /// The frequency of the copy's block: what one copy the join removes
    /// saves.
    pub copy_frequency: f32,
}

impl CoalescerEnv for TargetEnv {
    fn joins_global_copies(&self) -> bool {
        self.joins_global_copies
    }

    fn joins_pinned_copies(&self) -> bool {
        self.joins_pinned_copies
    }

    fn joins_sub_registers(&self) -> bool {
        self.joins_sub_registers
    }

    fn tuned_for_size(&self) -> bool {
        self.size
    }

    fn rematerializable(
        &self,
        insn: &Insn,
    ) -> bool {
        match &self.remat {
            Some(shape) => insn.spread.is_empty() && insn.call.is_none() && shape(insn),
            None => {
                insn.spread.is_empty()
                    && insn.call.is_none()
                    && crate::greedy::remat::is_trivially_re_materializable(insn, false)
            }
        }
    }

    fn remat_wins(
        &self,
        def: &Insn,
        source: u32,
        destination: u32,
    ) -> bool {
        let palettes = self.palettes.borrow();
        let (Some(wins), Some(from), Some(to)) = (&self.remat_wins, palettes.get(&source), palettes.get(&destination))
        else {
            return true;
        };
        wins(def, from, to, self.size)
    }

    fn keeps(
        &self,
        value: u32,
    ) -> bool {
        self.keep_pinned && self.pins.contains_key(&value)
    }

    fn aliases(
        &self,
        value: u32,
    ) -> Vec<u32> {
        self.aliases.get(&value).cloned().unwrap_or_default()
    }

    fn can_share(
        &self,
        a: u32,
        b: u32,
    ) -> bool {
        if let (Some(x), Some(y)) = (self.pins.get(&a), self.pins.get(&b)) {
            if x != y {
                return false;
            }
        }
        let palettes = self.palettes.borrow();
        match (palettes.get(&a), palettes.get(&b)) {
            (Some(x), Some(y)) => x.intersection(y).next().is_some(),
            _ => true,
        }
    }

    fn can_share_sub(
        &self,
        narrow: u32,
        wide: u32,
    ) -> bool {
        if let (Some(x), Some(y)) = (self.pins.get(&narrow), self.pins.get(&wide)) {
            if self.regs.as_ref().is_none_or(|regs| regs.root(*x) != regs.root(*y)) {
                return false;
            }
        }
        let palettes = self.palettes.borrow();
        match (palettes.get(&wide), palettes.get(&narrow)) {
            (Some(big), Some(small)) => !self.hosts(big, small).is_empty(),
            _ => self.regs.is_some(),
        }
    }

    fn can_share_lane(
        &self,
        narrow: u32,
        wide: u32,
        offset: u32,
    ) -> bool {
        if !self.joins_lanes {
            return false;
        }
        if let (Some(x), Some(y)) = (self.pins.get(&narrow), self.pins.get(&wide)) {
            if self.regs.as_ref().is_none_or(|regs| regs.root(*x) != regs.root(*y)) {
                return false;
            }
        }
        let palettes = self.palettes.borrow();
        match (palettes.get(&wide), palettes.get(&narrow)) {
            (Some(big), Some(small)) => !self.hosts_at(big, small, offset).is_empty(),
            _ => self.regs.is_some(),
        }
    }

    fn narrow_lane(
        &self,
        wide: u32,
        narrow: u32,
        offset: u32,
    ) {
        let mut palettes = self.palettes.borrow_mut();
        let Some(small) = palettes.get(&narrow).cloned() else { return };
        if let Some(big) = palettes.get(&wide).cloned() {
            palettes.insert(wide, self.hosts_at(&big, &small, offset));
        }
    }

    fn narrow_sub(
        &self,
        wide: u32,
        narrow: u32,
    ) {
        let mut palettes = self.palettes.borrow_mut();
        let Some(small) = palettes.get(&narrow).cloned() else { return };
        if let Some(big) = palettes.get(&wide).cloned() {
            palettes.insert(wide, self.hosts(&big, &small));
        }
    }

    fn should_coalesce(
        &self,
        lis: &LiveIntervals,
        copy: InstrKey,
        src: u32,
        dst: u32,
    ) -> bool {
        // `TargetRegisterInfo::shouldCoalesce` is no work where the target has
        // no opinion: no `Join` is built for it.
        let Some(should_join) = &self.should_join else { return true };
        let palettes = self.palettes.borrow();
        let (Some(x), Some(y)) = (palettes.get(&src), palettes.get(&dst)) else { return true };
        let freq = |key: InstrKey| {
            let block = lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(key));
            self.freq.get(block).copied().unwrap_or(1.0)
        };
        self.catch_up(lis);
        let side = |value: u32, palette: &BTreeSet<RegId>| {
            let mut kinds = self.use_kinds(lis, value);
            // The copy is no use of the join it is the object of.
            if let Some(kind) = self.kind(lis.instr(copy), value) {
                self.take_away(&mut kinds, kind, freq(copy));
            }
            let uses = if self.check_kinds {
                let mut seen = crate::support::hash::HashSet::default();
                lis.operands(value)
                    .iter()
                    .filter(|op| op.instr != copy && seen.insert(op.instr))
                    .map(|op| (Arc::clone(lis.instr(op.instr)), freq(op.instr)))
                    .collect()
            } else {
                Vec::new()
            };
            JoinSide { value, palette: palette.clone(), kinds: sums(&kinds), uses }
        };
        // A partial pair's `src` is the narrow value: the joined value takes
        // the wide registers that have its registers as their low part.
        let widths = super::pair::move_copy(lis.instr(copy)).map(|(d, s)| d.width != s.width);
        let joined = if widths == Some(true) { self.hosts(y, x) } else { x.intersection(y).copied().collect() };
        JOINS_BUILT.with(|built| built.set(built.get() + 1));
        should_join(&Join { one: side(src, x), other: side(dst, y), joined, copy_frequency: freq(copy) })
    }

    fn tracks_uses(&self) -> bool {
        self.kind_of.is_some()
    }

    fn narrow(
        &self,
        dst: u32,
        src: u32,
    ) {
        let mut palettes = self.palettes.borrow_mut();
        let Some(from) = palettes.get(&src).cloned() else { return };
        match palettes.get_mut(&dst) {
            Some(into) => into.retain(|register| from.contains(register)),
            None => {
                palettes.insert(dst, from);
            }
        }
    }
}

/// Where each joined value went.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct JoinMap(IndexMap<u32, u32>);

impl JoinMap {
    /// The value that stands for `value` in the joined body: itself if it was
    /// not absorbed, else the survivor of the joins it went through.
    pub fn survivor(
        &self,
        value: u32,
    ) -> u32 {
        let mut now = value;
        while let Some(&next) = self.0.get(&now) {
            now = next;
        }
        now
    }

    /// Every absorbed value with its survivor.
    pub fn iter(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.0.keys().map(|&value| (value, self.survivor(value)))
    }
}

thread_local! {
    /// How many [`Join`]s `should_coalesce` has built on this thread, for a
    /// test that a target with no opinion costs none.
    pub static JOINS_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `body` with its copies joined by the translated coalescer.
pub fn joined(
    body: &LirBody,
    env: &dyn CoalescerEnv,
    depth: &[u32],
) -> LirBody {
    joined_with_map(body, env, depth).0
}

/// [`joined`], and where each absorbed value went.
pub fn joined_with_map(
    body: &LirBody,
    env: &dyn CoalescerEnv,
    depth: &[u32],
) -> (LirBody, JoinMap) {
    // The intervals TwoAddress edited, where it ran just before.
    let mut lis = crate::liveness::liveintervals::take_handed_over(body).unwrap_or_else(|| LiveIntervals::new(body));
    if env.tracks_uses() {
        lis.track_changes();
    }
    let stats = driver::run(&mut lis, env, depth);
    let after = lis.body_like(body);
    if llrm_support::env_flag!("LLRM_COALESCE_WHY") {
        for reason in why_left(&mut lis, env) {
            eprintln!("WHY\t{}\t{reason}", body.name);
        }
    }
    if llrm_support::env_flag!("LLRM_CHECK_LIVEINTERVALS") && stats.joined > 0 {
        // The premise: the intervals the joins left are the ones made afresh.
        let compared = lis.assert_intervals_cover_afresh(&after);
        assert!(compared > 0, "{}: the check compared no interval", body.name);
    }
    // The allocator starts from these (`take_handed_over_edited_to`).
    crate::liveness::liveintervals::hand_over(lis, &after);
    (after, JoinMap(stats.renames.into_iter().collect()))
}

/// `body` with its copies joined, for a caller that states the palettes and
/// pins itself (a target whose registers are not x86 classes). The palettes
/// come back narrowed to what each surviving value may take.
pub fn coalesce(
    body: &LirBody,
    palettes: IndexMap<u32, BTreeSet<RegId>>,
    pins: IndexMap<u32, RegId>,
    size: bool,
    joins_global_copies: bool,
    joins_pinned_copies: bool,
    joins_sub_registers: bool,
    joins_lanes: bool,
    regs: llrm_lir::registers::Regs,
    should_join: Option<Box<dyn Fn(&Join) -> bool>>,
    remat: Option<Box<dyn Fn(&Insn) -> bool>>,
    remat_wins: Option<RematWins>,
    kind_of: Box<dyn Fn(&Insn, u32) -> Option<u32>>,
) -> (LirBody, IndexMap<u32, BTreeSet<RegId>>, JoinMap) {
    let aliases = pins
        .iter()
        .map(|(value, register)| {
            let others = pins
                .iter()
                .filter(|(other, theirs)| {
                    *other != value
                        && regs.root(**theirs) == regs.root(*register)
                        && regs.lanes(**theirs) & regs.lanes(*register) != 0
                })
                .map(|(other, _)| *other)
                .collect();
            (*value, others)
        })
        .collect();
    let mbfi = crate::liveness::blockfrequency::MachineBlockFrequencyInfo::of(body);
    let freq = (0..body.blocks.len()).map(|block| mbfi.get_block_freq_relative_to_entry_block(block) as f32).collect();
    let env = TargetEnv {
        palettes: palettes.into(),
        pins,
        size,
        keep_pinned: true,
        joins_global_copies,
        joins_pinned_copies,
        joins_sub_registers,
        joins_lanes,
        aliases,
        should_join,
        freq,
        remat,
        remat_wins,
        regs: Some(regs),
        kind_of: Some(kind_of),
        kinds: RefCell::default(),
        check_kinds: llrm_support::env_flag!("LLRM_CHECK_USE_SUMMARY"),
    };
    let depths = crate::analysis::intervals::depths_shared(body);
    let depth: Vec<u32> = body.blocks.iter().map(|block| depths.get(&block.at).copied().unwrap_or(0)).collect();
    let (after, map) = joined_with_map(body, &env, &depth);
    (after, env.palettes.into_inner(), map)
}

/// `LLRM_COALESCE_WHY=1`: the reason for each register copy the joins left. A
/// full copy is probed by running `join_copy` on the final state; a probe that
/// joins changes the state the next sees, so the reasons are a count, not a
/// replay. A store of a register is no copy.
pub(super) fn why_left(
    lis: &mut LiveIntervals,
    env: &dyn CoalescerEnv,
) -> Vec<&'static str> {
    use llrm_support::hash::HashSet;

    use super::pair::full_copy;
    use super::register_coalescer::{JoinResult, State, join_copy};
    use crate::model::ir::{Loc, Operation};
    let mut copies = Vec::new();
    for block in 0..lis.cfg.preds.len() {
        for one in lis.block_insns(block) {
            let Some(what) = one.what.as_ref() else { continue };
            if what.op != Operation::Move || what.dests.len() != 1 || what.sources.len() != 1 {
                continue;
            }
            let register = |loc: &Loc| matches!(loc, Loc::Held(_) | Loc::Reg(_));
            if register(&what.dests[0]) && register(&what.sources[0]) {
                copies.push(std::sync::Arc::clone(one));
            }
        }
    }
    let mut state = State::default();
    let mut reasons = Vec::new();
    for one in copies {
        let what = one.what.as_ref().unwrap();
        reasons.push(if one.group.is_some() {
            "parallel-copy member"
        } else if !matches!(
            (&what.dests[0], &what.sources[0]),
            (Loc::Held(_), Loc::Held(_))
        ) {
            "physical register"
        } else if !one.def_lanes.is_empty() {
            match super::pair::CoalescerPair::set_registers(&one, env) {
                None => "lane: can_share refused",
                Some(cp) if cp.is_partial() => {
                    super::register_coalescer::lane_join_blocker(lis, &cp, one.id()).unwrap_or("lane: joinable")
                }
                Some(_) => "lane: whole pair",
            }
        } else if full_copy(&one).is_none() {
            "width differs"
        } else if super::pair::CoalescerPair::set_registers(&one, env).is_none() {
            "can_share refused"
        } else {
            match join_copy(lis, env, &mut state, &mut HashSet::default(), one.id()) {
                JoinResult::Joined => "joinable now (not tried)",
                JoinResult::Deferred => "conflict (deferred)",
                JoinResult::Rejected => "rejected (high cost)",
            }
        });
    }
    reasons
}
