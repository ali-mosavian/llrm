> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_shifts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_shifts.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_shifts.rs sha256:e6828f3a06a0755c8f46fac3fe0073016f1fdfffb012987b5796467eb41b4dfb -->

# combine_shifts

InstCombine's `visitShl`, `visitLShr` and `visitAShr` and the helpers they
call, from
[`InstCombineShifts.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp)
at d1106deb71cc.

`visit_shl`, `visit_lshr` and `visit_ashr` are the C++ functions: the
same steps in the same order, each helper a function named for the C++ one
with its own permalink. A step that returns `nullptr` there returns `None`
here, one that returns an instruction returns `Some(Visit)` (a new
instruction replaces the visited one; `&I` is `Visit::Changed`).

MIRROR: the order of the steps and each condition.

DEVIATE 1: `foldVectorBinop` is not translated: MIR has no vector
operations.

DEVIATE 3 (temporary): the folds that build an intrinsic call (`bswap`) are
left out until the Builder can declare an intrinsic (`combine_build`
DEVIATE 2); each is a `// DEVIATE 3:` comment at its place.

DEVIATE 4: constants are `u128` bits with a width, so a mask built in twice
the width (`dropRedundantMaskingOfLeftShiftInput`) is not built for a type
wider than 64 bits, and a sum of shift amounts at or past twice the width
(which LLVM folds to a poison mask) is left alone; an `undef` or `poison`
constant amount is no constant sum, where LLVM replaces the undefs.

DEVIATE 5: the folds that match `scmp` and `ucmp` are translated, but MIR
has neither intrinsic yet, so no test reaches them.

## Used by
`combine::visit`, once the shift family's slot is flipped.

## How it works
`visit_*` first replaces the shift by a value it simplifies to, then tries
the transforms the three shifts share (`commonShiftTransforms`: a constant
amount pulled through the operand, two shifts the same way joined, an
amount reduced), then the transforms of its own direction. A rewrite that
needs new operands builds them with the `Combiner`'s builder, which folds
through `simplify*` first.

## Example
`(x >> 3) << 3` clears the low bits:

```text
%a = lshr i32 %x, 3
%r = shl i32 %a, 3    ; %r = and i32 %x, -8
```

## Runs
Not a pass: called by the combine driver once its slot is flipped.
