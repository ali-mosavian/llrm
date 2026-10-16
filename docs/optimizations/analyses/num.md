> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/num.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/num.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/num.rs sha256:61c8994b1d9aae47d073e32fd9d7702dbbfaaa19914bd9e874e53f45bd97897f -->

# num

An integer that is a machine word until it must not be.

## How it works
`Num` holds an `i128` and falls back to `BigInt` only where a checked
operation overflows, as LLVM's `APInt` keeps a word inline and allocates
only past 64 bits. It is normalised: a value that fits `i128` is always
`Small`, so equality and hashing are those of the number. Every operation
gives what `BigInt` gives, including the floor of `>>` and the two's
complement of `&` on negatives.

## Example
A bound that crosses the word and comes back:

```text
  (1 << 200) - (1 << 200) + 5   Big, then Small(0), then Small(5)
  i128::MAX + 1 - 1             Big(2^127), then Small(i128::MAX)
```

From unit test `a_number_is_the_same_number_by_either_path` in this file.

## Provides
`Num`, for [`crate::ranges`](ranges.md) and [`crate::induction`](induction.md).

## See also
LLVM `llvm/include/llvm/ADT/APInt.h`, `APInt::U` (inline word or heap
words).
