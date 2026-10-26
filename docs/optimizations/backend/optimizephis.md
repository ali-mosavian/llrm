> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/optimizephis.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/optimizephis.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/optimizephis.rs sha256:2e8623356fa5251b98192c23abd3ae8c7a83c5d5b1ae7d49559366b92a942957 -->

# optimizephis

A phi cycle that carries one value is that value, and one nothing reads is
gone: `OptimizePHIs` before MachineCSE. Translated from
[OptimizePHIs.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/OptimizePHIs.cpp)
(`IsSingleValuePHICycle`, `IsDeadPHICycle`, `OptimizeBB`).

## How it works
For each phi, the phis it reads are followed (at most 16, through plain
register copies too). If every other input is one value, the phi is that
value: its result is renamed and it goes. If every reader of its result is
a phi that is itself read only by phis in the cycle, the cycle goes.

LLVM's `InstCombine` folds `phi [a, self]` in IR. A machine phi of the kind
comes from DAG legalization, "e.g., when i64 values are split up for 32-bit
targets"; ours from isel splitting a far pointer's phi in an offset and a
selector that a constant step leaves as it was. Left to the lowering it is
three copies and a register of its own, and a selector that must sit in ES
is copied in and out of it each trip.

## Example
`bench/huge/huge.c` at `-m16 -O2`, `ISEL_DUMP=1`, stage `MachineSink`: the
sum loop's selector `v26` is carried round the loop and never changed.

```lir
block 0x0026
  v25 := phi 0x001d:v73 0x0026:v79
  v26 := phi 0x001d:v72 0x0026:v26
  0x0029  v77 := mov [[es:r0+0x0] v25@unplaced]
```

After the pass the load reads `v72` and the phi is gone.

DEVIATE O1 (no number): the pin's `constrainRegClass` of the single value to
the replaced register's class is not asked: a value's class here is what
its readers need, so the readers moved to it bring theirs. Exit: classes
are on LIR values.

## Runs
After `FarIndirectCalls`, before [`crate::backend::phiconst`](phiconst.md). Every level.
