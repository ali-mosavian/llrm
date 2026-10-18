//! What each block assumes, as LLVM's AssumptionCache: the conditions of the
//! `llvm.assume` calls in it.
//!
//! # How it works
//! `Assumptions::of` walks the body once and records, per block, the first
//! operand of every call to the `Assume` intrinsic, in program order.
//!
//! `here(at)` returns what block `at` itself assumes. `above(shape, at)` climbs
//! the immediate-dominator chain from `at` and returns what each strict
//! dominator assumes, nearest block first and the last assume of a block
//! first.
//!
//! ```text
//!  b0: assume(c0)            above(b3) = [c1, c0]
//!   |
//!  b1: assume(c1)            here(b3)  = [c3]   (not in above(b3))
//!   |
//!  b3: assume(c3); use
//! ```
//!
//! A block's own assumes are not part of `above` for that block, so a use
//! before an assume in the same block is not covered by it. The caller reads
//! `here` for the part of the block it is in. `above` returns early when no
//! block assumes anything.
//!
//! # Example
//! The assume in `entry` holds in every block `entry` dominates:
//!
//! ```mir
//! declare void @llvm.assume(i1)
//!
//! define i16 @f(i16 %n, i16 %len, i1 %c) {
//! entry:
//!   %fits = icmp ult i16 %n, %len
//!   call void @llvm.assume(i1 %fits)
//!   br i1 %c, label %loop, label %done
//! loop:
//!   br label %done
//! done:
//!   ret i16 0
//! }
//! ```
//!
//! The test asserts that `guards::holds` proves `n <u len` in `loop` and in
//! `done`, and that `entry` itself has no guards: its own assume does not cover
//! earlier code.
//!
//! From unit test `test_an_assume_proves_its_compare_below_its_block` in
//! `guards_tests.rs`.
//!
//! # Provides
//! The assumed conditions per block. Consumers: [`crate::guards`] turns them
//! into `Guard`s, and [`crate::ranges`] narrows value ranges with them. The
//! manager entry is [`crate::manager::AssumptionCache`]; a `Unit` without one
//! builds its own on first ask, which `built()` counts for tests.
//!
//! # Needs
//! [`crate::cfg::Shape`] for the dominator tree.
//!
//! # See also
//! LLVM `AssumptionCache`.

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

/// How many times this thread has gone through a body for its assumptions, for
/// a test that a pass asks once and not per query.
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
    pub fn here(
        &self,
        at: i64,
    ) -> &[Operand] {
        self.here.get(&at).map_or(&[], Vec::as_slice)
    }

    /// What the blocks strictly above `at` assume, nearest first.
    pub fn above(
        &self,
        shape: &Shape,
        function: &llrm_mir::module::Function,
        at: i64,
    ) -> Vec<Operand> {
        let mut found = Vec::new();
        let mut reached = at;
        while !self.is_empty()
            && let Some(up) = shape.dominance.immediate(function, reached)
        {
            found.extend(self.here(up).iter().rev());
            reached = up;
        }
        found
    }
}
