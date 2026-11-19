> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_fabs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_fabs.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_fabs.rs sha256:2b205f3bb1274d6cf4920d3a1033703c81040897f2bc4cf43e61d4db5edcc459 -->

# combine_select_fabs

`foldSelectWithFCmpToFabs`: `x < 0.0 ? -x : x` is `fabs(x)` where the user
ignores the sign of zero and of a NaN.

Translated from
[`foldSelectWithFCmpToFabs`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L3321)
at d1106deb71cc, with `canIgnoreSignBitOfZero` and `canIgnoreSignBitOfNaN`
of `llrm_mir::valuetracking::signbit`. `-0.0` makes it tricky: the fold
needs `nsz` or a user that does not read the sign of a zero (`fptosi`,
`fcmp`, `fabs`), and `nnan` or a user that does not read the sign of a NaN.

## Example
```text
%c = fcmp olt x86_fp80 %x, 0.0                  %a = call @llvm.fabs.f80(%x)
%n = fneg x86_fp80 %x                  =>       %i = fptosi %a to i32
%s = select i1 %c, %n, %x
%i = fptosi x86_fp80 %s to i32
```
x_ldcmp -O2 at m32 compared and branched for every `|x|` the pin takes
with `fabs`.

DEVIATE 1: `llvm.fabs` is made with the select's fast-math flags
(`CreateFAbs(X, &SI)`), and the result of `-fabs` with its flags, as the
pin does; `prof` metadata is not carried (MIR has none on a select).

## Used by
Named by `combine_visit_select`.
