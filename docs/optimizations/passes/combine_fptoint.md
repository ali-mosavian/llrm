> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_fptoint.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_fptoint.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_fptoint.rs sha256:653ebf290443c15fb838a585256f77de3bf7b5432833fa8cf8e926cb2ad378a8 -->

# combine_fptoint

InstCombine's `visitFPToUI` and `visitFPToSI`, from
[`InstCombineCasts.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp)
at d1106deb71cc.

`visit_fptoi` is
[`visitFPToUI`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L2606)
and `visitFPToSI` (:2616), the same three steps in the same order:
`foldItoFPtoI` (:2469), `foldFPtoI` (:2537), `commonCastTransforms`.
`canBeCastedExactlyIntToFP` (:2197) is `can_be_casted_exactly_int_to_fp`.

MIRROR: the order of the steps and each condition.

DEVIATE 1: MIR has no vectors, and no saturating variant of
`foldItoFPtoI` (`fptosi.sat`) is translated.

DEVIATE 2: `foldFPtoI`'s divisor is read through `f32`/`f64`; an `x86_fp80`
divisor is left alone.

## Example
```text
%f = sitofp i16 %x to double           %r = %x
%r = fptosi double %f to i16
```

## Used by
`combine` (its `visit`, the `FPToUI` and `FPToSI` opcodes).
