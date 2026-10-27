> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/loopslots.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/loopslots.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/loopslots.rs sha256:ae088e9234f37b2a1e47ca4ce3342b1e23066a991a081017f65c096d9ec72b88 -->

# loopslots

Moves a loop's invariant loads, of a spill slot or of a constant, out of
the loop to its entry edges.

## How it works
Allocation spills a value across its whole range, and reloads it in the
loop. A register whose every write inside a natural loop loads one source
the loop never writes (a spill slot, or a constant) and which nothing reads
before that load keeps one load, placed at the loop's entry edges; the
other loads of it are dropped and their values renamed to the first's.
Segment registers qualify too: this is what keeps `ES` set once ahead of a
loop of string moves (`pushw DGROUP; pop es` is a physical definition that
MachineLICM does not hoist, and the pin has no 16-bit target to say).
Loops are taken innermost first; a loop that overlaps one already taken is
skipped.

DEVIATE LS1 (2026-10-25): the pass used to run a second step, `promoted`,
which gave a loop's word-wide spill slots registers the loop left free (a
push and pop around the loop for a register that is live through). Retired:
measured with it off against the same parent (own dists), m16 executed
instructions (`LLRM_DEBUG=cost`, 71 programs, -march=i386) -O1 0.000%, -O2
-0.000%, -O3 -0.000%, -Os -0.001%; m32 (rows, qcport.py, objsweep): QCport
-O1 -0.006%, -O2 -0.009%, -O3 -0.007%, -Os -0.018%; 7 of 568 objects change;
x_transpose +0.55% clocks (-O2) and +0.58% (-Os), code -4.9% and -1.2%. The
other half, `hoisted`, off: m16 executed instructions +0.13% (-O1), +0.13%
(-O2), +0.01% (-O3), +0.18% (-Os), shellsort +8.3%, and m32 -Os rows up to
+3.3% instructions, so it stays.

## Example
A loop of two string moves sets `ES` ahead of the loop, once, not before
each move (`isel_tests::test_near_string_ops_set_es_once_and_save_none`).

## Runs
After `peephole` (spill traffic is final), before `schedule`. Always on;
loops that qualify are rewritten and others are left as they are.
`LLRM_DEBUG=traffic` prints the spill traffic by cause first.

## Needs
[`crate::backend::liveness`](liveness.md) for what each instruction writes and what is
live into the header and the exits, and the loop forest and block
frequencies from `analysis`.
