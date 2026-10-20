> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopfuse.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopfuse.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopfuse.rs sha256:a4ca30326e10a87842047c953afb142439a473d60917ad289506a951eb9a63e8 -->

# loopfuse

Loop fusion: two adjacent loops of the same trips become one, so the data
is walked once and the loop's own work is paid once.

## How it works
A candidate is a pair of loops with nothing between them but code that can
run before the first: the first's one exit leads, through blocks of one way
in and one way out, to the second's preheader (`Direct`); or both are
entered behind the same test, the first's skipping to the second's guard
(`Guarded`), and the first's exit leads to that guard. Each loop has one
latch and one exit, taken from its header (tested before each trip) or its
latch (tested after), both the same; and `induction::same_trips` proves one
count for both (LLVM's `haveIdenticalTripCounts`).

The pair is fused when
- nothing the first loop computes is read outside it: the second loop and
  the code between do not read it, and the loop's values do not leave;
- the code between the loops (and the second's guard) has no effect and
  cannot trap, and its operands are available before the first loop;
- no body has work that may trap, and where the loops are tested before each
  trip, no header has work but what can run once more or not at all (a
  header runs one time more than its body);
- the loops have one parent loop, and a `Guarded` pair is tested after each
  trip and reached by no other way than the two guards;
- for a `Guarded` pair, the first's test proves the second's one way and its
  negation the other (`guards::on_edge`, `decides`);
- the accesses of both loops are affine in their counters
  ([`llrm_analysis::access`](../analyses/access.md)), on one root or on objects that cannot be the
  same, and `dependence::fusable` finds no pair the second loop would reach
  before the first (LLVM's `dependencesAllowFusion`).

The second loop's phis move to the first's header and both bodies run in
one trip:

```text
  tested before each trip              tested after each trip
  H0: test; B0 .. L0 -> H0             H0 .. L0: test -> H0 | X0
  H1: test; B1 .. L1 -> H1             H1 .. L1: test -> H1 | X1
  becomes                              becomes
  H0: test; B0 .. L0; B1 .. L1 -> H0   H0 .. L0 -> H1 .. L1: test -> H0 | X1
```

The setup between them moves ahead of the first loop. The copy is a
candidate, as in [`crate::loopversion`](loopversion.md): [`crate::candidate`](candidate.md) runs the
pipeline over it and keeps it when it has fewer loops, no more operations
and no larger price, spills included. At most `MAX_TRIALS` candidates are
tried per body and the loops are at most `MAX_LOOP_OPS` operations
together, so the work stays linear in the body.

`LLRM_DEBUG=fuse` prints each pair fused, and why each other adjacent pair
was not.

## Example
Before:
```mir
b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %more = icmp slt i16 %i, 8
  br i1 %more, label %b2, label %b3

b2:
  ; a[i] = i
  %i.next = add i16 %i, 1
  br label %b1

b3:
  br label %b4

b4:
  %j = phi i16 [ 0, %b3 ], [ %j.next, %b5 ]
  %again = icmp slt i16 %j, 8
  br i1 %again, label %b5, label %b6

b5:
  ; b[j] = a[j] + 1
  %j.next = add i16 %j, 1
  br label %b4
```
After:
```mir
b1:
  %j = phi i16 [ 0, %b0 ], [ %j.next, %b5 ]
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b5 ]
  %more = icmp slt i16 %i, 8
  br i1 %more, label %b2, label %b3

b2:
  ; a[i] = i
  %i.next = add i16 %i, 1
  br label %b5

b5:
  ; b[j] = a[j] + 1
  %j.next = add i16 %j, 1
  br label %b1
```
`a[j]` is read in the trip after `a[i]` is written, so one loop does the
work of two.

From unit test `two_loops_over_one_range_become_one` in `loopfuse_tests.rs`.

## Runs
After `unswitch` and `split`, before unroll-jam, at -Omax only
(`-floop-fusion`): no default pipeline has it, as LLVM's `LoopFuse` is not
in `-O3` and gcc has no such pass outside Graphite. Not run where one pass
is selected by name. The fused loop goes through the scalar rounds again.

## Needs
[`crate::candidate`](candidate.md), [`crate::profit`](profit.md), `induction` (`same_trips`),
[`llrm_analysis::access`](../analyses/access.md) and [`llrm_analysis::dependence`](../analyses/dependence.md) (`fusable`),
`guards`.

## Keeps
Nothing when it fuses: the function is replaced by the candidate.

## See also
LLVM `LoopFuse.cpp` (`collectFusionCandidates`, `dependencesAllowFusion`,
`performFusion`); gcc fuses only in Graphite (`-floop-nest-optimize`).
[`crate::unroll_jam`](unroll_jam.md) fuses the copies of one outer loop's inner loop.
