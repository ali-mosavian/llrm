//! The translated coalescer run beside the old one, on the same input, to
//! compare what is left. It rewrites nothing the caller sees.
//!
//! # How it works
//! `run` numbers the body, builds its live intervals and runs the driver with
//! an environment made from the old coalescer's own facts: two values may
//! share a register when the registers each may take (their class palettes)
//! have one in common and they are not pinned to different registers. It
//! reports the copies left. `LLRM_SHADOW_COALESCE=1` prints one line per
//! function from `backend::coalesce`.
//!
//! DEVIATE C8 (exit: the greedy allocator is told, in the PR that wires this
//! in): the old coalescer also refuses a join whose merged interval crosses a
//! call that destroys every register the values may take, and joins only
//! where Briggs or George says the colouring stays easy. LLVM does neither and
//! relies on the allocator to split; so does this, and the shadow shows what
//! that is worth.
//!
//! # Example
//! `LLRM_SHADOW_COALESCE=1 llrm-c -O2 f.c` prints
//! `shadow-coalesce f: copies 30 old-left 4 new-left 3 joined 27`.
//!
//! # Used by
//! `backend::coalesce` (behind `LLRM_SHADOW_COALESCE`).

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;

use super::driver::{self, Stats};
use super::pair::{CoalescerEnv, full_copy};
use crate::liveness::liveintervals::LiveIntervals;
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

/// The registers each value may take, and the ones that are fixed.
pub struct ShadowEnv {
    pub palettes: IndexMap<u32, BTreeSet<RegId>>,
    pub pins: IndexMap<u32, RegId>,
}

impl CoalescerEnv for ShadowEnv {
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
        match (self.palettes.get(&a), self.palettes.get(&b)) {
            (Some(x), Some(y)) => x.intersection(y).next().is_some(),
            _ => true,
        }
    }
}

/// What the shadow found.
#[derive(Clone, Copy, Debug, Default)]
pub struct Report {
    pub copies: usize,
    pub left: usize,
    pub stats: Stats,
}

fn copies_in(body: &LirBody) -> usize {
    body.blocks.iter().flat_map(|block| block.insns.iter()).filter(|one| full_copy(one).is_some()).count()
}

/// Joins the copies of `body`; `depth` is the loop depth of each block.
pub fn run(
    body: &LirBody,
    env: &dyn CoalescerEnv,
    depth: &[u32],
) -> Report {
    let copies = copies_in(body);
    let mut lis = LiveIntervals::new(body);
    let stats = driver::run(&mut lis, env, depth);
    let after = lis.body_like(body);
    if llrm_support::env_flag!("LLRM_CHECK_LIVEINTERVALS") && stats.joined > 0 {
        // The premise: the intervals the joins left are the ones made afresh.
        let compared = lis.assert_intervals_cover_afresh(&after);
        assert!(compared > 0, "{}: the check compared no interval", body.name);
    }
    let left = copies_in(&after);
    Report { copies, left, stats }
}
