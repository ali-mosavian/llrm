# Signed high product

`SMULHI(a, b)` computes the upper half of the signed, double-width product.
Both operands and the result have the same width (currently two or four
bytes). It is distinct from the truncated low product `MUL`.

MIR constant evaluation interprets each operand as signed at that width.
Lowering materializes constant operands, then emits a two-result signed
multiply. The unused low result remains an explicit definition: allocation
must account for its clobber. Register constraints belong to the existing
target description, not the MIR operation. Inserting the operation across
a live condition is refused.

This is the prerequisite for signed reciprocal division, not its integration.
LNGMXX still uses `mov ebx,7 / mov eax,ecx / cdq / idiv ebx`.
Before and after this addition its emitted objects are byte-identical:
862 bytes (PDS), 836 (QB), and 1035 (VBDOS).

Next: construct the signed magic multiplier and correction in MIR, then
measure the complete quotient/remainder expression. Replacing IDIV with
two multiplies can lose; the remainder reconstruction and surrounding
arithmetic must be considered too. The LLVM-informed reference in
`targets.md` reduces `q + r` to `n - 6*q` for division by seven.
