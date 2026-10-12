# Dead code and dead stores

For contributors to `crates/opt`. Covers `dead` (values), `dse` (stores), `promote` (the loads that make a store dead) and `loopmotion` (stores sunk out of loops).

## `dead`

[`dead.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-transforms/src/dead.rs): an instruction goes when nothing that stays reads its result. What stays whatever reads it is what `llrm_mir::memory::only_value` says is more than a value: a store, a volatile access, a call that may touch memory or not return, a terminator. A call with no effect that returns is a value and goes. `transform::live` marks every value a kept instruction reads, to a fixed point; `dead` replaces the rest by poison, erases them, then prunes phis only dead work read. A lifetime marker of an object nothing else reads goes too. After the last pass `assumptions_dropped` removes `llvm.assume` calls and what only they read, so a condition kept alive by its assume costs no compare.

```
 %q = mul i16 %a, %b              (nothing reads %q, %p)
 %p = add i16 %q, 1       ==>     (both gone)
 store i16 %a, ptr @g             store i16 %a, ptr @g
```

## `dse`

[`dse.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-transforms/src/dse.rs): a store is dead when every path overwrites its cell before reading it, or when nothing after it can read the cell. `avail::dead_stores` finds them; `observers::private` says which cells no call and no exit observes. A call's reach is its `CallEffects`; without `Summaries` every callee is unknown. Reads: memory operations and the CFG.

## `promote`

[`promote.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-transforms/src/promote.rs): write-through promotion. A load of a cell that a store left its value in on every path becomes that value, with a phi where paths join; every store stays, for `dse` to remove. `sroa` does the same for the leaves of aggregates. A cell is an exact leaf, or its pointer decomposed over any root, and one LLVM type per cell: a store keeps its type's value.

## `loopmotion`

[`loopmotion.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-transforms/src/loopmotion.rs): a loop store that nothing inside the loop observes is sunk to the loop's single exit (LICM's store sinking), so a loop that updates a cell each trip stores it once. A volatile access, a call that may touch memory, or an `invoke` in the loop refuses it.

## Levels and flags

| Pass | Flag | -O0 | -O1 and up |
|---|---|---|---|
| `dead` | `-ftree-dce` | x | on |
| `dse` | `-ftree-dse` | x | on |
| `sroa`, `promote` | `-ftree-sra` | x | on |
| `loopmotion` | `-fmove-loop-invariants` | x | on |

`dead` also runs inside other passes (after `lsr`, `indvars` and `fold`) to take what they leave.

Catalog: [passes](passes.md). Analyses: [analyses](analyses.md).
