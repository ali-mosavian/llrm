# Preserving popping arithmetic operands

`backend/floatalloc.py` can now allocate a popping arithmetic operation whose
left operand, right operand, or both remain live. It duplicates only live
operands and positions the right operand at the top before the original
operation. It also separates identical operands into two physical slots:
computing into ST(0) and then popping it would otherwise lose the result.

Before, sharing a value across FSUBP and a later store was refused. With
initial stack `[right, left]` and both values needed later, after:

```asm
fld st(1)          ; working left copy
fld st(1)          ; working right copy
fsubp st(1),st(0)  ; left-right, originals remain below the result
```

This does not merge arithmetic or change its order, storage conversions or
rounding. Inserted instructions own no original bytes and use the existing
emulator-aware encoding path. Insufficient stack capacity still requires
spilling; this change does not pretend to implement spills or cross-block
floating allocation.

Five fail-first cases cover each live-operand combination and identical
operands with/without a later use. The tests select every instruction and
simulate the allocated register stack, asserting subtraction and subsequent
stored values, not merely instruction counts. Focused allocation/selection
checks: 57 pass. A 154-object audit including the primary corpus and floating
regressions changes no emitted object or emission outcome. Therefore current
program metrics and before/after production assembly are unchanged; this
removes an allocation blocker for future valid FP value reuse.

## Straight-line block edges

The allocator now retains its stack across adjacent blocks when the predecessor
has exactly that successor, the successor has exactly that predecessor, and
the successor is not the function entry. Remaining-use counts cover the whole
linear region, so a use in the next block causes the producer to be preserved
before an earlier destructive operation. No boundary reload or store is added.

Before, a shared sum consumed by multiplication in one block and division in
its uniquely connected successor was unavailable at the division. After:

```asm
; first block
fld dword [input]
fld st(0)           ; preserve the shared value for the successor
fmul dword [factor]
fstp dword [product]
; unique successor: the original value is still ST(0)
fdiv dword [factor]
fstp dword [quotient]
```

The fail-first case verifies the selected sequence; fork, join and entry-edge
variants remain refused. This does not implement floating PHIs, loop-carried
stack assignments, arbitrary block ordering or spills. Calls and unmodelled
operations remain barriers. The focused allocation/selection set has 61 passes.
