> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/expansion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/expansion.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/expansion.rs sha256:fd845ebaea5376bc1dafb7a3f5c85e535acef3cd790069c4b0ef02a94d74c23c -->

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
Under codegen's `bitcounts`, `bytes` and `compares`, from
`driver::optimized`.

## See also
LLVM's `TargetLowering::expand*` family, which these follow
([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp)).
