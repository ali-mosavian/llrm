> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/leaffold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/leaffold.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/leaffold.rs sha256:1f41eea6906687c6bc93eee3151607e62a7a1101f0760e76b621e69e25109dbd -->

# leaffold

A constant a leaf block makes for a join, made in the block that jumps to
it.

## How it works
A block of one constant and a jump (`v := mov 3; jmp J`) that one branch
reaches costs its jump at -Os. The branching block makes the constant itself
and the branch goes to `J`; the leaf is gone and `J`'s phi takes `v` from
the branching block. A chain of tests of a sparse switch ends in one such
leaf per case; gcc loads the result ahead of its compare, as this does.

```text
  p:  cmp x, 7                    p:  v := mov 3
      je l                            cmp x, 7
  l:  v := mov 3                      je j
      jmp j                       j:  r = phi [p: v, ...]
  j:  r = phi [l: v, ...]
```

## Deviation
DEVIATE LEAF-FOLD-OS (owner m32, exit 2026-11-20). The pin keeps the leaf:
`PHIElimination.cpp` splits an edge only for a value live out past the phis
(L841, L858, L879), and clang -Os emits `mov $c, %edi; jmp .LBB0_19` per
case. `Target::leaf_fold_at_size` turns it on (m32); only at -Os, where the
jump counts and the move on the path that does not branch does not.

DEVIATE LEAF-FOLD-TRIAL (owner m32, exit 2026-11-24): the fold changes the
registers after it, so a module it folded is assembled again without it
(`driver::trial`) and the smaller object is kept.

## Runs
Phase 1 of the machine phases, after `SharedConstants`, before `SsaSpill`.

## Example
At -Os on a target that enables it, a leaf block holding only a constant and
a jump is removed and the constant is made in the block that branched to it.

```text
p:  cmp x, 7 ; je l    l: v := mov 3 ; jmp j   j: r = phi [l: v]
  becomes
p:  v := mov 3 ; cmp x, 7 ; je j               j: r = phi [p: v]
```
