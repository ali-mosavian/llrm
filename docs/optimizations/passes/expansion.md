> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/expansion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/expansion.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/expansion.rs sha256:00ef3d3f5f157b2beb256a34fdfc14e90be9cc94aeda801a3512829dee100d61 -->

# expansion

Finding the calls of an intrinsic in a module and handing each to a rewrite:
what every MIR expansion before selection shares.

## How it works
`expand_calls` walks every function of a module and gives each call of an
intrinsic the caller picks to a rewrite, with what the pick returned.

## Example
`memsetpattern` picks `memset.pattern` and rewrites each call as a loop.

## Runs
From the expansions in this crate and in `llrm-codegen`, before selection.

## See also
LLVM's `PreISelIntrinsicLowering`
([PreISelIntrinsicLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PreISelIntrinsicLowering.cpp)).
