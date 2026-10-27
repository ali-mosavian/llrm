> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/fpconvert.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/fpconvert.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/fpconvert.rs sha256:0de0905d14314661d79a543ded8e697ad38d1bfb9b5feebc4f34b244aa660e13 -->

# fpconvert

Rewrites a float converted to an unsigned 64-bit integer as signed
conversions and a select, in MIR before instruction selection.

## How it works
The x87 stores only signed qwords. LLVM's legalizer (`expandFP_TO_UINT`) and
gcc's `fixuns` expansion do the same: where `x < 2^63` the conversion is
`fptosi x`; otherwise it is `fptosi (x - 2^63)` with the top bit set. Each
`fptoui` to i64 becomes:

```text
 %u = fptoui double %x to i64
                  |
                  v
 %below  = fcmp olt double %x, 2^63
 %adjust = select %below, 0.0, 2^63
 %rest   = fsub %x, %adjust
 %stored = fptosi %rest to i64
 %high   = select %below, 0, 1<<63
 %u      = xor %stored, %high
```

Uses of the old result are redirected to the `xor`. The selects are then
lowered by `selects` as any other (a branch and phi on a 486, which has no
conditional move), so select has one answer.

## Example
Before:

```mir
define i64 @f(double %d) {
entry:
  %u = fptoui double %d to i64
  ret i64 %u
}
```

After:

```mir
define i64 @f(double %d) {
entry:
  %0 = fcmp olt double %d, 0x43E0000000000000
  %1 = select i1 %0, double 0.000000e+00, double 0x43E0000000000000
  %2 = fsub double %d, %1
  %3 = fptosi double %2 to i64
  %4 = select i1 %0, i64 0, i64 -9223372036854775808
  %5 = xor i64 %3, %4
  ret i64 %5
}
```

The `fptoui` is gone: below 2^63 the answer is the signed conversion, above
it the signed conversion of `d - 2^63` with the top bit set by the `xor`.

From unit test
`test_a_float_to_unsigned_qword_is_signed_conversions_and_a_select` in
this file, printed after `expanded`.

## Runs
`mir fp to unsigned` in `driver::optimized`, after `ehprepare` and before
`selects`, once per module, only where the target's data layout lists
`fptoui.i64` in `expand` (the m16 and m32 x86 targets). Not gated by an -O
level.

## See also
LLVM `TargetLowering::expandFP_TO_UINT`.
