> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/checkedconv.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/checkedconv.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/checkedconv.rs sha256:e8494b8652c8af3001f415947b06aed16fe5f8b37202580dad19e032e5a14a2b -->

# checkedconv

Expands `llvm.experimental.fp{to}{s,u}i.checked`, a float taken to an integer together
with whether it fits, before instruction selection.

## How it works
Where the target converts into an integer wider than the result
(`Tti::wide_float_to_int`: the x87's `fistp` stores a dword or a qword, and
NaN and overflow come out as that width's most negative value), the float is
converted at that width and the check is made on the integer: the result
fits when sign-extending its low part gives the wide value back (unsigned:
when the wide value is below `2^bits`). No float bound is read, so the pool
holds none and no `fcom` runs. Elsewhere the check is two float compares
against the integer's range, as C's `-fsanitize=float-cast-overflow` and
the Nib frontend made them before this intrinsic.

## Example
```text
%p = call { i32, i1 } @llvm.experimental.fptosi.checked.i32.f64(double %x)
==> %w = fptosi double %x to i64 ; %v = trunc %w to i32
    %hi = trunc (lshr %w, 32) to i32 ; %s = ashr %v, 31 ; %ok = icmp eq %hi, %s
```

## Runs
Once per module in `driver::optimized`, after the mid-end: the intrinsic is
opaque to it, and `constantfolding` folds a constant argument.

DEVIATE FPTOINT-CHECKED (permanent): LLVM has no checked float-to-integer
(`llvm.fptosi.sat` saturates, and a float past the range is poison in
`fptosi`); the structure is `X86TargetLowering::LowerFP_TO_INT_SAT`, which
converts at a width the hardware has and decides on the integer
([X86ISelLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86ISelLowering.cpp)).
The pin clamps the float first, which needs a float min/max the x87 lacks,
so the check here is on the integer. The trap stays the caller's: the
intrinsic returns `{ value, fits }`. Reopen when a Nib SSE or m64 target,
or another frontend, emits the intrinsic: `Tti::wide_float_to_int` is
x87-only because no program there could price an SSE form.
