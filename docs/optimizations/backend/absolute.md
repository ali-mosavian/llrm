> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/absolute.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/absolute.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/absolute.rs sha256:c126d40adf332c65eacbe959c10358b6c11351b432f9bbbe8d86bc4319489d53 -->

# absolute

Expands `llvm.abs` into a sign mask, an xor and a subtract in MIR before
instruction selection, for the targets whose description lists `abs` under
`expand`.

## How it works
With `s = x >> (w - 1)` (arithmetic), `abs(x) = (x ^ s) - s`; `x` is
frozen first, as it is read twice. The minimum value maps to itself, which
is also what the poison flag allows.

## Example
```text
%r = call i16 @llvm.abs.i16(i16 %x, i1 false)
==> %f = freeze %x ; %s = ashr %f, 15 ; %v = xor %f, %s ; %r = sub %v, %s
```

## Runs
Once per module in `driver::optimized`, for a target that lists `abs` under
`expand`.

DEVIATE 1: only the shift form of `expandABS`: the `smax`/`umin` forms are
taken where those are legal, and here they are the select lowering's.

## See also
LLVM's `TargetLowering::expandABS`
([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L11308)).
