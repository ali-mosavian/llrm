//! GNU C's `__builtin_*` functions: the one table.
//!
//! Open Watcom knows none of them, so a call to one was a call of an undefined
//! extern and the program did not link (`__builtin_expect(x, 0)`, in valid C,
//! compiled without a word). A builtin the table lists is lowered where the
//! call is translated; one it does not list is refused with its name, not
//! left to the linker.
//!
//! # Lowerings
//! - `Identity`: the call is its first argument; the others are evaluated for
//!   their effects. `__builtin_expect(x, c)` is `x`. The branch weights the
//!   pin's LowerExpectIntrinsic reads from `llvm.expect` are not carried yet: a
//!   missed hint, not a wrong answer.

/// What a builtin is lowered to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lowering {
    /// Its first argument.
    Identity,
}

const BUILTINS: [(&str, Lowering); 1] = [("expect", Lowering::Identity)];

/// The lowering of `__builtin_<name>`, where the table has one.
pub fn lowering(name: &str) -> Option<Lowering> {
    BUILTINS.iter().find(|(known, _)| *known == name).map(|(_, lowering)| *lowering)
}
