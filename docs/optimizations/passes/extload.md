> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/extload.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/extload.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/extload.rs sha256:1d57614dba9b60d7af180c1e069e78aaf0e64639693ca581561d65fd2fb4ee18 -->

# extload

One extending load for a narrow load and its extension: LLVM's
DAGCombiner [`ExtendUsesToFormExtLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15365)
and [`ExtendSetCCUses`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15421),
as the rewrite of MIR before isel.

## How it works
For a load whose value `E = sext/zext L` is made in the same block, the
compares of `L` with a constant in that block compare `E` instead, with the
constant extended the same way; every other reader of `L` reads
`trunc E`. `L` is then read only by `E`, and isel selects the pair as one
`movsx`/`movzx` from memory: the compare is on the wide register, not on a
byte register that `E` must extend again.

A `zext` is not made where a signed compare reads `L` (its sign bits would
be lost), and a load whose value and extension are both read in other
blocks is left unless a compare is widened (`BothLiveOut`). `E` is moved to
just after `L`, so that it precedes the compares it now feeds.

| # | DEVIATE |
|---|---|
| D1 | Done on MIR before isel, not in the DAG: isel selects one instruction at a time (`isel/combined.rs`) and has no combine stage. Owner mir-undef; revisit when isel gains one. |
| D2 | `isTruncateFree` is taken as true: a truncation to a narrower integer reads the low part of the register on every x86 mode, so the compare of an unextendable use is not a reason to stay. |

## Example
Before:

```mir
b0:
  %c = load i8, ptr %p
  %lt = icmp sgt i8 %c, 0
  %w = sext i8 %c to i16
  br i1 %lt, label %b1, label %b2
```

After:

```mir
b0:
  %c = load i8, ptr %p
  %w = sext i8 %c to i16
  %lt = icmp sgt i16 %w, 0
  br i1 %lt, label %b1, label %b2
```

From unit test `test_a_compare_of_a_loaded_byte_compares_its_extension` in
`extload_tests.rs`.

## Runs
Once per body, after `addresssink`, where the other passes that make the
forms isel sees run (the pin has none: its DAGCombiner does it). With
`wanted("extload")`.

## Needs
Nothing.

## Keeps
Blocks, dominators and loops.

## See also
Tests: `extload_tests.rs`. Levels: `docs/reference/levels.md`.
