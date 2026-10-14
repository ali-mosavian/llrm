> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/versionstride.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/versionstride.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/versionstride.rs sha256:3e459c1c58c66f8fa40c17594ed4960aae5430d8651e12c993034350c9201a7c -->

# versionstride

A loop version's first reason: an address that steps by an unknown stride is
contiguous where the stride is 1.

## How it works
`Stride::found` takes the recurrences of an innermost loop
(`induction::recurrences`): a value that indexes an address and whose step
has a term of one invariant unknown (`a[i * s]`) names that unknown, at
most `MAX_STRIDES` of them. Its test is `s == 1` for each, and the copy's
code reads the constant 1 for each.

## Example
`a[i * s] = 0` in a loop: the copy for `s == 1` stores `a[i * 1]`, which
the pipeline makes the fill `a[i] = 0`. See [`crate::loopversion`](loopversion.md).

## Provides
`Stride`, an [`crate::loopversion::Assumption`](loopversion.md).

## See also
gcc `gimple-loop-versioning.cc` (`loop_versioning::analyze_arbitrary_term`,
`-fversion-loops-for-strides`).
