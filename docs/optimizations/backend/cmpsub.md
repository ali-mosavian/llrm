> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/cmpsub.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/cmpsub.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/cmpsub.rs sha256:258639a39e364a669ccd1ff0a88dfb86f53afa39913b11a5a56eaa507a8b4c08 -->

# cmpsub

A compare of a value with a constant, where the same value minus (nearly)
that constant is made just before it: the subtract's flags are the
compare's. Translated from `X86TargetLowering::EmitCmp`
([X86ISelLowering.cpp#L24179](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L24179)):
"Else use SUB instead of CMP to enable CSE between SUB and CMP", after
which the DAG holds one `X86ISD::SUB` for the difference and the branch.

## How it works
In a block that ends `cmp x, K; jcc`, a `d := add x, -J` or `d := sub x, J`
before the compare, with `x` the same value and `J` equal to `K` (any
condition) or to `K + 1` (`ja` is then `jae`, `jbe` `jb`, `jg` `jge`, `jle`
`jl`: the pin canonicalises the setcc to the second form), is made
`d := sub x, J` and the compare goes. Nothing between the two may write the
flags. The subtract's own flags are not read by anything else, which is why
the pin could not sink it past the branch (x_adler: `x >= 65521 ? x - 65521
: x`, `sub; jae` against `cmp; jbe` around an `add`).

## Runs
A machine phase ahead of the first SSA phase (`flow::machine_with`), at
every -O level but -O0.
