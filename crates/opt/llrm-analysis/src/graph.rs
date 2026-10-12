//! Block-graph algorithms (loops, depth, reducibility, dominators) over any IR
//! whose blocks implement `loops::Node`.
//!
//! # How it works
//! The module holds one submodule, [`loops`]. A `Node` is a block id and its
//! successor ids; nothing here knows an IR. The rich MIR adapts a `Function`
//! through [`crate::cfg`], which turns it into `Block`s in layout order.
//!
//! ```text
//!  Function --cfg::graph--> [Block{at,succ}] --> graph::loops
//!                                                  (Loop, depth, ...)
//! ```
//!
//! The backend's `LirBlock` has its own copy of these walks in
//! `crates/backend/llrm-core/src/analysis/loops.rs`; both build on
//! `llrm_support::graph`.
//!
//! # Provides
//! The algorithms of [`loops`]. Consumers: [`crate::cfg`] (`Loop`, `Node`,
//! `irreducible_under`) and the loop passes of `llrm-transforms`, which read
//! `Loop` out of [`crate::cfg::Shape`].

pub mod loops;
