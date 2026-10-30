> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/stepflags.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/stepflags.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/stepflags.rs sha256:ad7ec0bd89de3545401656c8afadbce2986532eaf6b72d677f84ab5f7e2450dc -->

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

## Example
The names of the instructions of a block before and after, for a step `sub
j, 1` (or `add j, -1`) and the compare `cmp j, 1`.

Schematic, from the fixture of test
`test_the_step_of_a_compared_value_takes_the_compares_place` (not printer
output):
```text
before: sub, cmp, jle          after: sub, jle
before: add, cmp, jl           after: add, jl
before: add, cmp, jb           after: add, cmp, jb     (carry differs)
before: sub, mov, cmp, jle     after: sub, mov, cmp, jle   (result read between)
```

The compare is gone where the step's own flags answer the branch.

From unit tests `test_the_step_of_a_compared_value_takes_the_compares_place`
and `test_a_step_is_not_the_compare_where_the_carry_or_a_reader_differs` in
`stepflags.rs`.

## Runs
Once per block in instruction selection, after `comparefold` and before
`rmw`, in `isel/combined.rs`, on SSA values before allocation.
