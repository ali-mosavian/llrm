//! GNU C's `__builtin_*` functions: the one table.
//!
//! Open Watcom knows none of them, so a call to one was a call of an undefined
//! extern and the program did not link (`__builtin_expect(x, 0)`, in valid C,
//! compiled without a word). A builtin the table lists is lowered; one it does
//! not list is refused with its name where the call is translated, not left to
//! the linker.
//!
//! # Lowerings
//! - `Identity`: the call is its first argument; the others are evaluated for
//!   their effects. `__builtin_expect(x, c)` is `x`, translated at the call
//!   (`translate.rs`). The branch weights the pin's LowerExpectIntrinsic reads
//!   from `llvm.expect` are not carried yet: a missed hint, not a wrong answer.
//! - `Constant(n)`: the call is `n` and its arguments are not evaluated.
//!   `__builtin_constant_p(x)` is 0, the value GCC documents for an expression
//!   it cannot show constant.
//! - `Ignored`: the call is `(void)0`. `__builtin_prefetch` is a hint;
//!   `__builtin_unreachable` promises the point is not reached, so doing
//!   nothing there is a refinement.
//! - `Libc(name)`: the library routine `name`; `__builtin_abs` is `abs`, as GCC
//!   calls it where it does not expand it, and `__builtin_trap` is `abort`.
//!
//! `Constant`, `Ignored` and `Libc` are `#define`s of the unit's predefined
//! header (`predefined.rs`), so the compiler proper never sees the builtin.
//! The bit-count builtins (`popcount`, `clz`, `ctz`, `bswap*`) and `alloca`
//! have no lowering yet and are refused.

/// What a builtin is lowered to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lowering {
    /// Its first argument.
    Identity,
    /// This constant, its arguments not evaluated.
    Constant(i64),
    /// Nothing.
    Ignored,
    /// A call of this library routine.
    Libc(&'static str),
}

const BUILTINS: [(&str, Lowering); 20] = [
    ("expect", Lowering::Identity),
    ("constant_p", Lowering::Constant(0)),
    ("prefetch", Lowering::Ignored),
    ("unreachable", Lowering::Ignored),
    ("trap", Lowering::Libc("abort")),
    ("abs", Lowering::Libc("abs")),
    ("labs", Lowering::Libc("labs")),
    ("memcpy", Lowering::Libc("memcpy")),
    ("memmove", Lowering::Libc("memmove")),
    ("memset", Lowering::Libc("memset")),
    ("memcmp", Lowering::Libc("memcmp")),
    ("strlen", Lowering::Libc("strlen")),
    ("strcmp", Lowering::Libc("strcmp")),
    ("strncmp", Lowering::Libc("strncmp")),
    ("strcpy", Lowering::Libc("strcpy")),
    ("strncpy", Lowering::Libc("strncpy")),
    ("strcat", Lowering::Libc("strcat")),
    ("strchr", Lowering::Libc("strchr")),
    ("strrchr", Lowering::Libc("strrchr")),
    ("strstr", Lowering::Libc("strstr")),
];

/// The lowering of `__builtin_<name>`, where the table has one.
pub fn lowering(name: &str) -> Option<Lowering> {
    BUILTINS.iter().find(|(known, _)| *known == name).map(|(_, lowering)| *lowering)
}

/// The `#define`s of the builtins that need no translation of their own.
pub fn defines() -> Vec<String> {
    BUILTINS
        .iter()
        .filter_map(|(name, lowering)| match lowering {
            Lowering::Identity => None,
            Lowering::Constant(value) => Some(format!("#define __builtin_{name}(...) ({value})")),
            Lowering::Ignored => Some(format!("#define __builtin_{name}(...) ((void)0)")),
            Lowering::Libc(routine) => Some(format!("#define __builtin_{name} {routine}")),
        })
        .collect()
}
