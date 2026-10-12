# Value numbering and load reuse

For contributors to `crates/opt`. Covers `gvn` and `equalcalls`: one pass owns reuse of a value, whether it came from arithmetic, a load or a store.

## What it does

`Gvn` ([`gvn.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-transforms/src/gvn.rs#L56)) runs these steps on a body, in order:

1. **Branch equality.** In what the edge to a successor dominates, a branch's condition is that edge's constant (LLVM's `propagateEquality`).
2. **Numbering** (`transform::forwarded`, `transform::subexpressions`, `transform::equal_calls`). A load a known value serves becomes that value: a dominating store of the same bytes and type, or an earlier load of them, with nothing in between that may write them. Two pure operations on the same operands are one, with a copy counting as its source and a commutative pair ordered. Two calls with the same callee and arguments that touch no memory are one.
3. **Join PRE** (`joined`). An expression that every predecessor of a join already computes becomes a phi of those values. With PRE on, a predecessor that lacks it gets the computation inserted on its edge when that edge is unconditional, no loop is entered or left by it and the operands are available there. Divisions and loads are never inserted here.
4. **Load PRE** (`loadjoins`). A load at a join whose bytes every predecessor holds becomes a phi of those values; with PRE, a predecessor holding none gets its own load on its edge where that speculates nothing.

```
 b0: store i16 %x, ptr @g            b0: store i16 %x, ptr @g
     %r = load i16, ptr @g     ==>       ret i16 %x
     ret i16 %r

 b1: store %x, @g   b2: store %y, @g     b3: %r1 = phi i16 [%x,%b1],[%y,%b2]
 b3: %r = load @g                   ==>      ret i16 %r1
```

(`gvn_tests.rs`: `test_gvn_forwards_a_stored_value_to_its_load`, `test_gvn_joins_the_values_each_arm_stored`.) A volatile access is never numbered.

## What serves a load

A load is served by the value a MemorySSA walk finds in its bytes (`avail::forwardable_walk`); a value serves a read only of its own type, so a float store reaches a float load exactly and nothing rounds. `memoryssa::Accesses` says what each instruction reads and writes, `alias::PointsTo` what each pointer may point to, and a call's footprint is its `CallEffects` (from `Summaries`); without `Summaries` every callee is unknown. A call that may write the bytes, or a store that may overlap, ends the walk. A walk past 100 writes gives up and the load stays.

## Pricing

A value held across a store saves a load but lengthens a live range. gvn numbers the function twice where a load was served across a store, once crossing stores and once avoiding it, and keeps the crossing one unless the careful one prices lower (`profit::motion_price`: work weighted by block frequency, plus the spill forecast against the target's registers). Where nothing spills the crossing one stands. Trip counts of loops are the proven ones (`profit::proven_trips`). Without `pre` (-O1, -Og) there is no pricing and a load is served wherever it is redundant on every path: gcc's `-ftree-fre`.

## `equalcalls`

`EqualCalls` ([`gvn.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-transforms/src/gvn.rs#L125)) is the call part alone, run before `tailrec` so that two equal calls are one before one of them becomes the loop's next trip (EarlyCSE precedes tail-recursion elimination in LLVM and FRE does in gcc). It asks no analysis unless the body has two such calls.

## Levels and flags

| | -O0 | -O1, -Og | -O2 | -O3, -Omax |
|---|---|---|---|---|
| `gvn`, `equalcalls` (`-fgcse`) | x | on | on | on |
| PRE steps and pricing (`-ftree-pre`) | x | x | on | on |

`-fno-gcse` turns off both passes; `-fno-tree-pre` only the PRE steps. See [levels](../reference/levels.md).

## Floats

A float computation is numbered like an integer one. What is exact is a separate question, answered by `floatfacts` and used by `floatfold` ([constant-propagation](constant-propagation.md)).

## See it

`--dump DIR` writes the module after `gvn` (`N/NNN-gvn.ll`). gvn has no `LLRM_DEBUG` channel; compare the dump before and after. Tests: `gvn_tests.rs`, `loadjoins_tests.rs`.

Catalog: [passes](passes.md). Analyses it reads: [analyses](https://github.com/ali-mosavian/llrm/tree/main/crates/opt/llrm-analysis/src).
