> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/slpschedule.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/slpschedule.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/slpschedule.rs sha256:115a208731bc7505abae61ec1e814395903e4491542e769bce1cce21571dc1cd -->

# slpschedule

`BlockScheduling` for a tree of registers: the order the instructions of
the block stand in once a bundle is one instruction.

## How it works
`BoUpSLP::BlockScheduling` (SLPVectorizer.cpp l.19930) makes each bundle's
scalars one `ScheduleBundle`, finds what each instruction waits for
(`calculateDependencies`: the values it reads, and the memory the ones
before it write), and `scheduleBlock` places the ready ones, bundles whole,
in the order the block had where it can. A bundle whose lanes wait on each
other cannot be scheduled and the tree is not made. Here the instructions
of the block are the nodes, the values they read and the order of those that
touch memory are the edges, and the vectors made for the tree are placed
where the last scalar of their bundle stood, with whatever waits on them
after.

DEVIATE SLP6 (owner arm64, exit 2026-12-01): `calculateDependencies`' memory
edges are the order of the writes, and each read between the writes it
stood between, with no alias analysis.

## Runs
Under `slptree` (`schedulable_phis`, `emit`).

## Used by
`slptree`.

## Example
Once a tree is vectorized, the vector of each bundle is placed where the
last scalar of its bundle stood, and what waits on it follows.

```text
store float %f0 / %f1 / %f2 / %f3   (four scalar stores, in this order)
        =>
the <4 x float> chain is emitted in the place of the last scalar it
replaces; the one vector store stands at the last scalar store
```
