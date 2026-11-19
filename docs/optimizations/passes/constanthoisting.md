> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/constanthoisting.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/constanthoisting.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/constanthoisting.rs sha256:38999ea53b2d7063bae18e58d344083ca0e12578cbc9ee2a2f665d2924646be2 -->

# constanthoisting

An integer constant too costly to be an immediate is made once, where
every use can read it, as LLVM's `ConstantHoisting`.

## How it works
A translation of LLVM
[`ConstantHoisting.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp)
at d1106deb71cc. The integer constants the target prices above one
instruction (`getIntImmCostInst`, or `getIntImmCostIntrin` for an
intrinsic) are collected with their uses, a constant read through a cast
counting as read by the cast's user. They are sorted by type and value and
cut into runs where each is an `add` immediate (and an addressing-mode
offset, for a memory access) from the run's smallest. The run's base is the
constant of the greatest cumulative cost (at `optsize`, the one that leaves
most uses in range of a cheap immediate). A base with more than one use is
made as `bitcast iN C to iN` in the blocks of least total frequency that
dominate the uses, and each use is rebased: it reads the base, or
`add base, offset` made before it.

## Example
```text
 before                          after
 b0: store i64 K, ptr %p         b0: %const = bitcast i64 K to i64
 b1: store i64 K, ptr %q             store i64 %const, ptr %p
                                 b1: store i64 %const, ptr %q
```

DEVIATE 1 (exit 2026-12-31, [`consthoist-gep`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L84)):
constant `gep` expressions are not hoisted. The pin's option is off by
default and a MIR operand is never a constant expression.

DEVIATE 2 (exit 2026-12-31, [`consthoist-with-block-frequency`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L78)):
block frequencies are always used; the option is on by default, so the
dominator-only insertion point search is not translated.

DEVIATE 3 (exit 2026-12-31, [`optsize`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L937)):
`shouldOptimizeForSize` reads the function's `optsize` or `minsize`; there
is no profile summary.

## Parity slot
None yet: the pin runs it in `addIRPasses`
([TargetPassConfig.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetPassConfig.cpp#L895)),
before CodeGenPrepare, which the pipeline does not wire.

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## Used by
Named by `parity`, `pipeline`.
