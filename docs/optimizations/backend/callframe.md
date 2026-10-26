> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/callframe.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/callframe.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/callframe.rs sha256:5b578066d30191e3fc30e101628a79ddc657711bb7a3c6747dfd8913b6c95c76 -->

# callframe

Stores into a reserved call frame become pushes: `X86CallFrameOptimization`.
Translated from
[X86CallFrameOptimization.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp),
function by function in the pin's order: `isLegal` ([L147]), `isProfitable`
([L205]), `runOnMachineFunction` ([L248]), `classifyInstruction` ([L282]),
`collectCallInfo` ([L338]), `adjustCallSequence` ([L505]) and
`canFoldIntoRegPush` ([L605]).

## How it works
A call sequence is `call_frame_setup N, 0, 0` (`ADJCALLSTACKDOWN`: the size,
a second amount, the adjustment the pass sets), stores of the arguments into
the frame above the stack pointer (`mov [sp+k], r|imm`, `and [sp+k], 0`,
`or [sp+k], -1`), the `call`, and `call_frame_destroy N, M`. Where the
stores fill the slots from 0 up with no gap, are all the instructions
between the setup and the call but others that touch neither memory nor the
stack pointer, and the sequence is profitable by the pin's size heuristic,
each store becomes a `push` placed before the call, in reverse order, the
setup's adjustment is the bytes pushed, and a once-read load of the argument
is folded into `push [m]` unless `slowTwoMemOps`.

The input exists in LIR only as hand-built tests: selection pushes the
arguments itself (DEVIATE CG6 of row 50, `pushfold.rs`), and the pass finds
no setup in a body of ours. Its producer is the call lowering of the
SelectionDAG port (`LowerCall`: `ADJCALLSTACKDOWN`, stores to `[esp+k]`);
owner sdag-a, with the reserved call frame that prologue insertion (row 80,
2026-11-13) lays out. Until then the pass does nothing on the corpus.

N/A: the Darwin and Win64 conditions of `isLegal`, the stack probe, the
64-bit forms (`PUSH64`, `INSERT_SUBREG`) and `BuildCFI` (LIR has no CFI
instruction). DEVIATE CF2 (no number yet): `hasVarSizedObjects` is not in
LIR; the pass is given it (false until row 80). Owner w-5, review
2026-11-13.

[L147]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L147
[L205]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L205
[L248]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L248
[L282]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L282
[L338]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L338
[L505]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L505
[L605]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L605

## Example
```lir
call_frame_setup 8, 0, 0
mov [sp+4], v2
mov [sp+0], 7
call f
call_frame_destroy 8, 0
```
becomes
```lir
call_frame_setup 8, 0, 8
push v2
push 7
call f
call_frame_destroy 8, 0
```

## Runs
After `FixupSetCC`, before `FlagsCopy` (the pin's pre-allocation order).
Optimizing levels only, as the pin (`addPreRegAlloc`).
