> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr.rs sha256:83ec200438b55cf556fca7a4a807704787f3cf077e232be0506123a3c9aef4d6 -->

# lsr

Strength reduction: replaces a loop's induction variables, and the address
arithmetic built from them, with the cheapest set of counters the target
can address, so the loop carries few registers and no multiply (LLVM's
LoopStrengthReduce, gcc's ivopts).

## How it works
Every read of a counter, or of a value affine in one, is one
`induction::Recurrence`, `pointer + start + step * trip`, with `start` and
`step` forms in invariant symbols (`Scev`, modulo the width).
`induction::users` finds each recurrence and how it is read: as an
address, compared with an invariant, or otherwise. A recurrence times a
recurrence is not one.

Loops are taken innermost first (smallest body first), each once. For one
loop the pass needs a preheader, one latch, and a latch arm for every
counter; else the loop is left. A loop of more groups of uses than
`Bounds::groups` (gcc's `iv-max-considered-uses`) is left too. Three steps
come before the choice:
- a post-tested latch compare of a counter with a constant reads the
  counter's step instead, with the bound a step on, so one register serves
  for two (LLVM's `OptimizeLoopTermCond`);
- `crate::mirror` makes the pointer for a counter only some trips step,
  where the target's addresses take no scale (not at `-Os`);
- the loop is then planned again from the new body.

The candidates are recurrences the loop could carry in a register:
- each use's own, and its start with the symbols split every way between
  counter and base (up to three symbols), with and without the constant;
- the loop's existing counters;
- one counter per scale of an address form whose index is wider than the
  use;
- each step counted to zero at the exit, `-trips * step`, where the loop has
  a counted exit.

A use is realized from a candidate `r` as `base + k * r + c` (`Fit`): the
address form that takes it, or the shift, multiply and add that compute
it. A value read after the loop may come from the trip the candidate
counted, or from the known count. An equality with an invariant takes the
candidate itself and moves the bias into the invariant. The counted exit
either keeps its compare or tests one candidate for its last value
(`Exit`), which costs nothing where that is zero. A counter read only by
its own step and the exit is then dead and `dead` removes it, so a loop
counting `i` from 0 to `n` keeps one counter, counted to zero.

```text
 for (i = 0; i < n; i++) s += a[i] * b[i]      (a, b: 16-bit elements)

 before                          after, one counter counted to zero
 pre:                            pre:  c0 = -n; ea = a + 2n; eb = b + 2n
 l1: i = phi [0,pre] [i+1,l2]    l1:   c = phi [c0,pre] [c+1,l2]
     if i < n  -> l2 else exit         if c != 0 -> l2 else exit
 l2: o  = i * 2                  l2:   pa = ea + c*2   ; the address
     pa = a + o                          pb = eb + c*2   ; form takes *2
     pb = b + o
```

A candidate may test the exit only where it cannot come round to its final
value on an earlier trip. Its modular period is `2^w / gcd(step, 2^w)`;
the loop's most trips must not exceed it (nor equal it, where the test is
before a trip). The compare becomes an equality or inequality against the
candidate's last value. Only a pre-tested unit step has a symbolic
count, so a counter stepping by more keeps its compare unless the count is
a number.

```text
 i = 0..10 step 1, p = &a[i]            p = a, end = a + 10*step
 loop: *p = ...; p += step; i++    ==>  loop: *p = ...; p += step
       exit if i == 10                        exit if p == end
```

The price of a set of candidates (`Problem::total_of`) is the sum of
- an add per trip for each counter's step (a pointer's step is
  `profit::advance`; a counter wider than the native index pays the
  operand-size prefix);
- each use's cheapest realization, at its block frequency;
- building starts, steps and invariants in the preheader;
- a copy for each address that puts two values in the wrong register class;
- the spill traffic of every value past the target's registers
  (`crate::spill`, the one spill model `gvn` shares).

Frequencies are `Products`: trips multiplied, ten where unproven (#203),
a branch's cold arm less. Prices, registers and address forms are the
target's (`OperationCosts`, `Machine`, `AddressForm`): the pass prices, it
does not know the machine. `kept_across` says how many registers a call to
a named callee keeps, from its contract, and `multiply_by` what a multiply
by a constant costs, from the backend's shift and add chains. A spilled
counter costs a memory update a trip, an invariant a load per read.

The search is gcc's `find_optimal_iv_set`: from the loop's own counters
and from a greedy start (each use's cheapest candidate, added until every
use fits), add, remove or swap one candidate while the price falls, a tie
going to fewer counters; the cheaper end wins. Past
`Bounds::all_candidates` candidates, replacing is tried once, only where
no add or remove helps, and not for a candidate serving more than
`Bounds::always_prune` uses or exactly one. The loop is rewritten only
when the set beats the counters it has.

A loop with another way out was never counted before, so its rewrite is
kept only if the whole function's price (`profit::motion_price`) does
not rise; at `-Os` it is left as it is.

A symbolic count holds only once the loop is entered, so a loop tested
for its last value behind one is entered at its body behind a skip guard
(`counting::skip_guard`, `rotate`), unless a branch over the entry already
proves the first trip (`induction::guards`, as LLVM's
`isLoopEntryGuardedByCond`).

In a space where a pointer carries into its selector, an integer cannot
index a pair: the use advances the pointer, and pays what an advance
costs there (`profit::advance`). `crate::window` splits the huge pointers
this chooses.

## Example
Before:
```mir
define ... @"_sum@3"(ptr nocapture readonly %0, i16 %1) ... {
b1:
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %10, %b4 ]
  %3 = phi i16 [ 0, %b1 ], [ %9, %b4 ]
  %4 = icmp slt i16 %2, %1
  br i1 %4, label %b4, label %b3

b3:
  %5 = phi i16 [ %3, %b2 ]
  ret i16 %5

b4:
  %6 = mul i16 %2, 2
  %7 = getelementptr inbounds i8, ptr %0, i16 %6
  %8 = load i16, ptr %7, !tbaa !9
  %9 = add nsw i16 %3, %8
  %10 = add nsw i16 %2, 1
  br label %b2
}
```
After:
```mir
define ... @"_sum@3"(ptr nocapture readonly %0, i16 %1) ... {
b1:
  %2 = shl i16 %1, 1
  %3 = sub i16 0, %2
  %4 = getelementptr i8, ptr %0, i16 %2
  %5 = icmp sle i16 %1, 0
  br i1 %5, label %b3, label %12

b3:
  %6 = phi i16 [ 0, %b1 ], [ %10, %13 ]
  ret i16 %6

b4:
  %7 = phi i16 [ %10, %b4 ], [ 0, %12 ]
  %lsr.iv1 = phi i16 [ %lsr.iv.next, %b4 ], [ %3, %12 ]
  %8 = getelementptr i8, ptr %4, i16 %lsr.iv1
  %9 = load i16, ptr %8, !tbaa !9
  %10 = add nsw i16 %7, %9
  %lsr.iv.next = add i16 %lsr.iv1, 2
  %11 = icmp ne i16 %lsr.iv.next, 0
  br i1 %11, label %b4, label %13
  ; ...
}
```
The counters `i` and `2 * i` are one: `%lsr.iv1` runs from `-2n` to zero,
the address is `%4 + %lsr.iv1` with `%4 = a + 2n`, and the loop ends when
`%lsr.iv1` reaches zero.

From `--dump` of `sum` (`for (i = 0; i < n; i++) s += a[i];`, `int *a`) at
-O2 -m16, stages `fixednarrow` and `lsr`.

## Runs
Once per body, after the interprocedural step and `fixednarrow`, before
`algebraic::Differences` and `window`: nothing after it chooses a loop's
counters. It is in `-O1` and up with `-fstrength-reduce`; at `-Omax` the
search has no `Bounds` (gcc's values apply otherwise). See
[`crate::pipeline`](pipeline.md) and `docs/reference/levels.md`.

## Needs
`induction` (recurrences, counted exits, guards), `Registers`, `Pressure`
(`crate::spill`), `Products`, [`crate::counting`](counting.md) and [`crate::expand`](expand.md)
to build starts and guards, [`crate::rotate`](rotate.md) to enter at the body.

## Keeps
Nothing: a changed loop invalidates every analysis.

## Limits
A loop tested on a value other than a counter plus a constant (a scaled
compare) keeps a counter for the compare. Past the target's registers the
pressure estimate is coarse. Which value is the base and which the index
of a two-register address is the backend's choice, and it decides some
outcomes.

## See also
[`crate::indvars`](indvars.md) keeps the exit values, exit sinking and `rewound`: which
counters the loop keeps is this pass's. LLVM:
[`LoopStrengthReduce.cpp`](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp).
gcc:
[`tree-ssa-loop-ivopts.cc`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-loop-ivopts.cc).
`LLRM_DEBUG=lsr` prints each loop's uses, candidates and costs.
