> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/divrempairs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/divrempairs.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/divrempairs.rs sha256:92db5b2c6635d3d0e293ad6bbc9854c013443a42c81c2483f0c8feefb945eb74 -->

# divrempairs

DivRemPairs: a division and the remainder of the same operands share one
division, `x % y` being `x - (x / y) * y`.

## How it works
A translation of LLVM's
[`DivRemPairs.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/DivRemPairs.cpp)
at d1106deb71cc, `optimizeDivRem` (:178) over `getWorklist` (:121): every
`sdiv`/`udiv` and `srem`/`urem` (and the expanded `x - (x / y) * y`,
`matchExpandedRem`) is keyed by signedness and operands, and each remainder
with a division of its key is a pair. A remainder the division does not
dominate in a different block, with no block dominating the other, has the
division hoisted to their common predecessor when the remainder's block
leads only to the division's; then the remainder is decomposed to the
multiply and subtract of the division, the division moved before it if it
came after, `exact` and the like dropped from the division, and an operand
that may be `undef` frozen.

## Deviations
- D1: `TTI.hasDivRemOp` is false: no target here has a combined
  divide-remainder instruction (arm64's default), so the recompose and
  hoist-both arms are not translated. Exit: a `Tti::has_div_rem_op` and
  those arms, with the first target that answers true.

## Keeps
The blocks and edges: dominators and loops are preserved.

## Example
```text
%q = sdiv i64 %x, %y
%r = srem i64 %x, %y        ; becomes  %m = mul i64 %q, %y
                            ;          %r = sub i64 %x, %m
```

## Used by
`pipeline`: `divrempairs@1667` (OptimizePM, after the late combines) and
`divrempairs@2373` (LateFPM).
