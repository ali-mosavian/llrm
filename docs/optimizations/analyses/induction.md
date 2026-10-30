> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/induction.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/induction.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/induction.rs sha256:0627049565471a9e4fa37580c0a07469b01178139dc7e311994c24eea4670763 -->

# induction

Which values are affine functions of a loop's counter, and how many trips a
loop makes.

## How it works
Three layers, each built on the one before.

Counters. `basics(unit, loop)` finds the header phis that one invariant or
constant step (`add`, or `sub` of a constant) advances on every way round,
from one start on every way in: an `Affine { value, start, step, header }`,
`start + step * trip`. `pointers` finds pointer phis stepped by constant
bytes through a `getelementptr`. `followers` finds header phis that advance
by at most one a trip wherever a counter advances by one without wrapping.

Trip counts. `counted(unit, loop, ..)` returns a `CountedLoop` proof for a
loop with one latch and one conditional exit that goes on (`_control`: the
branch is in the header or the latch, a latch that only jumps back being the
end of its predecessor). The proof says the counter, its start and bound,
the `IntPredicate` that continues the loop, whether it is tested before or
after the trip (`posttested`) and on the stepped value (`stepped`), and how
the count is reached (`Reach`):

- `Distance`: `bound - start`, one more if the test is inclusive.
- `Solved`: where `start + k * step == bound` modulo the width, as LLVM's
  `howFarToZero`.
- `Ceil`: an ordered test with a step above one that is promised not to
  wrap, the distance divided by the step, rounded up (`howManyLessThans`).

`count` is the exact count when constant, `maximum` bounds an unknown one,
and `first` and `last` are the counter's signed values on the first and last
trip, given only when nothing wraps. A symbolic count is built on demand by
`trips` (and `exit_value`), which asks the caller to place each preheader
operation. `counted_unless_stopped` also accepts a loop that leaves into a
block that never returns (`stops`, using [`crate::noreturn::stranded`](noreturn.md));
`counted_leaving` also other exits that go on. `exits` counts every exit of
a single-latch loop, as LLVM's exit limits. The manager keeps `counted_all`
as the `Counted` analysis, and a unit that carries it is read without a
re-proof.

Recurrences. A `Scev` is a polynomial in loop-invariant unknowns modulo a
width (sums of `Monomial`s, at most 16 terms and degree 4), in one canonical
form so equal values are equal. A `Recurrence` is `pointer + start + step *
trip` with `Scev` start and step: LLVM's add recurrence. `recurrences` walks
a loop and gives every value affine in a counter one (a narrow counter
extended with `zext` too, where its start is made of values the enclosing
loop computes as recurrences and the whole walk stays inside the narrow
width unsigned (the bound is the enclosing loop's own recurrence and trip
count, induction's domain: the signed `ranges::Interval` cannot hold a
16-bit value past 32767, and a second range evaluator would duplicate it):
the wide start names those values as narrow unknowns,
which the expander reads zero-extended; an `nsw` operation past the signed
maximum is poison and proves nothing); `users` lists the
operands that read them, each as an `IvUse` of kind `Address`, `Compare` or
`Basic` with the low bits it observes (LLVM's IVUsers).

```text
  pre:    ...
  header: %i = phi [0, pre], [%n, latch]     Affine{start 0, step 1}
          %c = icmp slt %i, %len             test: Slt, bound %len
          br %c, body, exit                  Reach::Distance
  body:   %p = gep %a, %i                    Recurrence{pointer %a, step 4}
  latch:  %n = add %i, 1                     trips = %len - 0
```

Proofs of removal are `control_replacement` (the counter's phi is read only
by its compare, its step and the exit's phis, so the loop's own control may
go) and `zero_terminating_control` (a counter reaching its final value ends
control and takes no value twice in the trip count).

## Example
A loop that steps by 24 until it reaches `%start + 768`, tested after the
step:

```mir
define i32 @f(i32 %start) {
b0:
  %end = add i32 %start, 768
  br label %b1

b1:
  %i = phi i32 [ %start, %b0 ], [ %next, %b2 ]
  %trips = phi i32 [ 0, %b0 ], [ %up, %b2 ]
  br label %b2

b2:
  %up = add i32 %trips, 1
  %next = add i32 %i, 24
  %c = icmp ne i32 %next, %end
  br i1 %c, label %b1, label %b3

b3:
  %t = phi i32 [ %up, %b2 ]
  ret i32 %t
}
```

`trip_count` is 32, whatever `%start` is (768 / 24). The interpreter returns
32 for starts 0, -5 and 0x7FFFFFF0.

From unit test
`test_posttested_symbolic_sentinel_keeps_its_exact_trip_count` in
`induction_tests.rs`.

## Provides
`AffineOperand`, `Affine`, `CountedLoop`, `Counted`, `ExitCount`, `Scev`,
`Recurrence`, `Users`, `LoopFollowers` and the functions above. Manager
entries: `Counted` (incremental) and `Followers`. Consumers:
[`crate::ranges`](ranges.md), [`crate::alias`](alias.md), [`crate::peelsize`](peelsize.md),
[`crate::difference`](difference.md), [`crate::guards`](guards.md), [`crate::floatfacts`](floatfacts.md), and in
llrm-transforms `indvars`, `lsr`, `loopexit`, `peel`, `unroll`,
`rotate`, `decide`, `hoist`, `expand`, `exitfold`, `counting`,
`window`, `loopidioms`, `floatloop`.

## Needs
[`crate::consts`](consts.md) (the registers the proofs read), [`crate::occurrence`](occurrence.md),
[`crate::noreturn`](noreturn.md), [`crate::cfg`](cfg.md) (loops, dominance).

## See also
LLVM ScalarEvolution add recurrences and trip counts, InductionDescriptor,
IVUsers.
