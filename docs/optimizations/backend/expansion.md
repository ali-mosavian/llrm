> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/expansion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/expansion.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/expansion.rs sha256:1259c4ba7cfec0e48d125f24a7f3f99df3615ff72f6e6d056eb763aa168f26a2 -->

# expansion

What the MIR expansions of an intrinsic share: finding its calls, and the
instructions made before one.

## How it works
`expand_calls` walks every function of a module and hands each call of an
intrinsic the caller picks to a rewrite; an `Emitter` places the
instructions the rewrite makes right before the call, in one integer type.
The bit counts, `bswap` and the three-way compares are each a rewrite.

## Example
A `call i32 @llvm.bswap.i32(i32 %x)` is replaced, in front of the call, by
the shifts and masks that move each byte, and the call's uses read the
result.

## Runs
Under [`crate::backend::bitcounts`](bitcounts.md), [`crate::backend::bytes`](bytes.md) and
[`crate::backend::compares`](compares.md), from `driver::optimized`.

## See also
LLVM's `TargetLowering::expand*` family, which these follow
([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp)).
