> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopmotion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopmotion.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopmotion.rs sha256:a1b6868b6fe32ee933ebe6c40d7ab83ff188112984d62be6b2924cc2f27be005 -->

# loopmotion

Stores nothing inside a loop observes, sunk to the loop's single exit: the
store half of LICM, as `promoteLoopAccessesToScalars` leaves one store
after the loop.

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
