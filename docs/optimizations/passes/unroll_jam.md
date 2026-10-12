> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/unroll_jam.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/unroll_jam.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/unroll_jam.rs sha256:71d00b13881d81fa55a9c5c893e4eab350f7ef079098dd5edf94995c73aa030b -->

# unroll_jam

Unroll-and-jam of a loop nest at -O3: gcc's `pass_loop_jam`
([`gimple-loop-jam.cc`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc)),
LLVM's `LoopUnrollAndJamPass`.

The outer loop is unrolled by N (`loopclone::chained`, the partial unroll
of an exact count) and the N copies of the inner loop that results are fused
into one, so what the outer iterations share is used by all N in one pass
of the inner loop. What the nest may be, as gcc's `unroll_jam_possible_p`
has it: a perfect nest of two header-tested loops, the inner one leaving
only from its header, its count and its counters the same in every outer
iteration, producing nothing the outer loop reads; between the loops only
pure work, which moves ahead of the fused loop. N is the largest the
dependences allow (`llrm_analysis::dependence`), up to `max_unroll`, that
divides the outer loop's exact trip count.

Not yet, narrower than gcc: a trip count that N does not divide (gcc runs an
epilogue), loads between the loops, a rotated (guarded) inner loop, and an
access whose address is not an affine function of the two counters over
an identified object.
