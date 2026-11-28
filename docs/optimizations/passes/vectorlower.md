> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/vectorlower.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/vectorlower.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/vectorlower.rs sha256:104829e80f9fb879f9a26d981796259623a46488ed426e10680f34b44a5dcf68 -->

# vectorlower

Lowers the vectors one machine word holds to that integer. Translated from
gcc's `tree-vect-generic.cc` (`expand_vector_parallel`,
`expand_vector_piecewise` over a word type, `lower_vec_perm` of a splat),
the pass that makes `ix86_preferred_simd_mode`'s word-mode vectors of a
target with no SSE code.

## How it works
A `<2 x i16>` or `<4 x i8>` value is 32 bits, the word's width. Where the
target says so (`Tti::word_vectors`) each such value becomes an `i32` whose
lane `k` is bits `k*L .. (k+1)*L` (the memory order of a little-endian
target, so a vector load is the word load):

```text
  %v = load <2 x i16>, ptr %p          %v = load i32, ptr %p
  %w = xor <2 x i16> %v, %s            %w = xor i32 %v, %s
  store <2 x i16> %w, ptr %q           store i32 %w, ptr %q
  %i = insertelement poison, i16 %x, 0 %i = zext i16 %x to i32
  %s = shufflevector %i, poison, <0,0> %s = or (%i, shl %i, 16)
```

Lanes of a bitwise operation do not interact, so `and`, `or`, `xor`, `phi`,
`select` and `freeze` keep their opcode at the word's type. Loads and
stores keep their address and alignment. Lane inserts, extracts and
shuffles are shifts, masks and `or`s; a splat of one lane doubles it.

The pass is all or nothing for a function: one use of a word vector it does
not lower (an add, a compare, a call argument) leaves the function as it
is, so the selector refuses it as it did.

## Runs
In `cgpaddr`, with the other CodeGenPrepare rewrites, where the target's
`Tti::word_vectors` is true.

## See also
`loopvectorize`, which makes the vectors where the target prices them
(`Tti::vector_arithmetic_cost`: only the bitwise operations).

## Example
`%w = xor <2 x i16> %v, %s` becomes `%w = xor i32 %v, %s`, where `%v` and
`%s` are the same two lanes held in one `i32`.
