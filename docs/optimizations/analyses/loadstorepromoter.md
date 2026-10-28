> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/loadstorepromoter.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/loadstorepromoter.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/loadstorepromoter.rs sha256:2b4483d311d6d640de56bc710e2b3b03afba9e530b74c8096cc068d5f0f29fdf -->

# loadstorepromoter

`LoadAndStorePromoter`: the loads and stores of one memory slot, rewritten
so each load reads the value that reaches it.

## How it works
Within a block the order of the accesses says what a load reads: the last
store before it, or the value that enters the block when there is none.
Across blocks `SsaUpdater` places the phis. The loads that read a stored
value are replaced by it; the client decides which of the accesses go
(`Promotion::should_delete`) and what an alloca stands for as a store
(`Promotion::value_for_alloca`).

## Example
A slot stored in `b1` and `b2` and loaded in `b3`:
```mir
b1: store float 1.0, ptr %p      br label %b3
b2: store float 2.0, ptr %p      br label %b3
b3: %v = load float, ptr %p
```
`run` replaces `%v` by `phi [ 1.0, %b1 ], [ 2.0, %b2 ]` and deletes the
load when the client says so; the stores stay when it does not.

## Deviations
- L1 No debug records: `updateDebugInfo` and `replaceLoadWithValue` are not
  hooks (the base class does nothing in either), and
  `doExtraRewritesBeforeFinalDeletion` and `instructionDeleted` are not
  either: no client here needs them.

## See also
[`LoadAndStorePromoter::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SSAUpdater.cpp#L360).

## Used by
Named by `sroa`.
