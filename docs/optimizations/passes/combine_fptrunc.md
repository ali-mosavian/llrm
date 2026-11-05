> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_fptrunc.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_fptrunc.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_fptrunc.rs sha256:d78587cd26b9df3dd481558eec640f4f63b5776e14adf16a397e3cfa7cd73d7f -->

# combine_fptrunc

InstCombine's `visitFPTrunc`, from
[`InstCombineCasts.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L2256)
at d1106deb71cc.

`visit_fptrunc` is `visitFPTrunc`, the same steps in the same order:
`commonCastTransforms`, the narrowing of `fptrunc (binop (fpext x), (fpext
y))` (the `FAdd`, `FSub`, `FMul`, `FDiv` and `FRem` cases and their
double-rounding bounds), `fptrunc (fneg x)`, `fptrunc (select c, (fpext x),
y)`, the unary intrinsics, and `fptrunc (itofp x)` when the conversion is
exact. `getMinimumFPType` (:2142) is `minimum_fp_type`.

MIRROR: the order of the steps and each condition, the fast-math flags each
new instruction takes.

DEVIATE 1: MIR has no vectors (`shrinkInsertElt`, vector constants).

DEVIATE 2: MIR has no `half` or `bfloat`. A constant that fits `half` takes
`float` (precision 24, not 11). Every pair of MIR types the bounds see
(`float`, `double`, `x86_fp80`) gives the same answer for 11 and 24.

DEVIATE 3: an `x86_fp80` constant is held as a double, so it shrinks only
to `float` or `double`, as the pin shrinks a constant that fits.

## Example
```text
%a = fpext float %x to double          %r = fadd float %x, %y
%b = fpext float %y to double
%s = fadd double %a, %b
%r = fptrunc double %s to float
```

## Used by
`combine` (its `visit`, the `FPTrunc` opcode).
