> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/rotate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/rotate.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/rotate.rs sha256:103773d4e09e94d7a33a6a6cc3282c9e2f8addf4e62ba10c1ba7a25ce3065a6b -->

# rotate

Loop rotation: a loop proven to run at least once is entered at its body,
not its test; a loop that is not proven is entered behind a copy of its
test (gcc's `-ftree-ch`).

## How it works
A rotatable loop (`Shape`) has one preheader that jumps only to the header;
one latch that goes only back to the header; a header that holds the exit
test, a conditional branch to the first body block `first` and to the
exit; and a `first` reached from nowhere else, with no phis. What the
header computes besides its phis must be test-only (compares, casts, geps,
selects; a division only where the test is copied before the loop, so
skipping it on entry skips no trap), and a value a header phi takes round
the back edge, a counter's step, is not test-only.

`_rotate` moves the header's phis to `first`: `p(preheader: a, latch: b)`
becomes `p'(preheader: a, header: b')`, phis read in parallel so a swap
needs no order. `ssa::SsaUpdater` rewrites every other use, outside the loop
included: the body reads `p'`, the header and what follows the loop read
`a` as it left the preheader and `b'` as it left the latch. Afterwards the
test is reached only from the latch, and `cfg::merged` merges it into it.

```text
  before                           after (rotated)
  pre -> H: p = phi [a,pre][b,L]   pre -> F: p' = phi [a,pre][b',L]
         if t(p) -> F else X              ...   (the body)
  F: ... -> L -> H                 L: b' = ...; if t(b') -> F else X
```

Two modes, set by the fields of `Rotate`:
- `proven` (`rotated`): the loop has a proven trip count
  (`induction::trip_count`), so its first test passes. The entry goes
  straight to the body. A loop of none or of an unknown count keeps its
  first test.
- `copy` (`copied`): a loop not proven to run is entered behind a copy of
  its test, in the preheader, which skips the loop to the exit when it fails
  (gcc's `should_duplicate_loop_header_p`: a header that ends in the exit
  test and computes only test values). A loop with a known trip count is
  left to `proven`. The header may have at most 20 instructions besides phis
  (`param_max_loop_header_insns`); a chain of header tests (`while (a &&
  b)`) is copied test by test within what is left of that budget. The guard
  takes the loop test's branch weights, else the loop heuristic's, so the
  way into the loop is laid out as the likely one.

A rotated loop is no longer the pre-tested shape the counted-loop
analyses, [`crate::peel`](peel.md) and [`crate::unroll`](unroll.md) read, so rotation goes
after them. [`crate::lsr`](lsr.md) enters a loop it counts to zero at its body too,
behind a skip guard, with the same `_rotate`.

## Runs
Twice in [`crate::pipeline`](pipeline.md), outside the per-body fixed point. `copy` runs
only with `-ftree-ch` (`copy_headers`, off at every level) and not at
`-Os`, after the interprocedural step and before `fixednarrow`, followed by
loop simplification, LCSSA and cleanup. `proven` always runs, last of the
loop passes, after `window`. `LLRM_DEBUG=census` prints what the trip-count
analyses prove per loop, to compare builds with and without `-ftree-ch`.

## Keeps
Nothing where it changes a body.

## See also
gcc's `pass_ch` (`tree-ssa-loop-ch.cc`) and LLVM's LoopRotate.
