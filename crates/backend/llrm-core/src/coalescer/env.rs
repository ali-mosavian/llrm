//! What the translated coalescer asks of the target, and the pass over a body.
//!
//! # How it works
//! `TargetEnv` answers `CoalescerEnv::can_share` from the old coalescer's own
//! facts: two values may share a register when the registers each may take
//! (their class palettes) have one in common and they are not pinned to
//! different registers. `joined` numbers the body, builds its live intervals,
//! runs the driver and returns the body with the copies joined away.
//! `LLRM_CHECK_LIVEINTERVALS=1` checks every interval the joins left against
//! one made afresh (points live, touching segments as one).
//!
//! DEVIATE C8 (no number): the old coalescer also refused a join whose merged
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
//! `backend::coalesce`.

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;

use super::driver;
use super::pair::CoalescerEnv;
use crate::liveness::liveintervals::LiveIntervals;
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

/// The registers each value may take, and the ones that are fixed.
pub struct TargetEnv {
    pub palettes: IndexMap<u32, BTreeSet<RegId>>,
    pub pins: IndexMap<u32, RegId>,
    pub size: bool,
    /// POC: `Constrain`'s copies, which stay copies (the pin joins no
    /// non-reserved physical register).
    pub fixed: BTreeSet<u32>,
}

impl CoalescerEnv for TargetEnv {
    fn tuned_for_size(&self) -> bool {
        self.size
    }

    fn can_share(
        &self,
        a: u32,
        b: u32,
    ) -> bool {
        if self.fixed.contains(&a) || self.fixed.contains(&b) {
            return false;
        }
        if let (Some(x), Some(y)) = (self.pins.get(&a), self.pins.get(&b)) {
            if x != y {
                return false;
            }
        }
        match (self.palettes.get(&a), self.palettes.get(&b)) {
            (Some(x), Some(y)) => x.intersection(y).next().is_some(),
            _ => true,
        }
    }
}

/// `body` with its copies joined by the translated coalescer.
pub fn joined(
    body: &LirBody,
    env: &dyn CoalescerEnv,
    depth: &[u32],
) -> LirBody {
    let mut lis = LiveIntervals::new(body);
    let stats = driver::run(&mut lis, env, depth);
    let after = lis.body_like(body);
    if llrm_support::env_flag!("LLRM_CHECK_LIVEINTERVALS") && stats.joined > 0 {
        // The premise: the intervals the joins left are the ones made afresh.
        let compared = lis.assert_intervals_cover_afresh(&after);
        assert!(compared > 0, "{}: the check compared no interval", body.name);
    }
    if !llrm_support::env_flag!("LLRM_POC_NO_HANDOFF") {
        let keys = handoff_keys(&after);
        HANDOFF.with(|one| *one.borrow_mut() = Some((keys, lis)));
    }
    after
}

thread_local! {
    /// POC: the coalescer's intervals, for the allocator of the same body
    /// (the pin's LiveIntervals, preserved from RegisterCoalescer to RAGreedy).
    static HANDOFF: std::cell::RefCell<Option<(Vec<*const crate::model::lir::Insn>, LiveIntervals)>> =
        const { std::cell::RefCell::new(None) };
    pub static HANDOFF_HITS: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

fn handoff_keys(body: &LirBody) -> Vec<*const crate::model::lir::Insn> {
    body.blocks.iter().flat_map(|block| block.insns.iter().map(std::sync::Arc::as_ptr)).collect()
}

/// POC: the coalescer's intervals where `body` is the one it left, else new.
pub fn handed_or_new(body: &LirBody) -> LiveIntervals {
    let keys = handoff_keys(body);
    let taken = HANDOFF.with(|one| one.borrow_mut().take());
    match taken {
        Some((held, lis)) if held == keys => {
            HANDOFF_HITS.with(|n| n.set((n.get().0 + 1, n.get().1)));
            lis
        }
        _ => {
            HANDOFF_HITS.with(|n| n.set((n.get().0, n.get().1 + 1)));
            LiveIntervals::new(body)
        }
    }
}
