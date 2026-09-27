//! MIR to MIR passes, each adapted from the llrm-core optimization that
//! does it there, with that pass's tests. A pass asks llrm-analysis for
//! its facts and names nothing about the machine.

pub mod canonical;
pub mod edges;
pub mod lcssa;
pub mod lcssamerges;
pub mod loopclone;
pub mod loopsimplify;
pub mod profit;
#[cfg(test)]
pub mod testing;
pub mod transform;
pub mod unswitch;
// Not ported, meaning nothing where a value is whole: `wholephis` and
// `wholestores` joined word halves of the old MIR's split values (called
// from `algebraic`).
// Not ported, meaning nothing where an address is an operand: `pointeraccess`
// split a packed far pointer's memory references into offset and selector
// words for the old register allocator; a pointer here is its type, and
// isel splits it (called from `transform`'s `SplitPointers`).
