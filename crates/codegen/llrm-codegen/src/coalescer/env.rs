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

use llrm_lir::registers::RegId;

use super::driver;
use super::pair::CoalescerEnv;
use crate::liveness::liveintervals::LiveIntervals;
use crate::model::lir::LirBody;
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
    /// For each pinned value, the pinned values on overlapping registers.
    pub aliases: IndexMap<u32, Vec<u32>>,
}

impl CoalescerEnv for TargetEnv {
    fn tuned_for_size(&self) -> bool {
        self.size
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

/// `body` with its copies joined by the translated coalescer.
pub fn joined(
    body: &LirBody,
    env: &dyn CoalescerEnv,
    depth: &[u32],
) -> LirBody {
    // The intervals TwoAddress edited, where it ran just before.
    let mut lis = crate::liveness::liveintervals::take_handed_over(body).unwrap_or_else(|| LiveIntervals::new(body));
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
    after
}

/// `body` with its copies joined, for a caller that states the palettes and
/// pins itself (a target whose registers are not x86 classes). The palettes
/// come back narrowed to what each surviving value may take.
pub fn coalesce(
    body: &LirBody,
    palettes: IndexMap<u32, BTreeSet<RegId>>,
    pins: IndexMap<u32, RegId>,
    size: bool,
    regs: llrm_lir::registers::Regs,
) -> (LirBody, IndexMap<u32, BTreeSet<RegId>>) {
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
    let env = TargetEnv { palettes: palettes.into(), pins, size, keep_pinned: true, aliases };
    let depths = crate::analysis::intervals::depths_shared(body);
    let depth: Vec<u32> = body.blocks.iter().map(|block| depths.get(&block.at).copied().unwrap_or(0)).collect();
    let after = joined(body, &env, &depth);
    (after, env.palettes.into_inner())
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
