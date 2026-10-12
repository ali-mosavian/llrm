> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/floatfold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/floatfold.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/floatfold.rs sha256:0b6feec07af4f6f29f2858d6f06830d8863d6f239050a02eb0d75c34b67eca8a -->

# floatfold

Replaces floating values [`llrm_analysis::floatfacts`](../analyses/floatfacts.md) proves exact by
their constants, and applies the algebraic freedoms an operation's
fast-math flags give.

## How it works
`floatfacts` proves a result exactly representable in its format, so the
constant is right under any rounding mode. Three steps run in order over one
solve (the manager's, where it was made for the same call writes):

1. `discarded`: an exact float-to-integer conversion (`fptosi`,
   `llvm.lrint`, ...) has a known integer; every read of it reads that
   number.
2. `stored`: every read of an exact float value, a store's included, reads
   its constant. Whatever stops being read is [`crate::dead`](dead.md)'s.
3. `freedoms`: rewrites that need no analysis, only the flags (`Facts`).

```text
  %a = fadd double 1.5, 2.0          %a = fadd double 1.5, 2.0   (dead)
  store double %a, ptr @x     ==>    store double 3.5, ptr @x
  %b = load double, ptr @x           %b = load double, ptr @x    (3.5 too)
```

The freedoms, per operation: `x + -0.0`, `x - 0.0` and `x * 1.0` are `x`;
`x + 0.0` is `x` with `nsz`; `x * 0` is 0 with `nnan` and `nsz`; `x - x` is
0 and `x / x` is 1 with `nnan` and `ninf`. A constant moves to the right of
a sum or product. A division by a constant is a multiply by its reciprocal
with `arcp`, or without it when the reciprocal is exact (a normal power of
two whose reciprocal is normal). `(x op c1) op c2` is `x op (c1 op c2)` for
a sum or product when both carry `reassoc`; the new flags are the
intersection.

## Runs
Inside [`crate::fold`](fold.md), every time fold runs, which is the pass list's
second entry and after the header copy; there is no pass-list entry of its
own. On wherever `fold` is (-O1 and up).

## Needs
[`llrm_analysis::floatfacts`](../analyses/floatfacts.md) through `manager::FloatFacts`, and
`manager::writes` for what calls write.

## See also
LLVM's `InstructionSimplify` and `InstCombine` floating rules.
