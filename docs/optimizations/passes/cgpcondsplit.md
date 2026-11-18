> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cgpcondsplit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cgpcondsplit.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cgpcondsplit.rs sha256:82dfbda21f558732b631b8cc761ac894da2f01172d7a32d044e1c307046f23ad -->

# cgpcondsplit

A branch on `a || b` (or `a && b`) whose `b` needs dear work only it reads,
made as two branches with that work after the first.

Pass: `CodeGenPrepare::splitBranchCondition`, with the `Sink.cpp` rule for
what moves: a single-use instruction, and a load nothing between it and the
split can write over.

```text
 %b = <dear cone>                     br %a, %t, %side
 %c = select %a, true, %b       side: %b = <dear cone>
 br %c, %t, %f                        br %b, %t, %f
```

Applied where the cone costs more than twice a branch in the target's
prices: on x87 `fld`, `fadd` and `fstp` are 18 clocks against 3 for the
branch, paid on every trip though `a` decides one in 25.

DEVIATE CB1 (owner x87-b; exit 2026-11-24: the pin's DAG keeps the cone in
the first block, and `splitBranchCondition` takes only an `or`/`and`
instruction): `SimplifyCFG`'s `FoldBranchToCommonDest` made the select
from two branches, and the cone, made before the first branch in the
source, stayed in front of both. gcc's `tree-ssa-sink` moves it behind the
first test (sfm16_dbg -O1: 400 `fadd` against 384, +291 clocks).

## Used by
Named by `pipeline`.
