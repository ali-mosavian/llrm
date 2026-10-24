# Tail calls in the DAG path (sibling calls)

The pin turns a call in tail position into `jmp` (`TCRETURNdi` becomes `TAILJMPd`) at -O2; fib, hanoi and
recchop show it in the shadow. Neither selector does it today (`call g; ret`). Owner: sdag-a. Written
2026-10-24; steps dated below. The old selector is the default path and does not get it: this ships with the
DAG when the DAG becomes the default (`selectiondag-default-readiness.md`); until then only `--isel=dag` and
the shadow carry it.

## Steps, each a fail-first test by execution

| # | Step | Pin function | Date |
|---|---|---|---|
| 1 | Call in tail position: the call's value is the block's return, nothing between | [isInTailCallPosition](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/Analysis.cpp#L539), [SelectionDAGBuilder::canTailCall](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L9235) | 10-27 |
| 2 | Eligibility: same convention, no byval or sret mismatch, callee pops no more than the caller's argument area | [isEligibleForSiblingCallOpt](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLoweringCall.cpp#L2976), [LowerCall's isTailCall](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLoweringCall.cpp#L2160) | 10-30 |
| 3 | `X86ISD::TC_RETURN` node and its operands (callee, popped bytes, argument registers, no `callseq`) | [LowerCall TC_RETURN](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLoweringCall.cpp#L2712) | 11-02 |
| 4 | Matcher: `TCRETURNdi` / `TCRETURNri` (sdag-c's tables already have the patterns; check m16 has none) | [X86InstrCompiler.td](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrCompiler.td#L1393) | 11-04 |
| 5 | Emitter + frame: the epilogue goes before the jump | [emitEpilogue / isTailCallOpcode](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp#L2572), [X86ExpandPseudo](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ExpandPseudo.cpp#L281) | 11-06 (first PR) |
| 6 | m16 (far callee: `jmp far`, regparm3 pops) after m32 is green | same | 11-13 |

## Decisions

- m32 first; m16 after. At m16 a far call tail-jumps with the far return address already on the stack, so a callee
  that pops arguments differs from the caller's: step 2's check carries it.
- The LIR needs a return-like instruction that the frame pass gives an epilogue (`Operation::Return` with a
  symbol), not a plain `Jump`: step 5 settles it by reading `framelowering.rs`.
- Each step's test is an execution test (fib, hanoi, recchop) under `--isel=dag` against the old selector's output.
