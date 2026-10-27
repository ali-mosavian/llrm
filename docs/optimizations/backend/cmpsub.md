> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/cmpsub.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/cmpsub.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/cmpsub.rs sha256:2884ea87db0e32fb485deeb0afa68067c92d301783507c5c4e00ab4df8f00b61 -->

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

## Example
`unsigned h(unsigned x) { return x >= 65521u ? x - 65521u : x; }` on
`-m32 -march=i486 -O2`. The compare goes; the subtract's carry is the
branch (the pin also makes the difference once, then selects with `cmovb`):

```text
  mov ecx, eax                     mov ecx, eax
  add ecx, -65521                  sub ecx, 65521
  cmp eax, 65520          =>       jb  L_keep
  jbe L_keep
```

## Runs
A machine phase ahead of the first SSA phase (`flow::machine_with`), at
every -O level but -O0.
