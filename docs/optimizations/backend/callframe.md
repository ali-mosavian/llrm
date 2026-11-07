> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/callframe.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/callframe.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/callframe.rs sha256:cabc0db11993231f021fa8a2d40af48e9e92aab720912717249bff45c9dfd4fe -->

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

The producer is the call lowering of the SelectionDAG port: `LowerCall`
stores the arguments to `[sp+k]` between `ADJCALLSTACKDOWN` and
`ADJCALLSTACKUP`. `emit.rs` turns the pair into `call_frame_setup` /
`call_frame_destroy` at m32 only (`AddrMode::Flat32`, DEVIATE CF4); the
pass is in the flow at every x86 width. At m16 (`Real16`) and m64 `emit.rs`
makes `sub sp, N` / `add sp, N - M` itself, so the pass finds no setup and
changes nothing there. The m16 steps left: emit the pseudos for `Real16`
(`slot` 2, 16-bit `Held` cells), let `ReserveCallFrame` lay the reserved
area out in the m16 prologue, and re-check the readers of the old
`sub`/`add` (`cfi.rs`, `framefree`, `call_pops`) and the m16 byte and clock
rows. Pushes made at m32 selection time (DEVIATE CG6 of row 50,
`pushfold.rs`) are not this pass's input.

N/A: the Darwin and Win64 conditions of `isLegal`, the stack probe, the
64-bit forms (`PUSH64`, `INSERT_SUBREG`) and `BuildCFI` (LIR has no CFI
instruction). DEVIATE CF2 (no number yet): `hasVarSizedObjects` is not in
LIR; the pass is given it (false until row 80). Owner llrm-target-x86-m16,
review 2026-11-20.

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
