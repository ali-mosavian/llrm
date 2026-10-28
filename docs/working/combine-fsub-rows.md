# Rows of `visitFSub` (X - C is X + (-C))

m16 QCport growers of the pin-faithful `fsub x, C` -> `fadd x, -C` (#5642).

| row | scope | bytes | cause |
|---|---|--:|---|
| FS-1 | QCport 65, -Os | +140 (20 modules) | new `-C` entries in the float constant pool; the pin's pool holds them too |
| FS-2 | QCport 65, -O2 | +290 (20 modules) | the same pool entries; one `fsubp st(1), st(0)` became `fchs; faddp` |

The pin's DAG does not refold it (`getNegatedExpression` of a `ConstantFP` is `Neutral`; `visitFADD` takes only a `Cheaper` negation).

Tuning candidate, priced later as a DEVIATE: -Os x87 folds `fadd x, -C` back to `fsub [C]` when `-C` is single-use.
