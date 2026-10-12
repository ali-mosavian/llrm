> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/unroll_jam.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/unroll_jam.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/unroll_jam.rs sha256:2cc6a69e36813877376345be2ba14a3781f20cb279ffbdd1a8b871276a37caf8 -->

# unroll_jam

Unroll-and-jam: the outer loop of a two-loop nest is unrolled by N and the
N copies of the inner loop are fused into one, so what the outer
iterations share is used by all N in one pass of the inner loop.

## How it works
A nest is a loop that holds exactly one other loop and nothing else
besides pure work. The outer header tests and has one way into the loop. A
chain of single-successor blocks `pre` leads from it to the inner header,
and a chain `post` from the inner exit to the outer latch; no other block
is in the outer loop. Work in `pre`, `post` and the outer header is pure
(no memory, no division), and `pre`, `post` have no phis. The inner loop
has one latch and leaves only from its header, into a block only it
enters.

The inner loop must run the same in every outer iteration: its test and the
starts and steps of its counters read only values the nest does not
change, and nothing it computes is read outside it. Its memory accesses
(non-volatile loads and stores; no call, `invoke` or `alloca`) must each
be an affine function of the two counters over an identified object. They
go to `llrm_analysis::dependence::factor` with `max_unroll` (4, gcc's
`unroll-jam-max-unroll`) and `min_percent` (1, `unroll-jam-min-percent`),
which bounds N by the dependence distances and refuses a nest where fewer
than `min_percent` percent of the accesses go away. The outer loop needs an
exact trip count; N is the largest factor from 2 up to the bound that
divides it.

The outer loop is unrolled by `loopclone::chained` into N bodies. In each
copy the work between the loops moves ahead of the first inner loop, the
copy's inner counters are replaced by the first's (one iteration space),
the copy's inner test is dropped, and the first inner loop's body runs on
into the copy's body before the latch jumps back.

```text
  before (x2 copies)               after (N = 2)
  outer:  pre_0                    outer: pre_0, pre_1, post_0
    inner_0: body_0 ->|              inner: body_0 ; body_1 -> inner
    post_0            |              post_1
  outer:  pre_1       |
    inner_1: body_1 ->|            (inner_1's entry goes straight to its
    post_1                          exit; its header is erased)
```

Left alone, narrower than gcc: an outer count N does not divide (gcc runs
an epilogue), loads between the loops, a rotated (guarded) inner loop, and
an access whose address is not an affine function of the two counters
over an identified object.

`LLRM_DEBUG=unroll-jam` prints `jammed a nest` for each nest fused.

## Runs
Once per body, after `unswitch`, so an invariant guard is out of the
nest. If it fused a nest, `sroa` and the fixed point run again over the
result (the fused stores merge there). -O3 only: `-floop-unroll-and-jam`.
Not run where one pass is selected by name. See `docs/reference/levels.md`.

## Needs
`induction` (trip counts), `dependence`, `Registers` (`llrm_analysis`);
[`crate::loopclone`](loopclone.md) (`chained`).

## Keeps
Nothing: a change invalidates every analysis.

## See also
gcc's `pass_loop_jam`
([`gimple-loop-jam.cc`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc)),
LLVM's `LoopUnrollAndJamPass`.
