//! MIR to MIR passes, each adapted from the llrm-core optimization that
//! does it there, with that pass's tests. A pass asks llrm-analysis for
//! its facts and names nothing about the machine.

pub mod algebraic;
pub mod canonical;
#[cfg(test)]
mod corpus_tests;
pub mod cfg;
pub mod counting;
pub mod dead;
pub mod decide;
pub mod edges;
pub mod floatfold;
pub mod floatloop;
pub mod fold;
pub mod gvn;
pub mod inline;
pub mod interprocedural;
pub mod lcssa;
pub mod lcssamerges;
pub mod loadjoins;
pub mod loopexit;
pub mod loopmotion;
pub mod loopclone;
pub mod loopsimplify;
pub mod peel;
pub mod profit;
pub mod promote;
pub mod strength;
#[cfg(test)]
pub mod testing;
pub mod transform;
pub mod unroll;
pub mod unswitch;
// Not ported, meaning nothing where a value is whole: `wholephis` and
// `wholestores` joined word halves of the old MIR's split values (called
// from `algebraic`).
// Not ported, meaning nothing where an address is an operand: `pointeraccess`
// split a packed far pointer's memory references into offset and selector
// words for the old register allocator; a pointer here is its type, and
// isel splits it (called from `transform`'s `SplitPointers`).
