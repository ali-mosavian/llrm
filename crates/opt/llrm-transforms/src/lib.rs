//! MIR to MIR passes, each adapted from the llrm-core optimization that
//! does it there, with that pass's tests. A pass asks llrm-analysis for
//! its facts and names nothing about the machine.

pub mod addresssink;
pub mod algebraic;
pub mod argpromotion;
#[cfg(test)]
mod argpromotion_tests;
pub mod availableexternally;
#[cfg(test)]
mod availableexternally_tests;
pub mod calleepop;
#[cfg(test)]
mod calleepop_tests;
pub mod canonical;
pub mod cfg;
#[cfg(test)]
mod corpus_tests;
pub mod counting;
pub mod dead;
pub mod deadargs;
#[cfg(test)]
mod deadargs_tests;
pub mod decide;
pub mod dse;
pub mod edges;
pub mod exitfold;
pub mod exitsink;
pub mod expand;
pub mod fill;
pub mod fixednarrow;
pub mod floatfold;
pub mod floatloop;
pub mod fold;
pub mod gepoffset;
pub mod globaldce;
#[cfg(test)]
mod globaldce_tests;
pub mod globalopt;
pub mod gvn;
pub mod hoist;
pub mod homes;
pub mod indvars;
pub mod inferspace;
#[cfg(test)]
mod inferspace_tests;
pub mod inline;
pub mod interprocedural;
pub mod ipacp;
pub mod jumpthread;
pub mod lcssa;
pub mod lcssamerges;
pub mod loadjoins;
pub mod loopclone;
pub mod loopexit;
pub mod loopmotion;
pub mod loopsimplify;
pub mod lsr;
pub mod narrowspace;
#[cfg(test)]
mod narrowspace_tests;
pub mod peel;
pub mod phiopt;
pub mod pipeline;
#[cfg(test)]
mod pipeline_ported_tests;
pub mod ports;
pub mod profit;
pub mod promote;
pub mod rotate;
pub mod spares;
pub mod spill;
pub mod splitcopy;
pub mod tailrec;
#[cfg(test)]
pub mod testing;
pub mod transform;
pub mod trivialunswitch;
pub mod unroll;
pub mod unswitch;
pub mod window;
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
