# What the hoist is actually waiting on

Not a live-range split. Measured, three times, each answer replacing the
last.

## It is not the allocator refusing

76 bodies in the corpus refuse, all with the same message -- `interferes
with every register at once` -- and **none of them is one of the regressed
programs**. hotlpx, pressx, nested and matrix all colour.

## `_placement` had two bugs, and they are mine

`_placement` replaced `_insertion` when the hoist's allocator came out. It
asks the preheader for every value the run reads, and counted two kinds it
should not:

- a use that is only the previous contents of what the operation writes
  (`merges`), which is not an input
- a value an *earlier operation in the same run* defines -- a run is a
  chain, and the multiply reads the load standing in front of it

With both excluded the hoist fires again on every regressed program.

## And then the programs are wrong

hotlpx, pressx, nested, matrix and nbody miscompile on nine of twelve
configurations. The crossing value comes out of `interference()` with
**one neighbour** and keeps EAX -- which the preheader's own counter has.
A value defined in the preheader and read around the loop cannot have one
neighbour, so the graph is not seeing its range, and `colour()` has no
reason to move anything.

That is the thing to fix. The suspicion is `mir.resolved()` after the
hoist: the use inside the loop becomes a different version joined by a phi,
so the crossing value's own range is the preheader-to-header edge and
nothing more -- and whether the phi's congruence class carries the
interference is the question to answer next.

**Then** ask whether a split is needed. It may not be: if the graph sees
the range, identity is invalid and the existing allocator moves one of
them, which is all the old `_insertion` was doing by hand.
