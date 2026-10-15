> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/num.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/num.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/num.rs sha256:2eb287ed72fcb59e514656d2ac67b26be3ad1466fa0897afa1d67b6c128639fe -->

# num

An integer that is a machine word until it must not be.

## How it works
`Num` holds an `i128` and falls back to `BigInt` only where a checked
operation overflows, as LLVM's `APInt` keeps a word inline and allocates
only past 64 bits. It is normalised: a value that fits `i128` is always
`Small`, so equality and hashing are those of the number. Every operation
gives what `BigInt` gives, including the floor of `>>` and the two's
complement of `&` on negatives.

## Provides
`Num`, for [`crate::ranges`](ranges.md) and [`crate::induction`](induction.md).

## See also
LLVM `llvm/include/llvm/ADT/APInt.h`, `APInt::U` (inline word or heap
words).
