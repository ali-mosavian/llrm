> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopmotion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopmotion.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopmotion.rs sha256:5e3034c6300a558bec88caa5f60270685c079c976c8f7e9f4ae935ef2969ccc1 -->

# loopmotion

Sinks the stores nothing in a loop observes to the loop's single exit, so a
counter or accumulator is written once, not every trip: the store half of
LICM, as `promoteLoopAccessesToScalars` leaves one store after the loop.

## How it works
A loop is taken when it has exactly one exit edge `source -> destination`,
where `source` is the destination's only predecessor and no block of the
loop lacks a successor. Nothing in it may be more than a value, a store
aside (`llrm_mir::memory::only_value`): a volatile access, a call that may
touch memory, or an `invoke` refuses the loop.

A store is moved when it is unobserved (`_unobserved`): not volatile,
through an object's cell with no segment, whose base and root are defined
outside the loop and dominate its header, and which no other access of the
loop may overlap (`regions::overlapping` of the references `Accesses`
holds). A moved store goes to the front of `destination`, so its address
and value must reach there. Three cases:
- it is in `source` (the exiting block): moved as it is;
- the loop has one latch, `source` is the header, and a header phi's entry
  and latch values are what the cell holds on every path into the preheader
  and the latch (`_Exit::exit_value`): the store writes that phi, so an
  accumulator is stored once after the loop. The answer is asked once per
  block, not once per path;
- the store is in the latch and writes a value the loop does not change,
  where the loop certainly runs, or the counter, whose last value
  (`induction::controlling`) is then a number.

```text
 b1: %i = phi [0,b0] [%next,b2]       b1: %i = phi [0,b0] [%next,b2]
     store %i, @n                         %c = icmp slt %i, %k
     %c = icmp slt %i, %k         =>      br %c, b2, b3
     br %c, b2, b3                    b2: %next = add %i, 1 ; br b1
 b2: %next = add %i, 1; br b1         b3: store %i, @n      ; once
 b3: %v = load @n                         %v = load @n
```

Alias is asked again after each loop that changed, because a moved store's
narrowing was its old block's. A moved store keeps the blocks, values and
facts about them: only the points-to solve and memory results are dropped.

## Example
Before:
```mir
define ... @pipeline.body() ... {
b1:
  %0 = alloca i16
  store i16 0, ptr %0, !tbaa !9
  br label %b2

b2:
  %1 = phi i16 [ 0, %b1 ], [ %3, %b4 ]
  %2 = icmp slt i16 %1, 10
  br i1 %2, label %b4, label %b3

b3:
  ret void

b4:
  store i16 %1, ptr @_n, !tbaa !9
  %3 = add nsw i16 %1, 1
  store i16 %3, ptr %0, !tbaa !9
  br label %b2
}
```
After:
```mir
define ... @pipeline.body() ... {
b1:
  %0 = alloca i16
  store i16 0, ptr %0, !tbaa !9
  br label %b2

b2:
  %1 = phi i16 [ 0, %b1 ], [ %3, %b4 ]
  %2 = icmp slt i16 %1, 10
  br i1 %2, label %b4, label %b3

b3:
  store i16 9, ptr @_n, !tbaa !9
  store i16 %1, ptr %0, !tbaa !9
  ret void

b4:
  %3 = add nsw i16 %1, 1
  br label %b2
}
```
The loop runs ten times, so the store to `n` leaves it as the counter's last
value, `9`; the store to the counter's own slot is sunk the same way.

From `--dump` of `ten` (`for (i = 0; i < 10; i++) n = i;`, `n` global) at
-O2 -m16, stages `dead` and `loopmotion` of the first body.

## Runs
In [`crate::pipeline`](pipeline.md)'s per-body fixed point, after `hoist` and
before `trivialunswitch`; on where `hoist` is (`-fmove-loop-invariants`,
every level above -O0).

## Needs
`memoryssa::Accesses`, `Counted`, `Registers`, `cfg::Shape`.

## Keeps
`ExposedFrames`, `Annotated`, `Dominators`, `Loops`, `Registers`,
`AssumptionCache`, `Counted`, `DominatedEdges` and `Bounded`.

## See also
[`crate::hoist`](hoist.md) moves invariant instructions and loads to the preheader; it
sinks no store.
