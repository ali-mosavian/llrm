> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopvectorizeskeleton.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopvectorizeskeleton.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopvectorizeskeleton.rs sha256:c3bfd789b2957b3dbcfe740e43f80e0d454555eb62065355b26e7e1597368031 -->

# loopvectorizeskeleton

The control flow around a widened loop: the scalar remainder
(`InnerLoopVectorizer::createVectorLoopSkeleton`).

## How it works
The pin keeps the original loop as the scalar remainder and runs a vector
loop of `VF x IC` lanes a step in front of it. Here the vector loop is the
original loop widened in place and the remainder is a copy of the loop made
first, so `add_remainder` runs before the widening. The vector trip count
is the trip count less its remainder, `n.vec`; the remainder starts where
the vector loop ends.

```text
preheader:                              preheader:
  br loop                                 %n = sub %bound, %start
                                          %min.iters.check = icmp ult %n, VF*IC
loop:                      =>             br %min.iters.check, scalar.ph, vector.ph
  %i = phi [%start, ..], ..             vector.ph:
  ..                                      %n.mod.vf = and %n, VF*IC-1
  %c = icmp eq %i.next, %bound            %n.vec = sub %n, %n.mod.vf
  br %c, exit, loop                       %ind.end = add %start, %n.vec
                                        loop:  (widened, compares to %ind.end)
                                        middle.block:
                                          %cmp.n = icmp eq %n, %n.vec
                                          br %cmp.n, exit, scalar.ph
                                        scalar.ph:
                                          %bc.resume.val = phi [%ind.end, middle.block], [%start, preheader]
                                        loop.scalar:  (the original loop, from %bc.resume.val)
```

With both bounds constant the checks fold, as the pin's builder folds them:
the preheader falls into the vector loop, which leaves into the remainder.

## DEVIATE rows
- LVS1 the trip count is `bound - start` written as an instruction, not the
  SCEV expansion of the backedge-taken count plus one (the two are one value
  for the loops accepted: an induction stepping by one against an `eq`/`ne`
  exit test). Exit: the vector trip count from the SCEV expander when loops
  with a computed trip count are accepted.
- LVS2 the remainder is the scalar loop only (VP3).
- LVS3 the exit block's phis take invariant values only, but the final value
  of a reduction (the interleaved loops' `bin.rdx`, LVI2), which the middle
  block computes; any other loop-defined value used outside is refused by
  the recipe extraction, LVP2.

## Example
A loop gets a trip-count check, a vector trip count `n.vec` and a scalar remainder loop around it.

```text
%n = sub %bound, %start
%min.iters.check = icmp ult %n, VF*IC   ; br to scalar.ph or vector.ph
%n.vec = sub %n, (and %n, VF*IC-1)      ; vector loop runs to %ind.end
middle.block: br (%n == %n.vec), exit, scalar.ph
```

## Used by
Named by `loopunrollpass`, `loopvectorize`, `loopvectorizeearlyexit`, `loopvectorizeinterleave`.
