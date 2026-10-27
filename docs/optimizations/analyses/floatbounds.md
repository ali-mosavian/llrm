> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/floatbounds.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/floatbounds.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/floatbounds.rs sha256:146ea5f51e6e8251bc48552445ca3e852091cdbb6abd136933082cc27b82d17d -->

# floatbounds

Floating operations that are exact over bounded, not necessarily known,
integers.

## How it works
`exact(unit, constants)` returns the float instructions proven numerically
exact: every value they can produce is an integer of magnitude at most 2^24
(`binary32`) or 2^53 (`binary64`), so conversion and the arithmetic stay
exact. It is interval arithmetic over the same `Rule`s as
[`crate::floatfacts`](floatfacts.md) (`evaluated` on `(low, high)` pairs: `Neg` flips,
`Abs` splits at zero, `Add`, `Sub` and `Mul` combine the endpoints, `Div`
and `Sqrt` have no rule) and fails closed where the result leaves the
format's exact range.

Operands. An integer input is its whole type range. A float is bounded where
floatfacts knows it as an integer constant, or where it was bounded in an
earlier round. A float phi is bounded by the union of its incoming bounds.
Memory: for a load, `_memory` bounds every element it may read, from the
index's interval times its scale (at most 64 elements) and the cell facts at
the load (`floatfacts::cells_before`).

The pass iterates to a fixed point, each round bounding phis and then trying
every pending instruction, so a result proven in one round feeds the next.

```text
   %i  in [0, 100]             int input: its interval
   %f = sitofp i16 %i          Convert      -> [0, 100]
   %g = fmul double %f, %f     Mul          -> [0, 10000]  < 2^53: exact
   %h = fadd double %g, 0.5    0.5 not an integer: not exact
```

## Example
Floats converted from 16-bit integers (`%a`, `%b`) are bounded integers, so
sums stay exact:

```mir
define void @f(i16 %a, i16 %b, i1 %c) {
b0:
  %x = sitofp i16 %a to float
  %y = sitofp i16 %b to float
  %sum = fadd float %x, %y
  %product = fmul float %x, %y
  ; ...
  %half = fmul float %x, 0.5
  ; ...
}
```

| Value | `exact` | Why |
| --- | --- | --- |
| `%x` | yes | in `[-32768, 32767]` |
| `%sum` | yes | in `[-65536, 65534]` |
| `%product` | no | up to 2^30, past 2^24 |
| `%half` | no | 0.5 is not an integer |

From unit test `test_integer_inputs_bound_their_float_work` in
`floatbounds_tests.rs`.

## Provides
`exact`, `evaluated`, `_memory`. No pass consumes them: only this module's
tests call them.

## Needs
[`crate::floatfacts`](floatfacts.md) (`rule`, `Format`, facts and `cells_before`),
[`crate::consts`](consts.md), [`crate::ranges`](ranges.md) (the edges' intervals at a load).

## See also
LLVM ValueTracking's `computeKnownFPClass` with `isKnownExactCastIntToFP`.
