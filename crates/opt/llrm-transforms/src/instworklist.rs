//! The instruction worklist of InstCombine and the passes that drive a
//! simplifier to a fixed point. Translated from LLVM's `InstructionWorklist`
//! (`llvm/include/llvm/Transforms/Utils/InstructionWorklist.h`).
//!
//! # How it works
//! Two lists of instructions, neither holding one twice.
//!
//! `push` puts an instruction on the stack; `remove_one` takes the last one
//! pushed. `add` puts it on the deferred list instead; `pop_deferred` takes the
//! last one added, so a driver that moves each deferred instruction to the
//! stack after a combine finishes visits them in the order they were added.
//! `remove` takes an instruction off both lists.
//!
//! Every operation is O(1), amortised: a removed entry leaves an empty slot
//! that the next pop from that list steps over, and a map from instruction to
//! slot finds it. LLVM's deferred list is a `SmallSetVector`, whose removal is
//! O(n); the stack and its map are as LLVM's.
//!
//! ```text
//! add a, add b, add c      pop_deferred -> c, b, a
//! push c, push b, push a   remove_one   -> a, b, c   (the order added)
//! ```
//!
//! This type is the data structure only. The visit loop and the rule for which
//! users to add after a fold belong to the pass that uses it.
//!
//! # Example
//! Three instructions added, then moved to the stack as a driver does:
//! ```text
//! add %a, add %b, add %c
//! pop_deferred -> %c  push %c
//! pop_deferred -> %b  push %b
//! pop_deferred -> %a  push %a
//! remove_one   -> %a, %b, %c      (the order added)
//! ```
//!
//! From unit test `deferred_moved_to_the_stack_come_out_in_the_order_added` in
//! `instworklist_tests.rs`.
//!
//! # Runs
//! Not a pass: a data structure. Nothing uses it yet; `instsimplify`'s driver
//! will.
//!
//! # See also
//! LLVM:
//! [`InstructionWorklist.h`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/InstructionWorklist.h).

use llrm_mir::dense::IdMap;
use llrm_mir::module::InstId;

/// One list: slots in push order, emptied by `remove`, and where each live
/// instruction is.
#[derive(Default)]
struct Slots {
    slots: Vec<Option<InstId>>,
    at: IdMap<InstId, usize>,
}

impl Slots {
    fn insert(
        &mut self,
        id: InstId,
    ) {
        if !self.at.contains_key(&id) {
            self.at.insert(id, self.slots.len());
            self.slots.push(Some(id));
        }
    }

    fn remove(
        &mut self,
        id: InstId,
    ) {
        if let Some(slot) = self.at.remove(&id) {
            self.slots[slot] = None;
        }
    }

    fn pop(&mut self) -> Option<InstId> {
        while let Some(slot) = self.slots.pop() {
            if let Some(id) = slot {
                self.at.remove(&id);
                return Some(id);
            }
        }
        None
    }
}

#[derive(Default)]
pub struct InstWorklist {
    stack: Slots,
    deferred: Slots,
}

impl InstWorklist {
    pub fn is_empty(&self) -> bool {
        self.stack.at.is_empty() && self.deferred.at.is_empty()
    }

    /// Visit `id` in the order added, after the combine in progress.
    pub fn add(
        &mut self,
        id: InstId,
    ) {
        self.deferred.insert(id);
    }

    /// Visit `id` before everything pushed earlier.
    pub fn push(
        &mut self,
        id: InstId,
    ) {
        self.stack.insert(id);
    }

    /// The last instruction added and not removed.
    pub fn pop_deferred(&mut self) -> Option<InstId> {
        self.deferred.pop()
    }

    /// The last instruction pushed and not removed.
    pub fn remove_one(&mut self) -> Option<InstId> {
        self.stack.pop()
    }

    /// `id` is on neither list afterwards.
    pub fn remove(
        &mut self,
        id: InstId,
    ) {
        self.stack.remove(id);
        self.deferred.remove(id);
    }

    pub fn reserve(
        &mut self,
        count: usize,
    ) {
        self.stack.slots.reserve(count + 16);
    }

    /// The end of a run: both lists must be empty; frees the map.
    pub fn zap(&mut self) {
        assert!(self.stack.at.is_empty(), "worklist empty, but map not");
        assert!(self.deferred.at.is_empty(), "deferred instructions left over");
        self.stack = Slots::default();
        self.deferred = Slots::default();
    }
}
