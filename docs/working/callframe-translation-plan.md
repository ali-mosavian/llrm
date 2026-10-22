# Translating X86CallFrameOptimization (slot 50)

Pin d1106deb71cc, [`X86CallFrameOptimization.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp). Plan only. Row 50 of [codegen-slot-table.md](codegen-slot-table.md); land 2026-11-02.

## The premise: a count, not a story
Ours pushes every stack argument at selection (`isel.rs`, the argument loop that emits `Operation::Push`), so the pin's input form (a reserved call frame filled by `mov [esp+k], x`) never exists in ours. What the pass leaves behind that ours lacks is the fold of a single-use load into the push (`canFoldIntoRegPush`, [L603](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L603)). `g(struct s *p) { return f(p->a, p->b, p->c); }`, -O2 i486:

| | instructions | pushes |
|---|---|---|
| pin `clang -target i486 -O2` | `mov eax,[esp+16]`, 3 x `push dword ptr [eax+k]`, `call`, `add esp` | memory operands |
| ours | 3 loads into registers, 3 x `push reg`, `call`, `add esp` | registers (two more live values, two more instructions) |

Pin ablation of the pass: changed code in 59 (-O2) and 60 (-Os) of 77 IRs (codegen-slot-table row 50). The pass's other effects (a reserved frame turned into pushes, the CFA adjusts) are already ours by construction.

## Pieces, in the pin's order of functions (one PR each, each priced on all rows and QCport, no tuning)
1. `runOnMachineFunction` ([L248](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L248)), `isLegal` ([L147](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L147)): per-function gate. n/a pieces: Darwin compact unwind, Win64, stack probes. Kept: the frames close inside one block and do not nest (ours: one call sequence per call, always closed).
2. `collectCallInfo` ([L364](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L364)), `classifyInstruction` ([L290](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L290)): the sequence of stack stores before the call. Ours has pushes already, so this reads the pushes of one call (the run of `push` between the last non-push that matters and the `call`) and the registers they use.
3. `canFoldIntoRegPush` ([L603](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L603)) and the `PUSH32rmm` branch of `adjustCallSequence` ([L502](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L502)): a pushed vreg with one non-debug use, defined by a 32-bit load in the same block, with no load-fold barrier (`isLoadFoldBarrier`: a store, call or side effect) between the load and the call frame setup, becomes `push mem`; the load goes. `slowTwoMemOps` is false on i486 and on the 8086/80286 targets (check the 16-bit `push word ptr`: m16 has a 16-bit `push m16`, same rule on `MOV16rm`).
4. `isProfitable` ([L205](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L205)): a reserved frame is not a thing here, so the heuristic is "always" (`hasVarSizedObjects` returns true at once); recorded as n/a with that reason.

Pushes of immediates (`push imm`, `pushd`) are ours already; `AND/OR mi` conversions (`and [esp+k], 0` as a push of 0) are a pin spelling of a constant store and do not arise.

## Where in ours
After selection, before allocation, on the LIR: the call's `Push` run and the loads that feed it (`isel.rs`, the stack-argument loop), as a pass next to `copysink`/`precse`. Legality is `mayStore`/`isCall`/volatile between the load and the first push, over `Operation` effects (`effects.rs`).

## Deviations to record
- D1: the input is pushes, not stores into a reserved frame. The pass is the fold only; exit: none (the form the pin starts from is not produced).

## Tests
Fail first: `g` above has no register load for the three pushed members; a load with a store between it and the call keeps its register; a vreg with two uses keeps its load; volatile and `lock` loads are not folded; the 16-bit target folds `push word ptr`.
