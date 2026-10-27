> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/reassociate_rank.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/reassociate_rank.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/reassociate_rank.rs sha256:d5e4994a7de5e33ece72f672af50c8d8769770689d1ba98a52890b8b6033e55e -->

# reassociate_rank

Reassociate's ranks: how late in the function each value is computed.

Translation of `ReassociatePass::BuildRankMap` and `getRank`
([Reassociate.cpp:209](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L209),
[:233](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L233))
at d1106deb71cc.

MIRROR: constants and globals are rank 0, arguments 3, 4, ..., each
reachable block (reverse post-order) takes `n << 16` and its instructions
that cannot move take that plus a count, and any other instruction is one
more than the highest rank among its operands, a `not`, `neg` or `fneg` not
counting. The walk of operands is an explicit stack, as upstream's, and
stops at the rank of the instruction's own block.

## How it works
`Ranks::build` numbers arguments and pinned instructions once.
`Ranks::of` computes the rest on demand and memoizes it, so a value's rank
is worked out once per function: O(n) over the function.

## Used by
`crate::reassociate`.

## Example
`%a = add i32 %x, 1` with `%x` the first argument (rank 3) is rank 4.
