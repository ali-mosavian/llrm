# Floating-point environment

For contributors adding a floating-point optimization. States what the compiler assumes of rounding, precision and exceptions, and the runtime evidence behind not assuming more for BASIC.

## What MIR assumes

MIR observes no floating-point exception and states no rounding mode or flush-to-zero setting. A fact about a float result is therefore kept only when it holds under every mode: `floatfacts` proves a result *exact in its format*, and only then does `floatfold` replace it ([constant-propagation](../../optimizations/passes/fold.md)). Nothing reassociates float arithmetic, assumes no NaNs, or changes a rounding conversion. A strict-order float recurrence keeps its iteration order (`floatloop` runs only exact ones).

This matches LLVM's model: exception handling and rounding are separate properties, and a pass that reuses a value must know both (`SimpleValue::canHandle` in LLVM's `EarlyCSE.cpp` rejects strict-exception and dynamic-rounding operations).

## Evidence from QuickBASIC 4.5

QB 4.5's runtime source (not in this repository) shows:

- `_fpreset` calls the math vector with `BX=1` to reset it, then with `BX=4, AX=1332h` to set the control word; alternate and decimal math ignore the argument.
- Runtime start-up (`B$RTRUNINI`) calls `__fpreset`; the runtime error path calls it unconditionally before `B$ONERR`.

`1332h` decodes as: invalid-operation, divide-by-zero and overflow exceptions unmasked; denormal, underflow and precision masked; extended precision; round to nearest even. That is what that source requests, not the state at every call site of PDS, VBDOS or an emulator, so it is not propagated into MIR by analogy.

## Consequences

- The runtime contradicts a blanket masked-exception assumption, and error handling can reset the floating environment and stack.
- Reuse needs evidence about the operations and intervening effects, not a global switch: exactness (above), matching precision, and memory facts that an unknown call invalidates.
- An extended arithmetic result is not the SINGLE value produced by storing and reloading it; rounding conversions stay explicit in MIR as `fptrunc` and typed stores. `fpround` rounds a float to its own type's precision (operand and result one type): QuickBASIC rounds what it stores to a SINGLE or DOUBLE variable (BC stores through memory) while its expressions run in extended precision, so the QB front end makes every stored value that is not a load or a round already a `fpround` of itself, which a promoted variable keeps; a store of an `fpround` is one `fstp`. Not a precision-control mode: that rounds after every operation, and BC's `t! = t! * .999 + .1 / i` differs from the same operations stored one by one.
- Where the target is a BASIC module the backend keeps the x87 `wait` that BC emits around integer conversions: [native-fpu-waits](native-fpu-waits.md).
