> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/ssaupdater.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/ssaupdater.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/ssaupdater.rs sha256:d938f734823332495ca05558806d165ca83d80a869e3f8a64416a766ddb9af15 -->

# ssaupdater

`SSAUpdater`: a value defined in several blocks, rewritten so each use reads
the definition that reaches it, with a phi placed where definitions meet.

## How it works
The client says where the value is defined (`add_available_value`), then
rewrites each use (`rewrite_use`). A use in a block with no definition asks
for the value at the end of its block: `Impl::get_value` walks back through
the predecessors to the blocks that define the value, numbers the blocks
it reached in postorder, finds their dominators within that subgraph
(Cooper, Harvey and Kennedy), places a phi in the iterated dominance
frontier of the definitions, reuses a phi the block already has that
matches, and fills in the new phis' inputs. Every block it reached
remembers the value that reaches its end, so the next use in it is a lookup.

A use in a block that already holds a definition (or a remembered value)
asks for the value in the middle of the block, which merges what the
predecessors give; a phi it makes that simplifies is not made.

This is the translation LLVM's `LoopRotate` uses. [`crate::ssa::SsaUpdater`](ssa.md)
is the simpler updater the other passes were written against; it places a
phi where a loop's back edge meets the value and removes it by walking, and
may leave a phi LLVM's does not, or the reverse.

## Example
A value `%v` defined in `b1` and `b2`, used in `b4` below them:
```mir
b0: br i1 %c, label %b1, label %b2
b1: %v1 = ...    br label %b4
b2: %v2 = ...    br label %b4
b4: use %v
```
`add_available_value(b1, %v1)`, `add_available_value(b2, %v2)`,
`rewrite_use` of the use in `b4` places `%v = phi [ %v1, %b1 ], [ %v2, %b2
]` in `b4` and reads it.

## Deviations
- S1 No debug records: `UpdateDebugValues` and `insertDebugValuesForPHIs`
  are not translated (R4 of `looprotate`).
- S2 The `SSAUpdaterPhiSearchLimit` is the default, 80, with no option.

## See also
[`SSAUpdater`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SSAUpdater.cpp),
[`SSAUpdaterImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/SSAUpdaterImpl.h).

## Used by
Named by `jumpthreading`, `lcssa_form`, `loads`, `loadstorepromoter`, `looprotate`, `sroa`, `ssaupdater_walk_allocates_per_walk_not_per_block`, `ssaupdaterbulk`.
