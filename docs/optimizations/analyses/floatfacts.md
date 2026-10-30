> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/floatfacts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/floatfacts.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/floatfacts.rs sha256:6ad3eb6fde46b16e3f33d8221c24f521b16899b50657056ecb9973ce39cb8a5b -->

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

Only a load some store of a possibly known value may write is walked for
its providers (`_loads_a_known_store_may_reach`): a value is possibly known
where it is a constant, an integer the solve was given, a phi, an operation
on possibly known values, or a load that a store of one may write
(`memoryssa::Writers`, as `spares` decides it); a load of constant memory
reads its initializer. It is a fixed point from the constants outward and
leaves the facts as they were (`LLRM_CHECK_FLOAT_LOADS` solves both ways and
asserts they are equal).

Counted float loops. `loop_exits` finds two-block counted loops (a
single-latch loop whose latch ends in its `br` to the header, with a single
preheader) whose float state lives in memory. If induction proves the trip
count it replays the latch `count` times over the cells found before the
loop (`repeated`) and reports what each stored cell holds at exit as a
`LoopExit`. `cells_before` reads the cells one point holds over these facts.

## Example
Float values stored and loaded through memory, with exact and inexact
results:

```mir
@m = global [12 x i8] zeroinitializer

define double @f() {
b0:
  store float 2.0, ptr @m
  store double 4.0, ptr getelementptr (i8, ptr @m, i16 4)
  %a = load float, ptr @m
  %b = load double, ptr getelementptr (i8, ptr @m, i16 4)
  %wide = fpext float %a to double
  %sum = fadd double %wide, %b
  %product = fmul double %sum, 8.0
  %quotient = fdiv double %sum, 8.0
  %third = fdiv double 1.0, 3.0
  ; ...
}
```

| Value | `known` |
| --- | --- |
| `%wide` | 2 |
| `%sum` | 6 |
| `%product` | 48 |
| `%quotient` | 3/4 |
| `%third` | none (1/3 is not a binary fraction) |

From unit test `test_known_inputs_reach_float_computations` in
`floatfacts_tests.rs`.

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
