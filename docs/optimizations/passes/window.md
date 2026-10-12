> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/window.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/window.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/window.rs sha256:454817aecc6ccb00a329834420ffde722d97cc64c8e2b76b041d1001e83a7d8f -->

# window

A counted loop's huge pointers are split into windows, inside each of which
the loop runs on far pointers that never carry into the segment selector.

## How it works
A huge pointer's step carries into the selector, so each step costs the
carry. The target names a window (`Machine::huge_window`: the far address
space, and the number of bytes from a normalized far pointer that no
displacement carries out of); a target without one is left alone. On the
16-bit x86 target it is 64K less one selector step's last byte.

The loop must have one preheader and one latch, and a counted-loop proof
from `induction` whose test is the header's (before each trip) or the
latch's (after). Its walks are the huge pointer recurrences with a
constant step, read only by their own step and by loads and stores at
constant offsets from them. A walk's `fixed` is the span of bytes its
accesses reach beyond the steps; `most`, the trips one window holds, is
the least over the walks of `(window - fixed) / |step|`, and must be at
least 2.

Where the trips provably fit one window (`count <= most`) nothing is
split: each walk is normalized once before the loop by the call
`llrm.ia16.window` (`Intrinsic::Window`), which returns the far pointer
with the lowest offset, and the loop keeps its own test but steps the far
pointer.

Otherwise an outer loop walks the windows:

```text
  preheader:  trips = <count>; (unless guarded, br trips==0 -> exit)
  windows:    remaining = phi [trips, pre], [left, window.next]
              here = min(remaining, most)
              origin_k = window(huge_k)   ; each walk, normalized
              left = remaining - here
  header..latch:  far_k = phi [origin_k], [far_k + step]
              window.trips counts down from `here`, tested after each trip
  window.next: huge_k = far_k, less the accesses' lowest byte
              br left != 0, windows, exit
```

Every other header phi is carried across windows by a phi in `windows`.
The exit's phis read the outer latch; values the loop computes may be read
outside only by the exit's phis. A walk down is normalized at its last
trip's lowest byte.

The split is priced. Each window normalizes every walk (three adds each)
and past the first steps it on as a huge pointer, besides its count and
test; it saves one carry step per walk per trip. For time, trips times the
saving must exceed the windows (`trips / most`, rounded up) times the setup,
where an unknown count is taken as its bound or `profit::UNKNOWN_TRIPS`.
For size (-Os), the saving must exceed one setup. A loop that fails is
left alone. The new branches carry `branch_weights`.

## Runs
Once per function, after LSR chose the pointers (the windows are made over
its recurrences) and before the final `rotate`. Off at -O0 and by
`-fno-strength-reduce`; a no-op on a target with no huge window.

## Needs
`induction` (`counted`, `pointers`, `trips`), `Registers` and `cfg::Shape`
(`llrm_analysis`); [`crate::profit`](profit.md) for the target's prices;
[`crate::counting`](counting.md) for the skip test and constants.

## Keeps
Nothing: a change invalidates every analysis.

## See also
LLVM's IRCE (inductive range check elimination), whose check the carry
into the selector stands in for.
