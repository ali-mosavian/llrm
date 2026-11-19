> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/leaffold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/leaffold.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/leaffold.rs sha256:04158f6228f930ecc55cf5ad4654f67f73f5b3f64082293593e661228a2fc721 -->

# leaffold

A constant a leaf block makes for a join, made in the block that jumps to it.

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
