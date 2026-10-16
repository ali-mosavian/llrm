//! Dead store elimination: LLVM's DSE. A store is removed when nothing can read
//! the bytes it wrote.
//!
//! # How it works
//! A store is dead when every path overwrites its cell before reading it, or
//! when nothing after it can read the cell. [`llrm_analysis::avail`]'s
//! `dead_stores_escaping` finds them over the function's memory accesses
//! (`memoryssa::Accesses`): a backward dataflow over cells of what is stored
//! again before any read. The cells no call and no exit can observe, which
//! `observers::private_of` computes from the `Pointers` and `Published`
//! analyses, are unread once the function returns, so a store to a local that
//! nothing reads afterwards is dead. An access through a pointer that no fact
//! follows spares only a frame cell whose object had not escaped by then
//! (`Pointers::escaped_before`).
//!
//! ```text
//!  store i16 1, ptr %p          (overwritten before any read)
//!  store i16 2, ptr %p    ==>   store i16 2, ptr %p
//!  ret                          ret
//! ```
//!
//! A call's reach is its `Summaries` footprint (reads, writes, `initializes`
//! bytes) where the pass manager requires them; a bare manager takes every
//! callee as unknown. A call that fills a buffer kills the stores before it
//! that it overwrites; a `llvm.memset` whose bytes later stores together
//! overwrite goes.
//!
//! `promote` leaves every store when it replaces loads by values; this is the
//! pass that removes those that are now read by nothing.
//!
//! # Example
//! Before:
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   %s = alloca [8 x i8]
//!   call void @llvm.memset.p0.i16(ptr %s, i8 0, i16 8, i1 false)
//!   br label %b1
//!
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
//!   %w = sext i16 %i to i32
//!   store i32 %w, ptr %s
//!   %next = add i16 %i, 1
//!   %go = icmp slt i16 %next, %x
//!   br i1 %go, label %b1, label %b2
//!
//! b2:
//!   ret i16 %x
//! }
//! ```
//! Nothing reads the slot, so the memset and the loop's store are dead; the
//! test runs `dse` then `dead` and asserts that no `alloca`, `store` or `call
//! void` is left.
//!
//! From unit test `test_a_zeroed_slot_nothing_reads_goes` in `dse_tests.rs`.
//!
//! # Runs
//! Once per pass of the function fixed point, after `trivialunswitch` and
//! `inferspace` and before `gvn`. Off with `-fno-tree-dse`; nothing runs at
//! -O0.
//!
//! # Needs
//! `Accesses`, `Pointers`, `Published` and the CFG `Shape`; reads the memory
//! operations and the CFG.
//!
//! # Keeps
//! Blocks and edges are unchanged: dominators and loops are preserved.
//!
//! # See also
//! [`crate::dead`], [`crate::promote`], [`crate::loopmotion`].

use llrm_analysis::avail;
use llrm_analysis::manager::Pointers;
use llrm_analysis::memory::Unit;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::observers;
use llrm_mir::kinds::{Kinds, Reads};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};

pub struct Dse;

impl FunctionPass for Dse {
    fn name(&self) -> &'static str {
        "dse"
    }

    /// A store is dead by what reads or overwrites its cell on every path after
    /// it: memory operations (and what addresses them) and the CFG.
    fn reads(&self) -> Reads {
        Reads { kinds: Kinds::MEMORY | Kinds::STRUCTURE, keyed: Kinds::NONE, stops: Some(Kinds::NONE) }
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        match dropped(unit, analyses) {
            Ok(true) => PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("dse: {error}"),
        }
    }
}

/// `unit`'s function without its dead stores; whether any went.
fn dropped(
    unit: &mut passes::Unit,
    analyses: &mut Analyses,
) -> Result<bool, String> {
    let (context, layout) = (&*unit.context, unit.layout);
    let accesses = Accesses::managed(context, layout, unit.function, analyses)?;
    let pointers = analyses.get::<Pointers>(context, layout, unit.function);
    let pointers = Result::as_ref(&*pointers).map_err(String::clone)?;
    let published = analyses.get::<observers::Published>(context, layout, unit.function);
    let published = Result::as_ref(&*published).map_err(String::clone)?;
    let shape = analyses.get::<llrm_analysis::cfg::Shape>(context, layout, unit.function);
    let memory = Unit::within(context, layout, unit.function, analyses.outer()).with_shape(&shape);
    let dead = avail::dead_stores_escaping(
        &memory,
        &accesses,
        Some(&observers::private_of(memory, pointers, published)),
        Some(&pointers.escaped_before),
    );
    unit.function.erase_all(&dead)?;
    Ok(!dead.is_empty())
}

#[cfg(test)]
#[path = "dse_tests.rs"]
mod tests;
