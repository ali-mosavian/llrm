> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopunrollandjam.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopunrollandjam.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopunrollandjam.rs sha256:c73f033bf8573b08257004ec928cf770df6fcb0b5e072748dbea8b8bc62bd581 -->

# loopunrollandjam

Unroll and jam: a loop with a loop inside is unrolled by `count`, and the
copies of the inner loop are fused into one, so a load of the inner loop
that does not move with the outer one is made once for all the copies
(`Utils/LoopUnrollAndJam.cpp` and `Scalar/LoopUnrollAndJamPass.cpp` at
`d1106deb71cc`).

## How it works
The blocks of the outer loop `L` are the *fore* blocks (before the inner
loop), the inner loop's, and the *aft* blocks (after it).
`isSafeToUnrollAndJam` asks, in order, that the nest is simple (every loop
in simplify and rotated form with one exit and one exiting block, the inner
loop an only child, the aft block a single one, each inner loop's trip count
the same every time round), that no block may throw, that what the header
phis read from the aft block can be moved before the inner loop, and that no
pair of memory accesses of the fore, inner and aft blocks has a dependence
(DA) the fuse would reverse: a direction `<` at the unrolled level that the
inner levels no longer carry forward, or `>` where the copies interleave.

`UnrollAndJamLoop` clones the fore blocks, the inner loop and the aft blocks
`count - 1` times (when the trip count is not a multiple of `count`, an
epilog of the outer loop runs the rest, [`crate::loopunrollruntime`](loopunrollruntime.md)), then
chains the copies: all fore copies, then one inner loop whose header phis
and body are those of the copies in turn, then the aft copies. The outer
loop's latch tests once for the whole set.

```text
 for i              for i step 2
   Fore(i)            Fore(i); Fore(i+1)
   for j        =>    for j
     Sub(i,j)           Sub(i,j); Sub(i+1,j)
   Aft(i)             Aft(i); Aft(i+1)
```

The pass chooses `count` the way the unroller does for the outer loop
(`computeUnrollCount`), cuts it until the jammed inner loop is under
`-unroll-and-jam-threshold` (60), and refuses unless the inner loop has a
load that does not move with the outer one, or the user asked.

## Deviations from LLVM
- D1: on at -O3 and -Omax only, and then with `Partial` and `Runtime` unroll
  preferences on. The pin's pass is off unless `-allow-unroll-and-jam` or a
  pragma asks (`enable-unroll-and-jam` is false) and the targets leave
  `UP.Partial` and `UP.Runtime` off; the `particle` row at -O3 is the
  reason. The pragma and `-allow-unroll-and-jam` ask as they do at the pin.
- D2: no loop metadata is made: not `llvm.loop.unroll_and_jam.followup_*`,
  not `setLoopAlreadyUnrolled` (a function pass cannot add metadata nodes),
  not `llvm.loop.unroll.disable` on the epilog. llrm has no partial or
  runtime unroller to run on the result.
- D3: `LoopInfo`, the dominator tree and SCEV are made again after the
  epilog and after the jam; `DomTreeUpdater` and `addClonedBlockToLoopInfo`
  are not translated.
- D4: the epilog remainder is [`crate::loopunrollruntime`](loopunrollruntime.md)'s: epilog only,
  one exit, no weights.
- D5: no optimization remarks.
- D6: closed. BasicAA's `aliasPHI`/`aliasSelect` are translated; `function`
  of `pragma-explicit.ll` is jammed as the pin does (`function2` still
  differs in the remainder's phis: `tools/loopunrollandjam_expected.toml`).
- D7: `ScalarEvolution::isKnownViaInduction` is a stub (S5), so the outer
  latch compare `niter.next != N` stays where LLVM folds it to `false`
  (`test6`, `test10`). Exit: S5.

## Example
`for i<n { for j<m { s[i] += a[j] * b[i] } }` with `a[j]` loaded in the
inner loop and not moving with `i`: unrolled by 2, the inner loop loads
`a[j]` once for `i` and `i+1`.

## Runs
-O3 and -Omax, in the `Late` stage (`loop-unroll-and-jam@1474`).

## See also
`isSafeToUnrollAndJam` (LoopUnrollAndJam.cpp:862), `UnrollAndJamLoop`
(:214), `computeUnrollAndJamCount` (LoopUnrollAndJamPass.cpp:142),
`tryToUnrollAndJamLoop` (:263).
