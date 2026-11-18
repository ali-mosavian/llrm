> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_fcmp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_fcmp.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_fcmp.rs sha256:4ddf13854422da4b919df565842d49e4b74a268cf8d5c4c77bd8ae0e3eff80e8 -->

# combine_fcmp

InstCombine's `visitFCmpInst`, from
[`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L9212)
at d1106deb71cc.

MIRROR: the order of the steps and each condition. `simplifyFCmpInst` is
the generic `simplify_instruction_in` that `visit` runs first.

DEVIATE 1: only the opening steps (up to the infinity canonicalisation,
L9215-L9325) are translated; the fold-by-operand-kind ones (bitcast, fsub,
phi, sitofp, fptrunc, fabs, sqrt, floor/ceil, the min/max-select exit) are
not. Exit: the next PR on `combine_fcmp.rs`.

DEVIATE 2: only the null value of a vector compare is vector-aware.

## Example
```text
%c = fcmp olt float %x, 0x7FF0000000000000     %c = fcmp one float %x, 0x7FF0000000000000
```

## Used by
`combine` (its `visit`, the `FCmp` opcode).
