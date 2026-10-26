> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/valuefacts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/valuefacts.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/valuefacts.rs sha256:0fda62b4f7a1055de7931188cdcdf2eca1368d1de5575fbcfc3536f45689de9f -->

# valuefacts

What the middle end proves of an integer value, for the instruction
selector to read: the bounds of an index or a dividend.

## How it works
`ValueFacts` answers `get(at, value)` with the value as a signed
integer interval where it is read at `at`, or none where nothing narrower
than the type is proven. The interval is the intersection of what
ScalarEvolution proves of the value (`getUnsignedRange`, `getSignedRange`:
its loop bounds), what ValueTracking proves under the branches and
assumptions that dominate `at` (`computeConstantRange` with known bits),
and what LazyValueInfo proves at `at`. The selector asks no analysis
itself: the table is made from the function once and asked many times,
each (block, value) worked out at its first ask; LazyValueInfo works on a
copy of the context, made at that ask, because it makes constants.

`exact_offsets` marks the index values every access names exactly at any
wider width, from the same table.

DEVIATE V1: the pin's `X86ISelDAGToDAG::matchAddress` asks
`SelectionDAG::computeKnownBits` of the address operand in place, on the
lowered DAG. MIR is lowered to LIR before selection and there is no
LIR-level known-bits analysis, so the facts are the IR-level range of the
same value, handed over in a table. Exit: a LIR known-bits analysis, or
the lowering attaching the ranges to the LIR value.

DEVIATE V2: the table knows more than the counted-loop solve it replaces
(the range of a `urem`, of a masked value). `isel::significant_bits` reads
it only for a range that needs the whole width; a narrower one prices a
reciprocal as an unknown dividend does, the early-out minimum, because
pricing by it chose `idiv` for `switch` (-8.6% clocks) where the emulator
counts the reciprocal cheaper. Exit: the multiply and divide pricing of
`cpu.rs` (lir-cleanup-port) agrees with the emulator.

## Example
`a[i]` over `for i in 0..8` with `a: [8 x i16]` and `inbounds`: the range
of `%i` is `[0, 7]`, so `exact_offsets` holds `%i` and the address folds
to `a[ebx+ebx]`.

## Runs
Once per function selected, in `isel::selected_with`.
