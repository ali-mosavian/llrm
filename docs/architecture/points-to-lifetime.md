# Points-to kept across edits

gcc computes points-to twice per function (`pass_build_ealias` after the early inliner, `pass_build_alias` after IPA inlining) and keeps it across the passes between. Its escape set is one flow-insensitive `ESCAPED` per function; a new call gets `gimple_call_reset_alias_info`, which is "anything"; restrict is held as dependence cliques that the inliner remaps. llrm re-solves for every body state and its escape facts are positional (`escaped_before` per `InstId`). This note is the measured bound of keeping the answer and what an edit must do for it to stay sound.

## Measured bound

QCport host, render, model and game, 97 files. Timers inside `point_values` (value solve) and `escapes` (escape phase), shares of user CPU. A body is a `Function` object (its address): the lineage uid is not one, because a copy takes a new uid and gvn, hoist's trial and inline's restore commit copies. "Beyond two" counts, per body, the runs after its first two.

| | -O1 | -O2 |
|---|---|---|
| bodies | 759 | 759 |
| points-to, all runs | 12.9% | 11.7% |
| **beyond the first two per body** | **11.8%** | **10.8%** |
| `call-effects`, span total | 4.2% | 5.3% |

By uid the bound reads 11.5% and 8.7% over 878 and 3,663 "bodies": the uid splits bodies, and the address does not.

This is an upper bound, and a loose one:

- It counts every later run as free. A run after a new call, a stored pointer, an inline, a changed `Outer` or a copied block is one the rules below must redo. The bound that matters counts only the runs whose edits since the last solve were erase or derive: not yet measured (see Next).
- It keeps two runs per body. A body needs one escape run per configuration its consumers ask: `Pointers` (no arguments), `CallEffects` (arguments and captures), `initialized`, GlobalsAA.

`call-effects` is not on top of this: it is the escape phase and the per-call lists of the same solves.

## What an edit must do

The answer is positional and keyed by value. By edit class:

- **Erase an instruction, or replace a use by a value that points to a subset.** Nothing: a stale set is a superset and the escape sets only shrink.
- **A new value derived from existing ones** (GEP, cast, select): the union of its operands'. A new phi on a back edge is not a union of its operands (an induction `iv = phi [start], [gep iv, step]` would read as the exact offset of `start`): widen the offset as the solver does for a phi in a cycle. lsr, indvars and tailrec make such phis.
- **A new load of a pointer:** everything that has escaped or been stored to, and UNKNOWN. Without UNKNOWN the `nonnull` fact is wrong for a pointer loaded through a parameter.
- **A new alloca:** a new object, not escaped.
- **A stored pointer or a new call:** the object escapes from there on; `escaped_before` after it is stale. Invalidate the body.
- **A copied instruction or block** (unroll, peel, loopclone, jumpthread, tailrec): it has no `escaped_before` entry, and a missing entry reads as nothing escaped (`unwrap_or_default` at alias.rs:692, :1374, :1383). Count a copy as a new call, or make a missing entry read UNKNOWN. New blocks also change back edges, which is where widening happens.
- **Moving an instruction:** an escape moved earlier or a reader moved later, on any path (sinking to a join included), invalidates. An escape moved later or a reader moved earlier leaves a superset: sound.
- **Inline:** invalidate the caller. Restrict roots are `Identity::Int(parameter index)` with no function attached (alias.rs:257), so a callee's roots mapped into the caller collide with the caller's: `f(p, restrict y)` inlined into `g(restrict a)` as `f(0, a)` makes `y` and `a` disjoint. The splice adds calls and stores anyway.
- **The body's signature changes** (deadargs, argpromotion): renumbers `Parameter(i)` and the roots; invalidate.
- **The same address rebuilt on another base** (lsr, gepoffset, addresssink): not a subset of the old; derive from the new base.
- **A changed callee declaration:** the kept result needs the same lineage and, for `Pointers`, the same `Outer` (passes.rs:845-851), so any declaration change in the module drops every body's answer, not the callers'. A per-callee dependence is a change to `Outer`'s identity, not a `Depends` filter.

## Next

The count, no code: replay a pipeline run with the solve kept at its two points, and per pass and per query record where a kept answer differs from a fresh solve (and in which direction), the objects that change, and the runs whose edits since the last solve were erase or derive. Two checks first: `_cell_key` (alias.rs:1701) keys any single stride-1 slice, a widened whole object included, so a widened offset may meet a strong update; and an `LLRM_CHECK_*` that asserts the kept answer is a superset of a fresh solve.
