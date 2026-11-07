> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/aggressive_instcombine.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/aggressive_instcombine.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/aggressive_instcombine.rs sha256:354e16afd1e437a87d7b313881c3fd2b30fe9a9128f8ad55412d242340040edb -->

# aggressive_instcombine

`AggressiveInstCombine`: `TruncInstCombine`, then the folds of
`foldUnusualPatterns`, here `foldConsecutiveLoads`.

## How it works
A translation of LLVM `d1106deb71cc` `AggressiveInstCombine.cpp`, each
function linking the lines it translates. `run_impl` is `runImpl`
([:2674](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L2674)):
the truncs first (`trunc_inst_combine`), then every instruction of every
reachable block, bottom up, is tried against the folds.
`fold_consecutive_loads` finds `(zext(load p) << s1) | (zext(load p+n) <<
s2) ...`, a byte-assembly of adjacent loads, and makes it one wider load.

```text
  before                           after
  %a = load i8, ptr %p             %a = load i16, ptr %p, align 1
  %q = gep i8, ptr %p, 1           (the or's uses take %a zext'd)
  %b = load i8, ptr %q
  %x = zext i8 %a to i16
  %y = zext i8 %b to i16
  %s = shl i16 %y, 8
  %r = or i16 %x, %s
```

DEVIATE (anything MIR or Rust lacks), numbered after `trunc_inst_combine`'s:
38. The scan between the two loads asks `mayWriteToMemory` and no alias
    analysis: any instruction that may write memory between them stops the
    merge (the pin stops only if it may write the loaded bytes). Sound, and
    it merges less. Owner: w2. Exit: the `AliasQuery` of `memcpyopt`,
    2026-11-21.
39. The target's `allowsMisalignedMemoryAccesses` is taken to be true and
    fast for every size the data layout has as a legal integer (the X86
    answer, `X86ISelLoweringCall.cpp#L356`). Owner: w2. Exit: a
    target-supplied fact, 2026-11-28.

40. The pin ends `foldUnusualPatterns` with `SimplifyInstructionsInBlock`;
    there is no public equivalent here, so the dead chain a fold leaves is
    the next `dead` pass's. Owner: w2. Exit: 2026-11-21, expose the helper.
41. `foldConsecutiveStores` ends a run at any instruction that may read or
    write memory, or throw; the pin asks `BatchAA` whether it touches the
    run's base. The pass `Unit` carries no module globals, so a callee's own
    attributes are not seen either. Sound, merges less. Owner: w2. Exit:
    2026-11-21, with deviation 38's `AliasQuery`.
42. `tryToRecognizeTableBasedCttz` marks the select it makes for a table
    whose zero entry is not the width with unlikely branch weights; MIR has
    no `!prof` on a select here, so none is set. Owner: w2. Exit:
    2026-11-21, with select weights.
43. `tryToRecognizeTableBasedLog2` asks the target's `getIntrinsicInstrCost`
    for `ctlz`; a pass run with no `tti` has no answer and does not fold.
    Owner: w2. Exit: 2026-11-21.
49. `tryToFPToSat` is not translated: MIR has no `llvm.fptosi.sat` (no
    interpreter or lowering) and no target prices it. Owner: w2. Exit:
    2026-11-30, when a corpus cell needs it.

## Example
The pair above is one `load i16`; see `aggressive_instcombine_tests.rs`.

## Runs
As `aggressive-instcombine@678`, after `instcombine` and before `tailrec`
(`PassBuilderPipelines.cpp#L678`); not at -O1.

## Needs
The dominator tree, to leave unreachable blocks alone.

## Keeps
The dominator tree and the loops: no block or edge changes.
