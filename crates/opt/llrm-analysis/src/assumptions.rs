//! What each block assumes, as LLVM's AssumptionCache: the conditions of the
//! `llvm.assume` calls in it, found once, and by the blocks above a block
//! in the dominator tree what holds there. A block's own assumes cover what
//! follows it, not the code before them.

use std::collections::BTreeMap;

use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::Operand;

use crate::cfg::{self, Shape};
use crate::memory::Unit;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Assumptions {
    here: BTreeMap<i64, Vec<Operand>>,
}

thread_local! {
    static BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has gone through a body for its assumptions, for a test that a pass asks once and not per query.
pub fn built() -> usize {
    BUILT.with(std::cell::Cell::get)
}

impl Assumptions {
    pub fn of(unit: &Unit) -> Self {
        BUILT.with(|built| built.set(built.get() + 1));
        let mut here = BTreeMap::<i64, Vec<Operand>>::new();
        for (block, inst) in unit.function.walk() {
            if unit.intrinsic(inst) == Some(Intrinsic::Assume)
                && let Some(&condition) = unit.function.instruction(inst).operands.first()
            {
                here.entry(cfg::id(block)).or_default().push(condition);
            }
        }
        Self { here }
    }

    pub fn is_empty(&self) -> bool {
        self.here.is_empty()
    }

    /// What block `at` assumes.
    pub fn here(&self, at: i64) -> &[Operand] {
        self.here.get(&at).map_or(&[], Vec::as_slice)
    }

    /// What the blocks strictly above `at` assume, nearest first.
    pub fn above(&self, shape: &Shape, at: i64) -> Vec<Operand> {
        let mut found = Vec::new();
        let mut reached = at;
        while !self.is_empty()
            && let Some(up) = shape.dominance.immediate(reached)
        {
            found.extend(self.here(up).iter().rev());
            reached = up;
        }
        found
    }
}
