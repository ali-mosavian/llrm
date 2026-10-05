//! MIR to MIR passes, each adapted from the llrm-core optimization that
//! does it there, with that pass's tests. A pass asks llrm-analysis for
//! its facts and names nothing about the machine.

pub mod algebraic;
pub mod canonical;
#[cfg(test)]
mod corpus_tests;
#[cfg(test)]
mod pipeline_ported_tests;
pub mod cfg;
pub mod counting;
pub mod dead;
pub mod dse;
pub mod decide;
pub mod edges;
pub mod exitfold;
pub mod expand;
pub mod exitsink;
pub mod fill;
pub mod floatfold;
pub mod floatloop;
pub mod argpromotion;
pub mod calleepop;
pub mod deadargs;
pub mod inferspace;
pub mod narrowspace;
#[cfg(test)]
mod calleepop_tests;
#[cfg(test)]
mod argpromotion_tests;
#[cfg(test)]
mod deadargs_tests;
#[cfg(test)]
mod inferspace_tests;
#[cfg(test)]
mod narrowspace_tests;
pub mod globalopt;
pub mod fold;
pub mod splitcopy;
pub mod availableexternally;
#[cfg(test)]
mod availableexternally_tests;
pub mod globaldce;
#[cfg(test)]
mod globaldce_tests;
pub mod fixednarrow;
pub mod addresssink;
pub mod gepoffset;
pub mod gvn;
pub mod hoist;
pub mod indvars;
pub mod inline;
pub mod interprocedural;
pub mod lcssa;
pub mod lcssamerges;
pub mod loadjoins;
pub mod loopexit;
pub mod loopmotion;
pub mod loopclone;
pub mod loopsimplify;
pub mod lsr;
pub mod window;
pub mod peel;
pub mod pipeline;
pub mod ports;
pub mod profit;
pub mod promote;
pub mod rotate;
pub mod spill;
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
// isel splits it. So `transform`'s `SplitPointers`, which ran it, is not
// ported either.
// Not ported, an analysis here: `transform`'s `PointerProvenance` wrote
// `alias::annotated`'s references into the body; alias's `Annotated`
// manager entry answers them on demand.
// Not ported, meaning nothing where a call's arguments are its operands:
// `transform`'s `Place` (`placed`, `_argument_run`, `_may_pass`, `_meets`)
// moved what stood among a call's argument pushes ahead of them.
