> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/walk.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/walk.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/walk.rs sha256:9c4e424a153085560c372058f5314218ab26d6169ca668a4a66767f9d3ccaa5c -->

# walk

The module passes that LLVM runs between the early function pipeline and
the function simplification (`ipsccp`, `deadargelim`), so that a body they
change goes through the Simplify list once and not again afterwards.

## How it works
`ModuleIpo` is a program pass placed after `Phase::Early`. It applies the
edits of the interprocedural step that used to run after the whole list
and send each body through the Simplify list again (`ipa-deadargs.`). The
bodies it changes are reported to their module's analyses; no pipeline
runs here.

`Walk` is a program pass placed after `Phase::Cleanup`, in place of the
module-wide Simplify stage. Callees' components first, it splices the
calls `inlinecost::sites` accepts into each function, then runs the
Simplify list over it, as LLVM's CGSCC walk does: a loop around a call is
unrolled after the call is inlined (keybind `_action_for`, 8 inlined copies
of `_text_eq`, 325 instructions against 55).

## Deviations
- W1: `deadargelim` runs before the Simplify list, LLVM's after the CGSCC
  walk (`PassBuilderPipelines.cpp` l.1335). Fact: nothing here cleans a body
  after the walk (LLVM's later module optimization pipeline does), and the
  argument a dropped parameter leaves behind is dead code only the list
  removes. Issue: it sees bodies simplified by Early only. Exit: a post-walk
  cleanup stage, then `deadargelim` after the walk.
- W2a: the walk is per function in callee-first order, not per SCC with the
  inliner's own updates; the clocks inliner still runs afterwards;
  `inlinecost::sites` stands for `decide` until it lands. Exit: W2.
- W2: `ipa-cp` clones with the clocks' cost model (`ipacp`), not
  `FunctionSpecialization`; exit W5.

## Example
`@f` is called with an argument its second parameter never reads; it is
dropped before the Simplify list, which then runs once over `@f` and
`@top`:

```mir
define internal i16 @f(i16 %a, i16 %unused) noinline {
b0:
  %r = add i16 %a, 1
  ret i16 %r
}

define i16 @top(i16 %x) {
b0:
  %r = call i16 @f(i16 %x, i16 12)
  ret i16 %r
}
```

From unit test
`test_a_body_changed_by_an_interprocedural_stage_does_not_rerun_the_list` in
`walk_tests.rs`.

## Runs
Once per compile, after the Early stage.

## See also
LLVM `PassBuilderPipelines.cpp` (`buildModuleSimplificationPipeline`:
`IPSCCPPass`, `GlobalOptPass`, `DeadArgumentEliminationPass` before the
CGSCC walk), `DeadArgumentElimination.cpp`.
