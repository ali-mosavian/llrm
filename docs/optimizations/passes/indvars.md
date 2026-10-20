> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/indvars.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/indvars.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/indvars.rs sha256:b5eff7fabcd56269f2c17d367b2e89da1b2fd6439c410627e4efb9bd8274a0bb -->

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
   ScalarEvolution's exit values, from every exit of the loop, a rotated
   loop's latch included); `LLRM_NO_SCEV_EXITVALUES` turns it off.
1. `simplifyindvar::simplified_and_extended` (`simplifyAndExtend`): the IV
   users simplified and a counter read through `zext` or `sext` at a wider
   index becomes a counter of that width (`WidenIV`). C's `unsigned short i`
   reads `a[zext i]`: a conversion per trip, which hides the loop from
   idioms and strength reduction.
2. `congruentiv::replaced` (`SCEVExpander::replaceCongruentIVs`; the older
   `congruent` under `LLRM_NO_SCEV_CONGRUENT`): header phis SCEV proves
   equal, a narrow one a truncation of a wide one included, are one, and
   `dead` removes the rest.
3. `canonexit::canonicalized` (`canonicalizeExitCondition`;
   `LLRM_NO_SCEV_CANONEXIT`), `exitopt::optimized` (`optimizeLoopExits`;
   `LLRM_NO_SCEV_EXITOPT`), `exitpredicate::predicated`
   (`predicateLoopExits`; `LLRM_NO_SCEV_PREDICATE`), then
   `exitfold::folded`: exits their counts decide are folded.
4. `lftr::replaced` (`linearFunctionTestReplace`; `LLRM_NO_SCEV_LFTR`): an
   exit tests a counter against the value it has at the last trip.
5. `exitsink::sunk`: final updates only exit phis read move to the exits.
6. `rewound`: see below.

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
In the body pipeline after `promote`. With `-ftree-ch` (not at -Os) it runs
again after the loop-header copy, where the copy made a counted exit's test
the one that leaves (a number is a `mov` where the counter's register
already holds it).

## Needs
`Registers`, `induction`, `liveness`,
[`crate::exitvalues`](exitvalues.md), [`crate::exitfold`](exitfold.md), [`crate::exitsink`](exitsink.md),
[`crate::profit`](profit.md).

## Keeps
Nothing after steps 1 to 3; dominators and loops after the others.

## See also
LLVM `IndVarSimplify.cpp`. Tests: `indvars_tests.rs`.
