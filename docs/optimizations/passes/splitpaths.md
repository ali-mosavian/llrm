> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/splitpaths.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/splitpaths.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/splitpaths.rs sha256:59e46bc741f2788e14d28f8ad17f8a4b16bab324c939dc73d342b177d63c3a19 -->

# splitpaths

Path splitting: gcc's
[`gimple-ssa-split-paths.cc`](https://gcc.gnu.org/git/?p=gcc.git;a=blob;f=gcc/gimple-ssa-split-paths.cc)
(`pass_split_paths`, -O3), as the rewrite of MIR before isel.


## How it works
A loop's latch `J` that is the join of an if-then-else diamond (`d` ends in
a conditional, each arm of `J` is `d` or a block that only `d` reaches and
that only reaches `J`) is copied into the arm that has a block of its own
and merged into it. The phis of `J` take the arm's value in the copy, so
what read them (an address, an index, the loop's test) is computed from a
constant or a value the arm already has, and `J` and its copy are two
latches.

`is_feasible_trace` decides: the join must be small (under 15 statements),
must not look like an if-conversion candidate (two arms of one statement
each that feed the same phi), and must have a phi that something in the join
uses (a use that is only a compare does not count), or a header phi with an
unchanged path, or an equality the dominating branch decides.

| # | DEVIATE |
|---|---|
| SP-1 | gcc 15.2 treats a join of arms of at most one statement each, and at most three phis, as an if-conversion candidate (`poor_ifcvt_pred`) and does not split it. gcc 13.4, which the bars use, takes only two arms of exactly one assignment each that feed the same phi. Here the narrow form: an empty arm (`side = c ? 1 : 2`) is split, as gcc 13 does (phi_slot_unknown_arm: gcc 15 does not split, 13 does, 205,539 against 207,458 Ir). Owner llrm-target-x86-m64-b; exit when the bars use gcc 15. |
| SP-2 | Done on MIR before isel, not on GIMPLE: the copy is merged into its arm at once (gcc merges after `cleanup_cfg`), and the values the join defined that are read beyond it are repaired with an `SsaUpdater`. Owner llrm-target-x86-m64-b; exit 2026-11-30. |
| SP-3 | Run only where the target says so (`Tti::split_paths_at_o3`) and at -O3, as gcc's `-fsplit-paths` is on at -O3: a target is measured before it takes it. Owner llrm-target-x86-m64-b; exit 2026-11-30. |

## Example
Before, `%j` joins the arms of `%d`, the loop's latch:

```mir
d:
  br i1 %c, label %j, label %arm
arm:
  br label %j
j:
  %side = phi i32 [ 1, %d ], [ 2, %arm ]
  %next = add i32 %i, %side
  %again = icmp ne i32 %next, 0
  br i1 %again, label %d, label %out
```

After, `arm` holds a copy of `j` with `%side` as 2:

```mir
arm:
  %next.2 = add i32 %i, 2
  %again.2 = icmp ne i32 %next.2, 0
  br i1 %again.2, label %d, label %out
j:
  %next = add i32 %i, 1
  ...
```

From unit test `test_a_join_of_an_empty_arm_is_copied_into_it` in
`splitpaths_tests.rs`.

## Runs
Once per body, at -O3 on a target that asks, after `cgpaddr` and before
`ree`. With `wanted("splitpaths")`.

## Needs
Dominators and loops.

## Keeps
Nothing.

## See also
Tests: `llrm-transforms/src/splitpaths_tests.rs`. Levels:
`docs/reference/levels.md`.
