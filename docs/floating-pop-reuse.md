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
