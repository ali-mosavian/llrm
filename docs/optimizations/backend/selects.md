> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/selects.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/selects.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/selects.rs sha256:2983be296b85e3997fc637630c6773e62fe4069cdbb7150f133547221b7e1ce4 -->

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

DEVIATE S3 (no number): `llvm.usub.sat` is expanded here, before selection,
to `umax(a, b) - b`. The pin expands it in the DAG legalizer
(`TargetLowering::expandAddSubSat`, `TargetLowering.cpp`) to a subtraction
and a conditional move. Exit: the legalizer's expansion lands.

DEVIATE S4 (no number): `llvm.{u}{add,sub}.with.overflow` is expanded here
to the operation and `icmp ult`, for the selectors that have no UADDO
(`--isel=old`, shadow): CodeGenPrepare forms the intrinsic on every x86 width
(`shouldFormOverflowOp`), and the old selector stopped at
"@llvm.uadd.with.overflow.i16" (nib and qb tests). The pin's legalizer expands
UADDO the same way (`TargetLowering::expandUADDSUBO`). Exit: the old selector
is deleted.

DEVIATE S5 (no number): `llvm.experimental.noalias.scope.decl` is erased
here for the same selectors: no code runs it, and the DAG builder returns
at it (`SelectionDAGBuilder`, `case Intrinsic::experimental_noalias_scope_decl`).
Exit: the old selector is deleted.

DEVIATE S2 (no number): the pin keeps the select to instruction selection.
CodeGenPrepare's `optimizeSelectInst` forms no branch where
`isPredictableSelectExpensive` is false (in-order i486, `CodeGenPrepare.cpp`
`isFormingBranchFromSelectProfitable`), and `X86TargetLowering::
EmitLoweredSelect` then expands the select to a diamond on machine IR, with
both operands computed before it. Ours expands in MIR. The operand sinking
of `sinkSelectOperand`, which the pin does not do here, is deleted: it
moved nothing over 834 objects. Exit: the expansion at selection, the
registry slot `emit-lowered-select` (lir-cleanup-port, PR by 2026-11-13;
##3052).

The loop is repeated until no select is left in any function, so two selects
in one block each get their own split. The result is checked by an
interpreter test that runs a function before and after.

## Example
Before:

```mir
define i16 @f(i16 %a, i16 %b) {
entry:
  %less = icmp ult i16 %a, %b
  %least = select i1 %less, i16 %a, i16 %b
  %twice = add i16 %least, %least
  br label %done
done:
  %r = phi i16 [ %twice, %entry ]
  ret i16 %r
}
```

After:

```mir
define i16 @f(i16 %a, i16 %b) {
entry:
  %less = icmp ult i16 %a, %b
  br i1 %less, label %0, label %1
0:
  br label %1
1:
  %2 = phi i16 [ %a, %0 ], [ %b, %entry ]
  %twice = add i16 %2, %2
  br label %done
done:
  %r = phi i16 [ %twice, %1 ]
  ret i16 %r
}
```

The select became a branch to the empty block `0` and a phi in `1`; the phi
of `done` now names `1` as its predecessor.

From unit test `test_a_select_becomes_a_branch_and_a_phi` in `selects.rs`.

## Runs
Once per program in `driver::optimized`, after `fpconvert` and before the
spill-model dump, only where the target's `datalayout.toml` lists `select`
under `expand` (m16 and m32: no conditional move before the Pentium Pro).
There is no flag; it runs at every -O level.

## See also
LLVM's CodeGenPrepare (`optimizeSelectInst`) makes the same change where the
target prices a branch below a conditional move.
