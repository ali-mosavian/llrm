> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cmpopt.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cmpopt.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cmpopt.rs sha256:f6e4aa9f030f8453435f3af260898c6c4a1e8ebf45da44483b0d65d639023f18 -->

# cmpopt

CodeGenPrepare's `optimizeCmp` steps after the overflow combines: the
power-of-two test of `ctpop` unfolded, an equality made the opposite of the
signed compare that dominates it, and compare operands swapped to match a
subtract
([CodeGenPrepare.cpp#L8805](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L8805)).

## How it works
For each `icmp`, in the pin's order, the first that acts ends the visit.

`unfoldPowerOf2Test`: `ctpop(x) == 1`, `ctpop(x) u< 2` and their inverses,
where `ctpop` is not fast (no POPCNT), become `(x & (x - 1)) == 0` when x
may be zero or is known non-zero, else `(x ^ (x - 1)) u> (x - 1)`.

`foldICmpWithDominatingICmp`: only where the target does not lower an
equality with the signed compare beside it (PowerPC), so never on x86:
`a == b` in the false arm of `a < b` becomes `a > b` with its branches and
selects swapped, to share one compare.

`swapICmpOperandsToExposeCSEOpportunities`: a compare of `a` and `b` with a
`sub b, a` among the users of `a` reads `b`, `a`, so the subtract and the
compare are one instruction.

```text
 %p = call i32 @llvm.ctpop.i32(i32 %x)         %m = add i32 %x, -1
 %c = icmp eq i32 %p, 1                  ==>   %a = and i32 %x, %m
                                               %c = icmp eq i32 %a, 0   ; x known non-zero
```

MIRROR: the three functions. DEVIATE CMP1: `isCtpopFast` is false for every
x86 CPU llrm builds for (none has POPCNT), not a query of the CPU. Exit:
the CPU-carrying `Lowering` (row 23, 2026-12-09).

## Example
`test_ctpop_equal_one_of_a_nonzero_value_is_an_and_with_its_predecessor` in
`cmpopt_tests.rs` is the sketch above.

## Runs
In `cgpaddr`'s run, after the compare sinks.
