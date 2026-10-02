//! Selective whole-module MIR inlining.
//!
//! Inlining is a CFG operation, not a call peephole: split the caller at the
//! call, clone the callee's blocks, bind formal parameter loads to the actual
//! SSA values, and join every return back to the continuation.  The ordinary
//! body pipeline then simplifies the result.
//!
//! The initial policy covered leaf procedures called once.  It also admits a
//! straight-line private leaf at every direct call site when the target-priced
//! call work exceeds the semantic work duplicated by cloning.  In both cases
//! the ordinary body pipeline simplifies the result; MIR chooses from semantic
//! costs and never sees opcodes or registers.
//!
//! Direct port of `qbopt/optimize/inline.py`.

/// Python's `getattr(arg, "width", 0)`: a cell or opaque has no width.
macro_rules! width_of {
    ($arg:expr) => {
        match $arg {
            Arg::Held(one) => one.width,
            Arg::Const(one) => one.width,
            Arg::Symbol(one) => one.width,
            Arg::FrameAddress(one) => one.width,
            Arg::FrameSelector(one) => one.width,
            Arg::Cell(_) | Arg::Opaque(_) => 0,
        }
    };
}
