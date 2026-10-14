> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/versionalias.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/versionalias.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/versionalias.rs sha256:4714527c78283b3c2f8e73780f0c791a430f10c86457554b1a8666f34ea024ef -->

# versionalias

A loop version's second reason: the pointers a loop reads and writes may be
one object, and where the bytes they reach do not meet, they are not.

## How it works
`Disjoint::found` lists the accesses of an innermost counted loop
([`llrm_analysis::access`](../analyses/access.md)), each an affine address on a root, and the pairs
of them, one a store, through different roots that nothing proves apart
(`access::Apart::unknown`). Accesses of one root with one invariant part and
one step are a group, their bytes at the trip `i` `[low, high)` from
`root + sum of terms + step * i`.

Its test, made in the entry, is for each such pair of groups that their
bytes over the loop's trips do not meet: with `start` the lowest byte of a
group (`root + low + terms`, less `|step| * (trips - 1)` where it steps
down) and `length = |step| * (trips - 1) + high - low`, the groups `a` and
`b` are apart where `start_b - start_a >= length_a` and `start_a - start_b
>= length_b`, unsigned and modulo the pointer's width, which holds whether
or not the ranges end at the top of the space.

The copy's code is told so: each root of such a pair is replaced there by
`llrm.noalias(root)` (`llrm_mir::intrinsics::Intrinsic::Restrict`), whose
provenance the alias owner makes a restrict root of its own, so every pass
after asks it as it would of a `noalias` parameter. Every pair of roots the
copy is told apart was tested, and any other pair is the alias owner's as it
was.

```text
  for i: a[i] = *c + b[i]        if a, b, c apart: for i: a'[i] = *c' + b'[i]
                                 else:             for i: a[i]   = *c  + b[i]
```

A loop is copied only where there is something for the copy to take out of
it: a load of one place every trip, or one read twice, which a store through
a pointer that may be another's keeps in. Only pointers in one flat space
are tested (an address there is an integer): not far pointers. At most
`MAX_CHECKS` pairs of groups are, and `MAX_ACCESSES` accesses are listed.

## Example
`a[i] = *c + b[i]` over parameters: `*c` is read again every trip, as a
store through `a` may reach it. The copy that the test of `a`, `b` and `c`
sends to has `*c` read once, before the loop. See [`crate::loopversion`](loopversion.md).

## Provides
`Disjoint`, an [`crate::loopversion::Assumption`](loopversion.md).

## See also
LLVM `LoopAccessAnalysis` (`RuntimePointerChecking`, `generateChecks`) and
`LoopVersioning::versionLoop` with its `!noalias` scopes, here the restrict
root.
