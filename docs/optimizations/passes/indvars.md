> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/indvars.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/indvars.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/indvars.rs sha256:6d0e49b3ce9b0cc88114c3eb212194ea0a78822670cec858139355eb24615e27 -->

# indvars

Settles what a loop's counters are worth: a counter's value after its loop
becomes a number, and a narrow counter read as a wider index becomes a
counter of that width. LLVM's IndVarSimplify (`rewriteLoopExitValues`,
`widenIV`) plus an exit-value rewind across an outer loop. Which counters a
loop keeps, and which ends it, is `lsr`'s.

## How it works
One run is these steps, each followed by a refresh of what is known where it
changed the body.

0. `exitvalues::rewritten`: a value read after a loop becomes an expression
   of the loop's count where the loop is left (`rewriteLoopExitValues`,
   ScalarEvolution's exit values); `LLRM_NO_SCEV_EXITVALUES` leaves it to
   steps 1 and 3.
1. `loopexit::evaluated`: values after a counted loop computed from its
   trips.
2. `exitfold::folded`: exits their counts decide are folded.
3. `exit_values` (only with `exit_values: true`, and without step 0): a
   counted exit's phi values that are affine in the counters, with constant
   starts and steps, become numbers. A counter stepped and then tested (`i +
   1 < 32`) leaves as 32, so the exit no longer keeps the old counter alive
   past its step.
4. `exitsink::sunk`: final updates only exit phis read move to the exits.
5. `widened`: a counter read through `zext` (or, where it is signed and
   tested by `slt`, `sext`) to the address's index width, where `induction`
   proves its values never pass its own (`rises_unsigned`, `rises_signed`),
   becomes a counter of that width. C's `unsigned short i` reads `a[zext
   i]`, and `short k` reads `a[sext k]`: a conversion per trip (`inc cx;
   movsx esi,cx; cmp cx,bx`), which hides the loop from idioms and strength
   reduction. A signed counter of a loop of at most `unroll`'s trip limit
   (`IndVars::unrolls`) is left for `unroll` until the run with
   `exit_values`: one operation fewer in the rolled loop made a loop that
   `unroll` took whole no longer fit its own size. The wide counter ends the
   loop against the bound extended once in the preheader; an unsigned or
   equality compare of the narrow one with a value the loop leaves alone
   compares the wide one with that value extended
   (`WidenIV::widenLoopCompare`); other readers of the narrow one read its
   low part.
6. `congruent`: header counters with one start and one step are one counter
   (`SCEVExpander::replaceCongruentIVs`); the later ones' readers read the
   first, and `dead` removes them.
7. `rewound`: see below.

`rewound` carries an exact inner recurrence's exit value around the outer
loop. An inner loop running exactly `count` trips leaves as
`start + count * step`; subtracting that distance on the outer back edge
rebuilds the next start, so the saved start is dead across the hot inner
loop.

```text
 outer:                                 outer:
   inner_pre: br inner                    %st = phi [%start,pre],[%r,latch]
 inner:                                 inner:
   %p = phi [%start,inner_pre],..  =>     %p = phi [%st,inner_pre],..
 exit:                                  exit:
   ...                                    %r = add %p_exit, -distance
```

It applies only for a pair of single-latch loops whose inner loop has one
exit, a known trip count, a counter with constant nonzero step, and a start
defined outside the outer loop and read in it only by that phi. It is
profitable when inner-loop register pressure reaches the target's registers
and the target's `add` costs no more than a `move` and its `memory_update`
no more than a `load` plus `store`: in a register the old copy and the
rewind are equal work; in frame cells the old form is a load and a store,
the new a memory update.

## Example
Step 5, the widening, on `for (unsigned short i = 0; i < n; ++i) a[i] = 1`
with 32-bit addresses. Before:

```mir
target datalayout = "e-p:32:32-n8:16:32"

define i32 @f(i16 %n) {
b0:
  %buf = alloca [64 x i8]
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %go = icmp ult i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %w = zext i16 %i to i32
  %p = getelementptr inbounds i8, ptr %buf, i32 %w
  store i8 1, ptr %p
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i32 0
}
```

After:

```mir
b0:
  %buf = alloca [64 x i8]
  %0 = zext i16 %n to i32
  br label %b1

b1:
  %widen.iv = phi i32 [ 0, %b0 ], [ %widen.iv.next, %b2 ]
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %1 = icmp ult i32 %widen.iv, %0
  br i1 %1, label %b2, label %b3

b2:
  %p = getelementptr inbounds i8, ptr %buf, i32 %widen.iv
  store i8 1, ptr %p
  %widen.iv.next = add i32 %widen.iv, 1
  %next = add i16 %i, 1
  br label %b1
; ... b3 unchanged
```

The loop now ends against `%n` extended once in the preheader, and the
address reads the wide counter: the `zext` a trip is gone.

From unit test `a_counter_extended_to_the_index_width_is_that_wide` in
`indvars_tests.rs`.

## Runs
In the body pipeline after `promote`, with `exit_values: false`. With
`-ftree-ch` (not at -Os) it runs again with `exit_values: true` after the
loop-header copy, where the copy made a counted exit's test the one that
leaves (a number is a `mov` where the counter's register already holds it).

## Needs
`Registers`, `Bounded` (for a `sext` counter), `induction`, `liveness`,
[`crate::loopexit`](loopexit.md), [`crate::exitfold`](exitfold.md), [`crate::exitsink`](exitsink.md),
[`crate::profit`](profit.md).

## Keeps
Nothing after steps 1 to 3; dominators and loops after the others.

## See also
LLVM `IndVarSimplify.cpp`. Tests: `indvars_tests.rs`.
