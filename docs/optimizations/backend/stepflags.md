> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/stepflags.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/stepflags.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/stepflags.rs sha256:7981d907375af0f2aded9133dfb0da7dbd1531acfa30792590f7785f44d2dfab -->

# stepflags

Removes a compare of a value with a constant when a step of the same value
already sets those flags.

## How it works
`j1 := sub j, 1` and `cmp j, 1; jl` set the same flags. The compare is
dropped and the step moves to its place, last before the branch. Left in,
the compare reads `j` after the step defined `j1`, so the two cannot share a
register and the counter is stepped through a `lea` and copied back (X86's
`optimizeCompareInstr`, run before allocation).

```text
 before                   after
 j1 := sub j, 1           ...
 ...                      j1 := sub j, 1
 cmp j, 1                 jle
 jle
```

The compare must be `cmp v, imm` followed (past anchors) by a conditional
branch. The step is searched backwards for `sub v, c` with the same
constant, or `add v, -c`. An add leaves the carry opposite to a subtract's
borrow, so it qualifies only for the signed conditions, and not when the
constant is the most negative value, whose negation overflows. The step's
result must not be read between the step and the compare, since the step
moves down past those instructions. Widths and the absence of relocations
must agree.

## Runs
Once per block in instruction selection, after `comparefold` and before
`rmw`, in `isel/combined.rs`, on SSA values before allocation.
