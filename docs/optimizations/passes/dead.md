> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/dead.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dead.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/dead.rs sha256:1d0f30aee0e96f9477364fc610b9b9e741df8da5173f0fc1d13208e95d586982 -->

# dead

Instructions nothing needs, removed: LLVM's ADCE and DCE. Work whose result
no one reads only costs code and clocks, and other passes leave it behind.

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

`markers_dropped` is a separate step over a module. It erases every call
of `llvm.assume` and then runs `dead` on each function until nothing
changes, so a condition kept alive only by its assume costs no compare.

## Example
Before:
```mir
define i16 @f(i16 %x) {
b0:
  %doomed = mul i16 %x, 3
  %kept = add i16 %x, 1
  ret i16 %kept
}
```
After:
```mir
define i16 @f(i16 %x) {
b0:
  %kept = add i16 %x, 1
  ret i16 %kept
}
```
`%doomed` has no reader and no effect, so it goes.

From unit test `test_dead_code_goes_and_the_bytes_are_still_accounted_for`
in `dead_tests.rs`.

## Runs
Once per pass of the function fixed point, after `algebraic`, and again
after the loop-header copy (`-ftree-ch`). `lsr`, `window` and `indvars`
call it on the code they leave. `markers_dropped` runs once
after the whole pipeline, before exception-handling preparation and
selection, once every reader has used the assumptions. `-fno-tree-dce` turns
off the pass only, not those calls; nothing runs at -O0.

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## See also
[`crate::deadstore`](deadstore.md) removes dead stores; [`crate::promote`](promote.md) makes the loads
that make a store dead; `crate::loopmotion` sinks stores out of loops.
