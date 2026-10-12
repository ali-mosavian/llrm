> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/dead.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dead.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/dead.rs sha256:438f097ce93f60b604c4e8a4dd4b9df906d2c826c4ba93dd96d74f1cc9d282e1 -->

# dead

Instructions nothing needs, removed: LLVM's ADCE and DCE.

## How it works
An instruction stays whatever reads it when `llrm_mir::memory::only_value`
says it is more than a value: a store, a volatile access, a call that may
touch memory or not return, a terminator. A call with no effect that returns
is a value and goes.

`transform::live` marks, to a fixed point, every value a staying instruction
reads, directly or through the instructions defining them. An instruction
that stays nowhere and whose result is unmarked is dead. Dead instructions
read only dead ones, so their results are replaced by poison, then all are
erased in one step; phis are handled last (`ssa::pruned_phis`), so a phi
that only dead work read, including a loop-carried cycle nothing outside
reads, goes in the same run.

```text
 %q = mul i16 %a, %b           (nothing reads %q or %p)
 %p = add i16 %q, 1       ==>  (both gone)
 store i16 %a, ptr @g          store i16 %a, ptr @g
```

Lifetime markers go first: those of an object that, leaving the markers out,
nothing that stays reads are erased with it.

`assumptions_dropped` is a separate step over a module. It erases every call
of `llvm.assume` and then runs `dead` on each function until nothing
changes, so a condition kept alive only by its assume costs no compare.

## Runs
Once per pass of the function fixed point, after `algebraic`, and again
after the loop-header copy (`-ftree-ch`). `lsr`, `window`, `indvars` and
`jumpthread` call it on the code they leave. `assumptions_dropped` runs once
after the whole pipeline, before exception-handling preparation and
selection, once every reader has used the assumptions. `-fno-tree-dce` turns
off the pass only, not those calls; nothing runs at -O0.

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## See also
[`crate::dse`](dse.md) removes dead stores; [`crate::promote`](promote.md) makes the loads that
make a store dead; [`crate::loopmotion`](loopmotion.md) sinks stores out of loops.
