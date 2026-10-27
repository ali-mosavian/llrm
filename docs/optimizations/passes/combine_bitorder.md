> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_bitorder.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_bitorder.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_bitorder.rs sha256:0352911785fb6eceeac8f8dcd2d37de987a50cad3074508e6ba2ff2461e4cb0d -->

# combine_bitorder

A byte or bit reversal written as shifts, masks and `or`s is the `bswap` or
`bitreverse` intrinsic, and a bitwise operation on a reversed value is the
reversal of the operation: LLVM's `matchBSwapOrBitReverse` (with
`recognizeBSwapOrBitReverseIdiom` and `collectBitParts` of `Local.cpp`) and
`foldBitwiseLogicWithIntrinsics` of InstCombine.

## How it works
`collect_bit_parts` walks the expression under an `or` through `or`,
constant shifts, constant masks, `zext`, `trunc`, `bswap`, `bitreverse` and
funnel shifts by a constant, and says, for each result bit, which bit of one
provider value it is (or that it is zero). If the map is a byte reversal or
a bit reversal of the low `n` bits, the expression is that intrinsic of the
provider, truncated to `n` bits and masked where bits are zero.

## Example
```text
  (x << 24) | ((x & 0xff00) << 8) | ((x >> 8) & 0xff00) | (x >> 24)
                      ==>  call i32 @llvm.bswap.i32(i32 %x)
```

DEVIATE 1: no vectors.

DEVIATE 2: only `visitOr` asks; the calls of `bswap` and `bitreverse`
(`visitCallInst`, :2619 and :2748) and `visitAnd`'s place of
`foldBitwiseLogicWithIntrinsics` (:2956, after folds not yet translated)
are not. In `visitAnd` and `visitXor` it runs after the folds translated.

## Used by
Named by `combine_and`, `combine_or`, `combine_xor`.
