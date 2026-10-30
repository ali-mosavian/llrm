> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/adce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/adce.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/adce.rs sha256:af853b5346c8f25c7e03f2d7bf077a5e5af377b47c08bacadfd66aa066d65968 -->

# adce

Aggressive dead code elimination: a translation of LLVM's ADCE
(`ADCE.cpp` at d1106deb71cc). Unlike [`crate::dead`](dead.md), which keeps whatever
feeds a branch, it assumes every instruction dead and proves live only the
ones with effects, the ones they read, and the branches whose outcome
decides whether a live block runs. A dead branch becomes a jump to its
successor closest to the exit, and the code that only fed it goes.

## How it works
Roots are the instructions with side effects, the exits that are not
branches, the terminators of back edges (a loop might not end), and the
blocks that cannot reach a return. Liveness flows backwards to operands;
a live phi makes control-live the predecessors it reads from. When the
data flow settles, the reverse iterated dominance frontier
([`llrm_mir::idf`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/idf.rs)) of the newly live blocks, pruned to blocks whose branch
is still dead, is the set of branches that decide them: those become live
and the walk resumes. What is left dead is removed; each dead branch is
made unconditional to the successor with the highest post-order number of
the reverse CFG, so a path to the exit remains.

```text
 b0: br %c, b1, b2        b0: br b2      (b1 only computes a dead value)
 b1: %d = add ..; br b2   b2: ret
 b2: ret
```

## DEVIATEs
1. No debug-info scopes: `AliveScopes`, `collectLiveScopes` and the dropping
   of debug records outside them are not translated; MIR debug records name
   values, not scopes, and `erase` forgets the ones whose value goes.
2. No `DomTreeUpdater`: llrm's pass manager recomputes dominators and loops
   when the CFG changed, and the post-dominator tree built for the run is
   used only before the first edge goes.
3. The instrumentation-profile call test (`isInstrumentsConstant`): MIR has
   no value-profiling calls.
4. The marking walk is a loop over a stack where LLVM's `markLive` recurses
   through jump chains; the set it reaches is the same.
5. `RemoveControlFlowFlag` is true and `RemoveLoops` false, the pass's
   defaults at -O1 and above.

## Example
Before:
```mir
define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %d = add i16 %x, 1
  br label %b2

b2:
  ret i16 %x
}
```
After:
```mir
define i16 @f(i1 %c, i16 %x) {
b0:
  br label %b2

b1:
  br label %b2

b2:
  ret i16 %x
}
```
(`b1` is unreachable and left to `cfg` cleanup, as LLVM leaves it to
SimplifyCFG.)

## Runs
LLVM runs it after SCCP, BDCE and before MemCpyOpt (slot 35). Not in llrm's
pipeline yet: `llrm-pass adce FILE` runs it alone.

## See also
[`crate::dead`](dead.md) is what it replaces; [`crate::bdce`](bdce.md) is the bit-level one.
