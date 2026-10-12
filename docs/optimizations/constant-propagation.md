# Constant propagation

For contributors to `crates/opt`. Covers `fold` (integers), `floatfold` (floats) and `floatloop`, and the analyses they read: `consts`, `ranges`, `floatfacts`, `floatbounds`.

## What it does

`fold` ([`fold.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-transforms/src/fold.rs#L60)) replaces every value whose number `consts` knows by that constant. Every use sees the constant, and `dead` takes the definition. It also folds a pure operation that a join feeds on each incoming edge, replacing it by a phi of the per-edge numbers, then runs `floatfold` and puts compares and identities in canonical form (`canonical.rs`).

```
 define i16 @f(i1 %c, i16 %x)           define i16 @f(i1 %c, i16 %x)
 b3:                                    b3:
   %p = phi [1,%b1], [2,%b2]              %p  = phi [1,%b1], [2,%b2]
   %y = add %p, %x                        %s1 = phi [11,%b1], [12,%b2]
   %s = add %p, 10             ==>        %y  = add %p, %x
   %t = add %s, %y                        %t  = add %s1, %y
```

(`fold_tests.rs`, `test_an_operation_on_a_join_is_folded_on_its_edges`.) An operation that reads an unknown value too stays.

## Facts

`consts::Known` is "the low `width` bits of this value are `n`" ([`consts.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/consts.rs#L274)). An operation folds only where its operands' widths agree, and a shift keeps the width of the value shifted, not of its count. `consts::known` solves registers to a fixed point (phis are known where every arm agrees; `constant_cycles` handles recurrences that return to a constant).

Memory is solved alongside. A cell is an address root plus displacement and a width in bytes (`Cells`); a store puts byte facts into the cells it covers, and a store to an unresolved address, or an indexed access, kills the cells it may overlap (`regions::overlapping`, with `ranges` intervals narrowing an index). An indexed cell whose index is a known number resolves to a constant offset; an unknown index, an incomplete value or an unresolved root stays unknown.

## What a call writes

`manager::writes` gives each call the references it may write (`CallEffects`, from `Summaries` and the call's `memory(...)`, `readonly` and `readnone` attributes). `consts::_kills` kills only the cells those references overlap; a call that states writes but has no effect list (`memory::unmodeled_write`) clears every cell. `edge_facts` clear all cells at a call, since a fact about reaching an edge says nothing about a callee. So a constant stored before a `readonly` call is still a constant after it:

```
 store i16 9, ptr @g                       store i16 9, ptr @g
 %a = load i16, ptr @g       ==>           call void @clobber()   ; writes @g
 call void @clobber()                      %b = load i16, ptr @g  ; unknown again
 %b = load i16, ptr @g
```

Here `%a` becomes 9 and `%b` is not folded (`test_a_load_of_a_known_cell_is_its_number`). With `@peek` `readonly` in place of `@clobber`, `%b` is 9 too (`a_cell_kept_across_a_readonly_call_is_folded`).

## Intervals

`ranges` ([`ranges.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/ranges.rs)) gives non-wrapping intervals: from the branch edges that dominate a block (`dominated_edges`), from counted loops (`bounded`: the counter, and what the loop computes from it, narrowed by the dominating edges), from `range` attributes and `!range` metadata, and, at -O2 and up, from the ranges every caller passes a parameter (`ipa-ranges`). Add and subtract by a constant step compute their interval and refuse a result that could wrap, so a counter's interval survives its `add i, 1` and `sub i, 1`; a value derived from it by a shift or multiply has the interval of that operation. Consumers: alias decisions (index extent of an access), `decide`, `indvars`, `loopexit`, `hoist`, `algebraic`.

At -O1 and -Og the loop intervals are off (`Options::loop_intervals`); `indvars`, `loopexit`, `hoist` and `algebraic` still ask `Bounded`, which then holds the edge facts and counted-loop counters only.

## Floating point

`floatfacts` proves a float result exact in its format, so it holds under any rounding mode: arithmetic, `fneg`, `llvm.fabs`, `llvm.sqrt`, the casts, `llvm.lrint`, and loads and stores of floats through memory the integer solve knows. `floatfold` ([`floatfold.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-transforms/src/floatfold.rs)) replaces a value so proven by its constant, and an exact conversion's integer by its number; fractional or out-of-range results stay unknown. `floatloop` runs a counted loop of exact float work once, at its last trip, from the memory the earlier trips leave.

`floatbounds::exact` ([`floatbounds.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/floatbounds.rs)) proves float operations exact over a range of integer values: an array element loaded at an index in a known interval, with at most 64 candidate offsets, each decoding to an integer, and results inside the format's exact integer range (2^24 for binary32, 2^53 for binary64). No pass calls it today.

## Levels and flags

| Pass | -O0 | -O1, -Og | -O2, -O3, -Omax |
|---|---|---|---|
| `fold` (with `floatfold`) | x | on | on |
| `floatloop` | x | on | on |
| `ipa-range` | x | x | on |
| loop intervals | x | x | on |

`-fno-ipa-vrp` turns off the caller ranges; no `-f` name turns off `fold`. See [levels](../reference/levels.md).

## See it

`llrm-c --dump DIR` (or `LLRM_MIR_STAGES=DIR`) writes a file for each changed step of each body, `N/NNN-<pass>.ll` (the Nth body run); `fold`'s change is the diff between the files around it. `llrm-mir FILE` prints a MIR file back, `llrm-mir --run` interprets it. Pass-level tests are in `crates/opt/llrm-transforms/src/fold_tests.rs` and `crates/opt/llrm-analysis/src/consts_tests.rs`.

Catalog: [passes](passes.md), [analyses](analyses.md).
