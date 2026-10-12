> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/floatfacts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/floatfacts.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/floatfacts.rs sha256:17fc93c0e864812094c7f15426c14e1e3d894c3d3b9c34d20e746bf9c84096be -->

# floatfacts

Exact finite values of floating-point operations, and the memory they pass
through.

## How it works
A fact is `Finite { value: Fraction, negative_zero }`: a result that is
exactly representable in its format. It therefore holds under any rounding
mode and any flush-to-zero, both of which are the environment's and not
stated in the rich MIR. `nsz`, the frontend's promise, lets an exact
cancellation's zero be +0; without it, a sum or difference that cancels to a
zero of unprovable sign is not a fact.

`rule(unit, inst)` gives an instruction's floating `Rule`: an `Operation`
(`Convert`, `Truncate`, `Neg`, `Abs`, `Sqrt`, `Add`, `Sub`, `Mul`, `Div`),
its operands' `Format`s (`Binary32`, `Binary64`, `Signed(w)`, `Unsigned(w)`)
and its result's. The instructions with one are the arithmetic, `fneg`,
`llvm.fabs`, `llvm.sqrt`, `llvm.lrint`, the casts, and a plain float load or
store (a `Convert` to its own format). `fptosi` and `fptoui` truncate, as
the rich MIR defines them. `evaluated(rule, inputs)` computes the result as
an exact rational and returns it only if it fits the result format (the
significand has at most 24 or 53 bits and the exponent is in range, or an
integer in range); `sqrt` needs a perfect square, `div` a nonzero divisor.

`solved_with(unit, calls)` (or `solved_over`, given the integers) is one
solve: [`crate::consts`](consts.md)' integers through memory (`known_walked`), then the
float facts over them, in program order until nothing changes. A float phi
is known where every incoming value agrees. A float load is read from the
stores [`crate::memoryssa`](memoryssa.md) says reach it, as consts reads an integer one,
and a float store's bits reach its cells as a fact about the stored value
(`_with_stored`).

```text
   %a = fadd double 1.5, 2.0       Add(Binary64)   -> 3.5, exact in 53 bits
   store double %a, ptr @x         Convert         -> cell @x holds 3.5
   %b = load double, ptr @x        read from MemorySSA's providers -> 3.5
   %c = fdiv double %b, 3.0        3.5/3 = 7/6, not exact: no fact
```

Counted float loops. `loop_exits` finds two-block counted loops (a
single-latch loop whose latch ends in its `br` to the header, with a single
preheader) whose float state lives in memory. If induction proves the trip
count it replays the latch `count` times over the cells found before the
loop (`repeated`) and reports what each stored cell holds at exit as a
`LoopExit`. `cells_before` reads the cells one point holds over these facts.

## Provides
`Solved { integers, facts }`, `solved_with`, `solved_over`, `known`,
`converted` (exact float-to-integer results), `loop_exits`, `exits`,
`repeated`, `cells_before`, `touches` (whether a body has any float value; a
body with none skips the solve). The manager entry is
[`crate::manager::FloatFacts`](manager.md). Consumers: `floatfold`, `floatloop` and
`fold` in `llrm-transforms`; [`crate::consts`](consts.md) takes `LoopExit`s through
`known_walked_over`; [`crate::floatbounds`](floatbounds.md).

## Needs
[`crate::consts`](consts.md), [`crate::memoryssa`](memoryssa.md), [`crate::induction`](induction.md) (trip counts),
[`crate::cfg::Shape`](cfg.md).

## See also
LLVM ValueTracking's `computeKnownFPClass`, narrowed to one exact value.
