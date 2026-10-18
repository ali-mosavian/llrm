> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopdistribute.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopdistribute.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopdistribute.rs sha256:e364187149091afdcca3876d81b37a5e0c0464ab46756cd5d55417e9c2f0f87e -->

# loopdistribute

Loop distribution: a loop whose stores do not need each other's trips
becomes one loop for each group of them, so each loop is smaller, holds
fewer values live and may be an idiom (a fill, a copy) of its own.

## How it works
A candidate is a counted loop in canonical form
([`llrm_analysis::cfg::Canonical`](../analyses/cfg.md)) whose one exit is where the loop tests
(`loopshape::controls`), whose body is one path of blocks with no call and
nothing that may trap, whose header, if it tests before each trip, touches
no memory (it runs once more than the body), and from which no value is
read outside (`loopshape`). Its accesses are affine in the counter
([`llrm_analysis::access`](../analyses/access.md)).

Each store seeds a group. The loads a store's value comes from, through any
chain of arithmetic and phis, are in its group, so a load two stores read
joins them: values pass between loops only through memory. Two accesses of
one body, at least one a store, order their groups where they meet
(`dependence::meetings`): `first` meeting `second` at an earlier or the
same trip puts `first`'s group before `second`'s, a later trip the other
way, and both ways one group. Accesses through roots that may be one object
(`access::Apart`) meet both ways. The strong components of the groups are
the partitions, in an order that keeps every edge forward, the first store
first where free: LLVM's `InstPartitionContainer` with its dependence
graph.

Splitting a loop costs a loop's control, so it pays only for a partition
that is no loop: one `loopidioms` takes whole (`loopidioms::takes`: a fill
of a value the loop does not change, or a copy), as in gcc's pattern
distribution. Those stand alone and what lies between them is one loop, as
it was.

```text
  for i: a[i] = 0; b[i] = c[i] * 3        memset(a); for i: b[i] = c[i] * 3
```

Cheap checks come before the copy: what the idioms' stores and their
operands cost, as often as they run (an upper bound on what a split saves),
must be `MIN_PERCENT` of the body's price. The loop is then copied once for
each partition but the last ([`crate::loopclone`](loopclone.md)'s `copied`), each copy
keeps its partition's stores and the original the last's, `loopidioms` runs
over the parts and what nothing reads is removed, and the result is priced
beside the body. Only then is the pipeline run over it, as in
[`crate::unswitch`](unswitch.md), and [`crate::candidate`](candidate.md) keeps it when its price,
spills included, is `MIN_PERCENT` smaller (`Gain::CheaperBy`). At most
`MAX_TRIALS` loop is tried per body, the most nested, a loop of at most
`MAX_LOOP_OPS` operations and `MAX_ACCESSES` accesses, in at most
`MAX_PARTITIONS` partitions.

`LLRM_DEBUG=distribute` prints each loop split and why each other loop
was not.

## Example
Before:
```mir
b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %b2, label %b3

b2:
  %o = mul i16 %i, 2
  %pa = getelementptr i8, ptr %a, i16 %o
  store i16 0, ptr %pa
  %pc = getelementptr i8, ptr %c, i16 %o
  %x = load i16, ptr %pc
  %y = mul i16 %x, 3
  %pb = getelementptr i8, ptr %b, i16 %o
  store i16 %y, ptr %pb
  %i.next = add i16 %i, 1
  br label %b1
```
After, the loop of the store to `a` and the loop of the one to `b`:
```mir
b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  ; ... the test of %n
b2:
  store i16 0, ptr %pa
  ; ... the counter

3:
  %4 = phi i16 [ 0, %b1 ], [ %12, %5 ]
  ; ... the test, %x, %y and the store to %b
```
The first loop is a fill, which the pipeline makes a `memset`.

From unit test `two_independent_stores_become_two_loops` in
`loopdistribute_tests.rs`.

## Runs
Once per body, after `unswitch`, `split` and `fuse`, at -O3 and -Omax
(`-ftree-loop-distribution`). Not run where one pass is selected by name.

## Needs
[`crate::candidate`](candidate.md), [`crate::loopclone`](loopclone.md) (`copied`), [`crate::loopshape`](loopshape.md),
[`crate::loopidioms`](loopidioms.md) (`takes`), [`crate::profit`](profit.md),
[`llrm_analysis::access`](../analyses/access.md), [`llrm_analysis::dependence`](../analyses/dependence.md) (`meetings`),
`induction` (`counted`).

## Keeps
Nothing when it splits: the function is replaced by the candidate.

## See also
gcc `tree-loop-distribution.cc` (`loop_distribution::distribute_loop`,
on at -O3); LLVM `LoopDistribute.cpp` (`InstPartitionContainer`,
`LoopDistributeForLoop::processLoop`; off by default).
[`crate::loopfuse`](loopfuse.md) is the opposite and judges by the same price; it runs
first, so what it joins is not parted again.
