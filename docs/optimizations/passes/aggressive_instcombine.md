> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/aggressive_instcombine.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/aggressive_instcombine.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/aggressive_instcombine.rs sha256:02a8d35be6c1a3f1f77d1a69b9ba27c34ae182a5913e0c0436b53e649b0e331d -->

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

## Example
The pair above is one `load i16`; see `aggressive_instcombine_tests.rs`.

## Runs
As `aggressive-instcombine@678`, after `instcombine` and before `tailrec`
(`PassBuilderPipelines.cpp#L678`); not at -O1.

## Needs
The dominator tree, to leave unreachable blocks alone.

## Keeps
The dominator tree and the loops: no block or edge changes.
