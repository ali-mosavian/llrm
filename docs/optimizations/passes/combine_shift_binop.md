> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_shift_binop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_shift_binop.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_shift_binop.rs sha256:5c015bba0910d982d5c690dda23d0777d27c75470ce8e5b50ad6a914593d3fa5 -->

# combine_shift_binop

`InstCombinerImpl::foldBinOpShiftWithShift`: a shift of each arm of a
bitwise operator or `add` is one shift of the operator.

Translation of
[`foldBinOpShiftWithShift`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L959)
at d1106deb71cc.

MIRROR: `(binop1 (binop2 (shift X, S), Mask), (shift Y, S))`, both shifts
logical and by the same amount, becomes `binop2 (shift (binop1 X, Y), S),
Mask` where the operators are the same (and not an `add` under a right
shift), else `shift (binop1 Y, (binop2 X, Mask')), S` with the mask shifted
back by a constant amount where that loses no bit; the arithmetic-shift
case is `(Y >>s S) op ~(X >>s S)`.

DEVIATE 1: no vectors, as everywhere in MIR.

## Used by
`visit_and`, `visit_or`, `visit_xor`, `visit_add`.

## Example
```text
%a = shl i32 %x, 4
%b = and i32 %a, 240                          %c = and i32 %x, 15
%d = shl i32 %y, 4                    ==>     %e = and i32 %y, ...  (see tests)
%r = or i32 %b, %d
```
