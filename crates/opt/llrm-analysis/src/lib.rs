//! Facts about the MIR, each owned by one module that the passes in
//! llrm-transforms (and the backend) ask.
//!
//! # How it works
//! An analysis here is a function from MIR to a fact. One module owns each fact
//! and its consumers ask it. The few functions that edit the MIR say so:
//! `ssa::pruned_phis`, `ssa::SsaUpdater`, [`noreturn`]'s `after_terminal_calls`
//! and [`interprocedural`]'s rewrites. The modules fall in groups:
//!
//! - Structure: [`cfg`] (blocks, dominance, loops, `Shape`), [`graph`] (the
//!   loop algorithms over any block graph), [`occurrence`] (instruction walks),
//!   [`nearest`] (items in dominating blocks), [`ssa`] (use lists, phi pruning,
//!   `SsaUpdater`), [`liveness`] (live values and register pressure).
//! - Values: [`consts`] (known values; [`constant_cycles`] is its sparse
//!   solver), [`demanded`] (bits read), [`floatfacts`] and [`floatbounds`]
//!   (exact float values), [`ranges`] (intervals), [`difference`] and
//!   [`guards`] (what dominating compares and `assume`s prove),
//!   [`assumptions`].
//! - Loops: [`induction`] (counters, trip counts, recurrences), [`peelsize`]
//!   (is a full copy worth it), [`dependence`] (distances for unroll-and-jam).
//! - Memory: [`memory`] (objects, slices, `MemRef`, `Unit`), [`pointerfacts`],
//!   [`regions`], [`alias`] (points-to, summaries), [`effects`],
//!   [`frameescape`], [`globalsaa`], [`sliceset`] and [`cellmap`] (containers),
//!   [`memoryssa`], [`avail`] (forwardable loads, dead stores), [`observers`]
//!   (cells nothing outside can read).
//! - Calls and control: [`interprocedural`], [`noreturn`],
//!   [`parameter_ranges`], [`branchprob`] (static frequencies).
//! - [`manager`] caches the facts that are expensive or asked repeatedly.
//!   `testing` and `generated` are test support, behind the `testing` feature.
//!
//! ```text
//!    llrm-mir: Function, Context, DominatorTree, Analyses
//!         |
//!         v
//!   +---------+   +----------+   +-------------------------------+
//!   | cfg     |-->| consts   |-->| ranges  difference  guards    |
//!   | graph   |   | demanded |   | floatfacts floatbounds        |
//!   | liveness|   +----------+   +---------------+---------------+
//!   +----+----+        |                         |
//!        |             v                         v
//!        |        +----------+            +------------+
//!        +------->| induction|----------->| peelsize   |
//!        |        +----------+            | dependence |
//!        v                                +------------+
//!   +-----------------------------+   +---------------------------+
//!   | memory regions pointerfacts |-->| memoryssa  avail          |
//!   | alias effects frameescape   |   | observers  noreturn       |
//!   | globalsaa sliceset cellmap  |   | interprocedural           |
//!   +--------------+--------------+   | parameter_ranges          |
//!                  |                  +-------------+-------------+
//!                  v                                |
//!         +------------------+                      v
//!         | manager (cache)  |------> llrm-transforms passes, backend
//!         +------------------+
//! ```
//!
//! The arrows point from a fact's owner to the modules that ask it. [`consts`]
//! asks [`memoryssa`] when it solves through memory, so the layers are
//! conceptual, not a strict order.
//!
//! # Provides
//! Invalidation and caching, in [`manager`]. A pass asks
//! `analyses.get::<A>(..)` and gets one shared result per function. After each
//! pass the result is kept if the pass named it preserved; or, for an entry
//! that reads only the function, if the function is unchanged; or, for an entry
//! with `depends`, if the change log cannot have altered it; or it is updated
//! from the log (`INCREMENTAL`: `Counted`, `DominatedEdges`, `Bounded`,
//! `Accesses`); else it is dropped and recomputed on the next ask. Module
//! entries (`GlobalsAA`, `Summaries`, `Callbacks`) are dropped unless every run
//! of a pass preserved them. `LLRM_CHECK_REPLAY` asserts that kept and updated
//! results equal a fresh solve. Free-standing analyses (not manager entries)
//! are recomputed per call; a `Unit` (see [`memory`]) carries the cached ones
//! so repeated queries share them.
//!
//! # See also
//! The manager contract is `crates/ir/llrm-mir/src/passes.rs`. LLVM's
//! `AnalysisManager` and `PreservedAnalyses` are its counterpart.

pub mod alias;
pub mod assumptions;
pub mod avail;
pub mod branchprob;
pub mod cellmap;
pub mod cfg;
pub mod constant_cycles;
pub mod consts;
#[cfg(test)]
mod corpus_tests;
pub mod demanded;
pub mod dependence;
pub mod difference;
pub mod effects;
pub mod floatbounds;
pub mod floatfacts;
pub mod frameescape;
#[cfg(any(test, feature = "testing"))]
pub mod generated;
pub mod globalsaa;
pub mod graph;
pub mod guards;
pub mod induction;
pub mod interprocedural;
pub mod liveness;
pub mod manager;
pub mod memory;
pub mod memoryssa;
pub mod nearest;
pub mod noreturn;
pub mod observers;
pub mod occurrence;
pub mod parameter_ranges;
pub mod peelsize;
pub mod pointerfacts;
pub mod ranges;
pub mod regions;
pub mod sliceset;
pub mod ssa;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
