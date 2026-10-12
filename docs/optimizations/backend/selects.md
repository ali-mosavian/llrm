> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/selects.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/selects.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/selects.rs sha256:8be332dc63282220a9ed34ed4e1124d52f6f15efbcc81371939d3a726fb97881 -->

# selects

Turns each MIR `select` into a branch around an empty block and a phi, where
the target has no conditional move.

## How it works
`select c, a, b` splits its block at the select. The instructions after it
move to a new join block, which takes over the block's way out (the phis of
its successors are renamed to name the join). A new block `taken` holds only
a jump to the join. The original block ends in a branch on `c` to `taken` or
the join, and the join starts with `phi [a, taken], [b, block]`, which
replaces the select's uses:

```text
 block:  ...                   block:  ...
         %r = select c, a, b           br c, taken, join
         rest                  taken:  br join
                               join:   %r = phi [a, taken], [b, block]
                                       rest
```

The loop is repeated until no select is left in any function, so two selects
in one block each get their own split. The result is checked by an
interpreter test that runs a function before and after.

## Runs
Once per program in `driver::optimized`, after `fpconvert` and before the
spill-model dump, only where the target's `datalayout.toml` lists `select`
under `expand` (m16 and m32: no conditional move before the Pentium Pro).
There is no flag; it runs at every -O level.

## See also
LLVM's CodeGenPrepare (`optimizeSelectInst`) makes the same change where the
target prices a branch below a conditional move.
