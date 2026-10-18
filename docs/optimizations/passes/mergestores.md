> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/mergestores.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/mergestores.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/mergestores.rs sha256:3d6959ca420a2c3d4bd75bba5bc6481646eec300b10e214fdb18a209500b610e -->

# mergestores

Adjacent copies through registers become wider ones: LLVM's
`DAGCombiner::mergeConsecutiveStores` for stores of loaded values
(`tryStoreMergeOfLoads`).

## How it works
In straight-line code, a store of a value another instruction loaded, the
only use of that load, is a copy of a cell. Copies of adjacent cells to
adjacent cells, `a[0] = b[0]; a[1] = b[1]`, are one wider load and one wider
store when the bytes they make are a legal integer (`getConsecutiveStores`
finds the run, the widest legal prefix is taken, the rest is tried again):

```text
  %x = load i16, ptr %b            %w = load i32, ptr %b
  store i16 %x, ptr %a      =>     store i32 %w, ptr %a
  %y = load i16, ptr %b2
  store i16 %y, ptr %a2
```

Between the first and the last of the copies of a span nothing else may
touch memory (a call, a store of anything else, a load whose value goes
elsewhere ends the span), and no object may be read in the span that is also
written in it, so the wider load and store can stand anywhere among the
narrow ones. The wider load goes before the load of the lowest address and
the wider store before the last of the narrow stores.

DEVIATE M1 (no number): the DAG finds candidates through a shared chain root
and checks cycles through operands (`getStoreMergeCandidates`,
`checkMergeStoreCandidatesForDependencies`); MIR has no chains, so a span of
straight-line code with no other memory operation, in objects that are told
apart (`MemRef::object`), is the same fact. Exit: none; MemorySSA could
widen the span.

DEVIATE M2 (no number): no vector types, no rotate of a reversed pair, no
truncating store; the target has none of them for integers here
(`TLI.isTypeLegal` is `largest_legal_integer`), and a load or store of
any alignment is fast on x86.

## Example
A struct of two words copied field by field in 16-bit code is a dword load
and a dword store.

## Runs
In `loopidioms::Merge`, after the scalar passes have settled.

## Used by
`loopidioms::Merge`.
