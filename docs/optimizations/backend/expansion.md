> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/expansion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/expansion.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/expansion.rs sha256:7e2f455e96ab2c8d53fafb1a73aa2f8695e976ba7e65434c93d35abd752b0cb4 -->

# expansion

What the MIR expansions of an intrinsic share: finding its calls, and the
instructions made before one.

## How it works
`expand_calls` walks every function of a module and hands each call of an
intrinsic the caller picks to a rewrite; an `Emitter` places the
instructions the rewrite makes right before the call, in one integer type.
The bit counts, `bswap` and the three-way compares are each a rewrite.

## Runs
Under [`crate::backend::bitcounts`](bitcounts.md), [`crate::backend::bytes`](bytes.md) and
[`crate::backend::compares`](compares.md), from `driver::optimized`.

## See also
LLVM's `TargetLowering::expand*` family, which these follow
([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp)).
