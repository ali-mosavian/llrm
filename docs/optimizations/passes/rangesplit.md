> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/rangesplit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/rangesplit.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/rangesplit.rs sha256:c9965476e590e6e5e1b1c6673dd8b0d94b59be73de11267ad144062964173eee -->

# rangesplit

A branch on a range test, `(x + c) u< n`, as two branches on `x u>= lo` and
`x u< hi`, for a target whose compare leaves its operand in place.

## How it works
InstCombine and SimplifyCFG join `lo <= x && x < hi` into one unsigned
compare of `x - lo`. Where `x` is read again after the test, the subtract
needs a copy of it (on the 6502 the accumulator holds both, so the value
goes to `x` and back); two compares leave it alone. This pass undoes the
join for the branches whose subtract feeds nothing else; the driver prices
the function with and without it and keeps the cheaper.

DEVIATE 6502-RANGE (owner llrm-target-6502, exit 2026-12-15): it is not
LLVM's. The pin has the join (`foldRangeChecks`, `SimplifyBranchOnICmpChain`)
and no inverse; llvm-mos keeps the two compares of `c >= 'a' && c <= 'z'`
(k_strupper, 47 B / 412 clocks). Exit: the target's own cost for the join in
SimplifyCFG's `FoldBranchToCommonDest`, once it can tell a branch from a value.

## Example
```text
  %s = add i8 %c, -97                 %g = icmp uge i8 %c, 97
  %t = icmp ult i8 %s, 26     =>      br i1 %g, label %in, label %out
  br i1 %t, label %T, label %F      in: %l = icmp ult i8 %c, 123
                                        br i1 %l, label %T, label %F
```
