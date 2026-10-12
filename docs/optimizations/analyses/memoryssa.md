> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/memoryssa.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/memoryssa.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/memoryssa.rs sha256:b05678c1b4d9a53f2d3d44519e1585ab014530f4bd1b6c83d9ba9ef5ceb5c6cb -->

# memoryssa

Memory SSA: a single memory state versioned by stores and writing calls,
with a walker that finds the writes a load may see.

## How it works
`Accesses` records what each instruction touches: a load's or store's own
`MemRef`, a call's footprint (its `CallEffects`, or anything its attributes
allow), and the bytes a call fills before reading any. It is the manager's
`accesses` analysis, built from [`crate::manager`](manager.md)'s `Annotated` and
`CallEffects`, and kept across changes that only move instructions or give a
store another value (`update`).

`built` numbers the accesses. Every instruction that writes (`None` writes
means anything) is a `Def`; one that only reads memory is a `Use`; one that
touches none is skipped. Each access names the access before it in its block
as `defining`. Each block starts in a memory `Phi` over its predecessors'
exit states, with the entry block also fed by `Live`, the state on entry to
the function. Phis are wired before they are resolved, so back edges need no
iteration over memory versions; then phis whose inputs reduce to one state
are replaced by it.

```text
        entry: Live(0)
          |
   b1: Def s1 --(defining)--> Def s2 ---+
          |                              |
   b2: Phi [b1: s2, b3: s3]  <-----------+----- b3: Def s3
          |
         Use l1   (defining = Phi)
```

The walker (`clobbers`, `clobbers_at_end`, `nearest_writes`) answers, for a
`MemRef` read at a site, the set of nearest defs whose write may change it,
or `Live`. It steps up `defining` links and through every phi input,
skipping defs that `may_clobber` rules out: a constant or `invariant` read
is changed by nothing; two references at fixed displacements of one frame
whose bytes miss are settled by `regions::displaced_span`; otherwise
[`crate::pointerfacts`](pointerfacts.md) disjointness is tried before `regions::overlapping`.
Per (cell, def) answers are remembered, and a walk jumps along chains of
accesses that leave the cell alone. After enough steps per access, a
persistent map from (object, byte) to the nearest def writing it replaces
stepping. A visited set closes cycles without taking a back edge as evidence
that memory is unchanged. An empty answer means no reachable source, not a
reusable value; the caller must establish value availability itself.
`nearest_writes` stops after `MAX_NEAREST_WRITES` (100) and the caller
serves nothing past that. `unchanged` and `available_on_edge` answer whether
an earlier access still supplies the bytes, the caller having proved
dominance and equal addresses.

Predicates shared with the passes: `changes`, `spares`, `covers`/`covered`
(outer references certainly hold every byte of the inner), `placed`,
`same_bytes`, `byte_keys`, and `Writers`, which indexes writes by object.

## Provides
`Accesses` (analysis `accesses`), `MemorySSA` via `built`, `Kind`, `Access`,
`Reach`, the predicates above. Consumers: [`crate::avail`](avail.md),
[`crate::consts`](consts.md), [`crate::floatfacts`](floatfacts.md), and in llrm-transforms `gvn`,
`dse`, `hoist`, `loadjoins`, `loopmotion`, `promote`, `spares`, `homes`,
`transform`.

## Needs
[`crate::manager`](manager.md) (`Annotated`, `CallEffects`, `ExposedFrames`),
[`crate::cfg`](cfg.md), [`crate::regions`](regions.md), [`crate::pointerfacts`](pointerfacts.md),
[`crate::memory`](memory.md).

## Keeps
Conservative: rebuilt after a function changes. `Accesses` survives moves.
`LLRM_CHECK_CLOBBERS` and `LLRM_CHECK_JUMPS` assert the fast paths against
the slow ones.

## See also
LLVM `MemorySSA` and its `ClobberWalker`
(`llvm/lib/Analysis/MemorySSA.cpp`).
