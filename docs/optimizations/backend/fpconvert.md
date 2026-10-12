> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/fpconvert.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/fpconvert.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/fpconvert.rs sha256:7ad0f13bef1afef3788a2159c831287aedf259183e891a0fc07212af981f8335 -->

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

## Runs
`mir fp to unsigned` in `driver::optimized`, after `ehprepare` and before
`selects`, once per module, only where the target's data layout lists
`fptoui.i64` in `expand` (the m16 and m32 x86 targets). Not gated by an -O
level.

## See also
LLVM `TargetLowering::expandFP_TO_UINT`.
