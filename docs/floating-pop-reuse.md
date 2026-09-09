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

## Exact-width pressure spills

When loading or preserving an operand would exceed eight stack entries,
allocation now evicts a non-operand to an owned 10-byte frame slot. It reloads
that value when needed and recalculates physical stack positions. Current
operands are protected from eviction; no frame means an explicit refusal.
Spill slots have a separate key namespace from integer-to-FP scratch slots,
so a prior two-byte FILD scratch cannot be reused for a ten-byte value.

Before, nine live FP values were refused. After, the ninth load can be preceded
by (slot numbers depend on the current stack):

```asm
fxch st(7)
fstp tword [spill]
fld dword [ninthInput]
; later, when the evicted value is consumed:
fld tword [spill]
```

The spill retains the extended format rather than adding a SINGLE/DOUBLE
conversion. These are waiting x87 operations at the next modeled FP operation,
not permission to cross calls or unknown instructions with a live value.
The fail-first nine-value regression simulates selected stack operations,
checks all stored values in order, never exceeds eight entries, verifies
ten-byte memory operands, and checks scratch-slot separation. Allocation,
selection and integer spiller tests: 85 pass. The 154-object audit remains
byte-for-byte unchanged; current suite costs are unchanged. Loop PHIs and
arbitrary CFG stack reconciliation still remain to be implemented.
