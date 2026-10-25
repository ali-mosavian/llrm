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

## Update 2026-10-25: the DAG path is the producer (owner sdag-d, plan 11-06, first PR 11-13)
The DAG emitter lowers `ADJCALLSTACKDOWN/UP` to a per-call `sub esp, N` / `add esp, N` with `mov [esp+k], x` stores between (the pin's input form, minus the pseudos and the reserved frame). `emit.rs::pushed_dwords` (DEVIATE CF3) turns every tiling area into pushes at m32, which took QCport m32 from +10.7..+18.9% to -0.7..+1.9% over the old selector (#4566). The pin instead keeps the frame reserved (one `sub esp, MaxCallFrameSize` in the prologue, no per-call adjust) and lets this pass push only where `isProfitable`.

Pieces, one PR each, rows priced after each (AM-BT 30, `qcport.py` m32 -O2/-Os, `run_tests.py c`):
1. Emitter: `call_frame_setup N, 0, 0` / `call_frame_destroy N, M` instead of the `sub`/`add`; the stores stay. `callframe.rs` runs on them (it already has `isLegal`, `isProfitable`, `adjustCallSequence`). Until 2, every setup it leaves is lowered to the old `sub`/`add` right after it, so output equals today's minus the pushes the heuristic declines.
2. `eliminateCallFramePseudoInstr` + prologue: `hasReservedCallFrame` (no variable-sized objects) puts the largest call area in the prologue's `sub`; the setup/destroy pairs vanish; sp-relative frame cells keep their offsets (the area is at the bottom). `framelowering.rs` takes `max_call_frame`. Needs frame indices for cells (FI-1, `pei-plan.md`) or the sp-relative offsets of locals recomputed there.
3. Delete `pushed_dwords` (CF3) and `canFoldIntoRegPush`/`PUSH32rmm` fold (pieces 3-4 above) so `push [m]` returns where the pin has it.

Risks: sp-relative addressing of locals and incoming arguments inside a call sequence (the area moves from per-call to the prologue: offsets change); `-Os`/`-Oz` profitability (the pin pushes always at minsize); the 16-byte alignment padding of sysv (`sub esp, 4` before the pushes).

## Design 2026-10-26: the reserved frame (piece 2), no frame indices needed

Owner: w12. Pin: `hasReservedCallFrame` = no variable-sized objects and no push sequence (`CallFrameOpt` sets `hasPushSequences`, so one pushed call anywhere un-reserves the whole function); `calculateCallFrameInfo` = largest `ADJCALLSTACKDOWN` size; `eliminateCallFramePseudoInstr` drops the pair (a callee-popped `M` becomes `sub sp, M`); `isProfitable` is consulted only when a frame is reserved.

**Choice: add the largest area to the prologue's `sub`, not frame indices (FI-1) and not locals recomputed.** The area is the lowest bytes of the frame. Locals and spills are addressed from BP, or by masm as `[sp + depth - slot]` with `depth` counting the prologue's `sub`; a larger `sub` moves `sp` and `depth` together, so no cell's address changes. The `mov [sp+k]` stores are raw sp cells, unshifted. FI-1 stays its own item (row 80); this does not wait for it.

**Numbers** (QCport m32 -O2, 65 modules): 575 functions, 4206 calls; 677 call sequences keep a mid-body `sub esp`/`add esp` pair after PR 1; 345 of them (57 functions, no pushed call) are removed by a reserved frame, about 2 KB (0.6%) at ~6 B a pair (`isProfitable`'s figure); the other functions push somewhere and keep their pairs, as the pin does. Turning `has_var_sized` to the pin's `false` alone (profitability on, pairs still per call) already gives -O1 -0.7%, -O2 -0.88%, -O3 -0.7%, -Os -0.28% against always-push (341,031 to 338,040 B at -O2); measured with the pairs still in, so the reservation adds to it.

**PRs**, each priced on QCport m32 and `run_tests.py c`:
1. `has_var_sized` false where the function has no variable-sized object (CF4 stays as the lowering of what is left). Test: a function the heuristic declines keeps `sub`/`mov`/`add`.
2. `ReserveCallFrame` replaces `LowerCallFrame` and runs in `Prologue`, in functions with no variable-sized object and no pushed call (the rest lower to `sub`/`add` as now) (the pseudos live through allocation, as the pin's): max of `N - A` over setups added to the frame size, pairs removed, callee-popped `M` becomes `sub sp, M`. `framefree::pinned_by` then needs no pseudo arm. Variable-sized objects keep the per-call `sub`/`add` and always push (`CannotReserveFrame`).
3. Delete CF3 leftovers (`LowerCallFrame`, DEVIATE CF4/CF5) and fold `push [m]` as the pin does (pieces 3-4 above).

Risks: allocation and the spiller seeing Barrier pseudos instead of `sub`/`add` (priced by the DOSBox C sweep); sysv 16-byte alignment padding of the area (`sub esp, 4`), which the pin folds into the reserved size.

Dates: PR 1 2026-10-27, PR 2 2026-11-13, PR 3 2026-11-17.

Status 2026-10-26: PRs 1 and 2 are #4634 and #4649; CF3 went in #4620. PR 3 needs no code: `LowerCallFrame` is the pin's own lowering for pushed and variable-sized functions, and `PushFold` is the `push [m]` fold. Open: DEVIATE CF5 (`framefree` keeps the frame register at -Os where a call frame exists), exit PEI-2 `emitPrologue` (`pei-plan.md`).

## DEVIATE CF6: stack alignment 4, the pin's Linux 16 (2026-10-25)
Our m32 target states a 4-byte stack (`S32`, `CALLING.native().stack_alignment` = the slot). The pin takes 16 from `i486-unknown-linux-gnu`. `isProfitable` charges a push call 3 bytes more per unaligned area, so at 16 a one-argument call keeps a reserved frame (`subl $12,%esp; movl $2000,(%esp)`) and at 4 it pushes (`llc -mtriple=i486-unknown-none-elf` gives ours exactly). A target choice (DOS/sysv32), not a defect; no exit. Comparisons now tell the pin 4: `pin.py` (module flag `override-stack-alignment` from the datalayout's `S`), clang `-mstack-alignment=4` and gcc `-mpreferred-stack-boundary=2` in the rows' reference builds (`vsgcc/build.sh`, `pinclocks.sh`).

Checklist for an instrument that runs the pin (the mistake was an unstated stack alignment; check each before quoting a number):
- Code or clocks against the pin's `llc`/`opt`: go through `tools/pairs/pin.py` (it states the target and the alignment); a direct `llc -mtriple=...-linux-gnu` is priced at 16.
- Code or clocks against pin `clang`/`gcc` objects: `-mstack-alignment=4` / `-mpreferred-stack-boundary=2` (`vsgcc/build.sh`, `pinclocks.sh`); stored reference objects (`main24d`) built before 2026-10-25 carry "reference built at stack alignment 16" until rebuilt.
- `llc -stop-after=finalize-isel` and the pin's `opt` oracles are alignment-neutral (before the call-frame pass; IR only).
- Compile time of clang (`compile-gap.py`, `clang_pass_budgets.py`, `pass_budgets.py`, `board.py`, `cbisect.py`): unchanged. Checked small: clang -O2 -fno-inline-functions on d_faces, user instructions 415.7 M at the default against 415.2 M with `-mstack-alignment=4` (-0.12%, perf stat, best of two).
