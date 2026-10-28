# Plan: three codegen rows (funcattrs-port)

Rows of docs/working/codegen-slot-table.md, each translated from the pin (d1106deb71cc) and landed when it builds and passes the runner's gate; size, clock and compile effects are numbered in the PR, never blocking.

## 8. Canonicalize Freeze Instructions in Loops (land 11-04)
[`CanonicalizeFreezeInLoops.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/CanonicalizeFreezeInLoops.cpp): `Impl::run` over each loop in LoopSimplify form: for a header phi that `is_induction_phi` (ivdescriptors.rs) with an add/sub/mul step whose step value is outside the loop, a freeze of the phi or the step instruction is pushed to the preheader (`InsertFreezeAndForgetFromSCEV`), the step's poison flags dropped, the old freezes removed.
- Have: SCEV session and `forget_value`, `is_induction_phi`, `is_guaranteed_not_to_be_undef_or_poison`, `local::drop_poison_generating_flags`, `loopsimplify`.
- Pass `canonfreeze.rs`, FunctionPass, registry slot `canonfreeze` Backend-prep, run before `lsr` as in the pin's codegen prepare (`-disable-lsr` and its canon-freeze run together).
- Tests: the pass's own comment example (before/after MIR), a nsw step that loses its flag, a step value defined in the loop that is skipped, an already-guaranteed value left alone (premise assertion).
- Ablation: 0 of 77 IRs change (no frontend emits freeze): the row lands as MATCH with the numbers; the effect is for later IR with freeze.

## 55. X86 DynAlloca Expander (land 11-04)
[`X86DynAllocaExpander.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86DynAllocaExpander.cpp) expands the DYN_ALLOCA pseudo: it chooses per allocation `TouchAndSub`, `Sub` or `Probe` from how much stack is known safe (`computeLowerings`, `getLowering`), then emits `sub esp` with or without probes. The pseudo exists only where ISel lowers a dynamic alloca under stack probing (`probe-stack`, Windows).
- Blocked on the HIR op: no frontend or HIR node makes a dynamic alloca (builtins row B2, `alloca`). Step 1: the HIR/MIR op (dynamic `alloca` with a runtime count) and its isel lowering to `sub sp, n` (with the 16-bit and 32-bit stack alignment the convention states); step 2: the expander as an LIR phase before `PhiElimination`, with the pin's three strategies and `stack-probe-size` 4096.
- Tests: fixed-size and variable-size allocations through the whole backend, the interpreter agreeing; a probe above the page size (premise assertion on the emitted touch).
- Land: step 1 by 11-04 with the row N/A-with-HIR-op replaced by the op's evidence; step 2 follows as the same row's second PR.

## 81. Machine Late Instructions Cleanup (land 11-06)
[`MachineLateInstrsCleanup.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineLateInstrsCleanup.cpp) (268 lines): after prologue/epilogue insertion, removes an identical immediate or address load to a register that an earlier identical load in a dominating path left unchanged (`processBlock`, `RegDefs`, `RegKills`, `removeRedundantDef`, `clearKillsForDef`), over a reverse post-order.
- Place: an LIR phase after `Prologue` (flow.rs `machine_with`), reusing `liveness` lanes for the register-written test; slot `latecleanup@13`.
- Tests: the same `mov r, imm` twice in a block and across a dominating edge removed; a def between them or a call clobbering the register keeps it; a load with a different immediate kept.
- Ablation (premise): 13 and 12 of 77 IRs change in the pin (-O2/-Os); our effect numbers in the PR.
