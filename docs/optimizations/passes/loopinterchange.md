> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopinterchange.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopinterchange.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopinterchange.rs sha256:f403ca44e659a6073a457aa38ecaee0c7b1990bee0d6337fc3686b0cef8aa6dd -->

# loopinterchange

Loop interchange: the two loops of a nest swap places, so the inner loop
walks the memory the outer one did (gcc's `gimple-loop-interchange.cc`,
LLVM's `LoopInterchange`).

## How it works
A nest (`loopnest`) whose loops each hold only a counter: a header of a phi,
the test of it against a bound, and the branch, and a step of the counter
by a constant. The counters are one type, and read nowhere outside their
loop; the bounds are defined outside the nest. What the outer loop does
before the inner is pure work that only the inner body reads.

The loops swap places when
- the outer count is a constant in range (a bound on the dependence
  distances) and the accesses of the nest are affine over identified objects
  (`loopnest::accesses`);
- every access moves with both loops (one that does not is a repeat, for
  jamming or promotion);
- `dependence::interchangeable`: no pair with a write has a vector `(x, y)`
  of `x > 0`, `y < 0`;
- `dependence::walk_cost` of the outer loop run innermost is less than that
  of the inner, with the cache line the target states (`cache_line`; 64
  where it states none, as gcc's and LLVM's defaults): the bytes an access
  steps, at most a line, none where it does not move.

The blocks stay where they are. The starts, the steps and the tests of the
counters change places, the body's uses of one counter become the other's,
and the outer loop's work before the inner moves into the inner body.

```text
  for i in 0..N                for j in 0..M            (the outer loop's
    for j in 0..M      ==>       for i in 0..N           header now counts j)
      a[j][i] = ..                a[j][i] = ..
```

`LLRM_DEBUG=interchange` says what each nest was refused for.

## Example
`a[j][i] = i + j` over `int a[64][64]`, `i` outer: the inner loop steps 256
bytes. Interchanged, `i` is the inner counter and steps 4.

From unit test `a_column_walk_is_swapped` in `loopinterchange_tests.rs`.

## Runs
Once per body; a swapped nest goes through `sroa` and
the fixed point again. -O3 and -Omax: `-floop-interchange`.

## Needs
`loopnest`, `dependence`, `induction` (the counts), the target's
`cache_line`.

## Keeps
Nothing: a change invalidates every analysis.

## See also
gcc `tree_loop_interchange` (`gimple-loop-interchange.cc:1605`),
`should_interchange_loops` (:1467); LLVM
`LoopInterchangeLegality::canInterchangeLoops`
and `LoopInterchangeProfitability::isProfitable`.
