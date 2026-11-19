> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_ptrcast.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_ptrcast.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_ptrcast.rs sha256:450a9e04041438cdb805c3475c84531aa6db72b652babeb95a433eb489909e7d -->

# combine_ptrcast

InstCombine's pointer-integer casts: `visitIntToPtr` and `visitPtrToInt`
([InstCombineCasts.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L2673)).

## How it works
A cast through an integer of the wrong width goes through the pointer-sized
integer first, so later folds see it. `inttoptr (ptrtoint p + n)` that is
only compared or turned back into an integer is `p + n` as a GEP, and
`ptrtoint` of a one-use GEP chain on `null` or on an `inttoptr` is integer
arithmetic.

## Example
```text
%i = ptrtoint ptr %p to i64              ==>   %q = getelementptr i8, ptr %p, i64 %n
%a = add i64 %i, %n
%q = inttoptr i64 %a to ptr              ; when %q is only compared or ptrtoint'ed
```

MIRROR: the order of the folds and each guard.

DEVIATE 1: the `ptrmask` fold of `visitPtrToInt` (no `llvm.ptrmask`
intrinsic in MIR) and the vector `insertelement` fold are not translated.
Exit 2026-11-12.

## Used by
`combine` (its dispatch on `PtrToInt` and `IntToPtr`).
