> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopversioning.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopversioning.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopversioning.rs sha256:b6a2026a7e6c11a8873555bb2f71605c4b98446c0e624936b06aa5f1f60d346c -->

# loopversioning

`LoopVersioning`: a loop copied, the copy kept for when run-time checks
find that pointers may overlap, and the original told that they do not.

## How it works
A translation of LLVM `d1106deb71cc` `LoopVersioning.cpp`. `version_loop`
needs a loop in simplify and LCSSA form with one exit block. It
1. writes the memory checks of [`crate::runtimechecks`](runtimechecks.md) into the preheader,
   which becomes `<header>.lver.check`;
2. splits off a new preheader and clones the loop before it
   (`<name>.lver.orig`);
3. ends the check block in a branch on the checks: true enters the clone,
   the fall-back, false the original, the versioned loop;
4. gives the exit block a phi for each value the loop defines and uses
   outside, with an arm from each loop, and makes each loop's exits
   dedicated again.

`annotate_loop_with_noalias` then gives every memory instruction of the
versioned loop the scope of its checked pointer group and the scopes of
the groups it was checked against as `noalias`.

## Example
`a[i] = b[i]`: `loop.lver.check` holds `a < b + 4n && b < a + 4n`; if true
control goes to `loop.ph.lver.orig`, else to `loop.ph`. In `loop` the load
reads `!alias.scope !0` and the store `!alias.scope !3, !noalias !0`.

## DEVIATE 29 (no SCEV predicates)
The checks are the memory checks only: `LAI.getPSE().getPredicate()` is
always true here (D1), so there is no `expandCodeForPredicate` and no
`lver.safe`. `version_loop` asserts there is a memory check, the case
where LLVM's predicate alone would be the check. Exit: SCEV predicates.

## DEVIATE 30 (no analysis update)
The dominator tree, the loop info and `MemorySSA` are not updated
(`MSSAU`, the `DT` and `LI` arguments of `SplitBlock`,
`cloneLoopWithPreheader` and `formDedicatedExitBlocks`): the caller finds
them again, as `simpleloopunswitch` does (SU7). The new loop's blocks are
in `LoopVersioning::non_versioned_blocks`. Exit: updatable `LoopInfo` and
`DomTree`. Boundary: `loopversioning_tests` finds both again and checks
that each loop is in simplify form and the module verifies.

## DEVIATE 31 (no entry count)
The new branch is not marked with unknown weights
(`setExplicitlyUnknownBranchWeightsIfProfiled`): MIR has no function
entry count, so LLVM would not either. Exit: entry counts in MIR.

## DEVIATE 32 (SCEV is the caller's)
`forgetLcssaPhiWithNewPredecessor` is not called: the CFG changed, so the
caller drops its expressions (`ScevSession::forget_expressions`).

## Runs
Called by the loop vectorizer and loop distribution once they are
translated; not a pass here (`LoopVersioningPass` is a unit-test shim).
