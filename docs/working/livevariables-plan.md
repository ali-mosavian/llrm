# LiveVariables: the translation TwoAddress reads (w-1, 2026-10-24)

Pin d1106deb71cc. TwoAddress's kill questions (`isPlainlyKilled`, `findKill`, `removeKill`) are answered by the pin's `LiveVariables`, built in SSA before PHIElimination, which maintains it. Ours answers them from lazy `LiveIntervals` (`BodyFacts`): 6.1 M of TwoAddress's 11.73 M Ir on d_faces -O2 (the plan in [intervals-once](intervals-once.md)). The ruling (coordinator, 2026-10-24): translate it as it is, in the pin's order; no analysis of ours beside it.

## Pin functions and ours
New module `liveness/livevariables.rs`; `VarInfo` is `AliveBlocks` (a bit per block) and `Kills` (the killing instruction's id).

| Pin | Line | Ours |
|---|---|---|
| `getVarInfo` | [#L114](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L114) | `LiveVariables::get_var_info` |
| `MarkVirtRegAliveInBlock` (both) | [#L120](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L120), [#L145](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L145) | same names |
| `HandleVirtRegUse` / `HandleVirtRegDef` | [#L157](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L157), [#L208](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L208) | same names |
| `runOnInstr`, `runOnBlock`, `analyze` (virtual registers) | [#L477](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L477), [#L540](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L540), [#L592](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L592) | `run_on_instr`, `run_on_block`, `analyze` |
| `analyzePHINodes` | [#L760](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L760) | `analyze_phi_nodes` (a block's `phis`, not instructions) |
| `VarInfo::isLiveIn`, `isLiveOut` | [#L772](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L772), [#L789](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L789) | same |
| `addVirtualRegisterKilled`/`Dead`, `removeVirtualRegisterKilled`/`Dead`, `VarInfo::findKill`/`removeKill` | header | same |
| `replaceKillInstruction`, `removeVirtualRegistersKilled`, `recomputeForSingleDefVirtReg` | [#L734](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L734), [#L742](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L742), [#L648](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L648) | same |
| `addNewBlock` | [#L814](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp#L814) | for an edge split (`edges::split`) |
| physical-register half (`HandlePhysReg*`, `UpdatePhysRegDefs`, `HandleRegMask`) | #L217-#L476 | DEVIATE LV1: absent; LIR carries `requires`/`delivers`/`clobbers`, which TwoAddress reads off the instruction (`processCopy`) |

DEVIATE LV2: a kill is an instruction id (`Insn::id`) with a block lookup, as the pin's is a pointer with `getParent`.

## Who calls it
- PHIElimination ([#L258](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L258), [#L440](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L440)-[#L489](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L489), [#L607](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L607)-[#L709](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L709)): the kills of the phi's sources move to the copies it inserts. Ours: `phielim.rs`.
- TwoAddress ([#L956](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L956), [#L1085](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1085), [#L1144](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1144), [#L1265](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1265), [#L1488](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1488)-[#L1509](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1509), [#L1740](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1740), [#L1828](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1828)-[#L1872](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1872), [#L2107](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L2107)): `BodyFacts` becomes these calls; `rewrite`, `replace`, `move_before` become the kill updates above. `handleMove` stays the intervals' (the pin keeps LiveIntervals current there only if present).

## Steps
| Date | PR |
|---|---|
| 2026-11-06 | this note |
| 2026-11-13 | PR1: `livevariables.rs` (analyze, the update API; no consumer), tests against hand-made SSA bodies and the pin's example |
| 2026-11-20 | PR2': a phase before TwoAddress fills it by the dataflow (LV3); TwoAddress reads it; `BodyFacts` loses `LiveIntervals` and `finish` stops handing intervals over (the coalescer builds its own until the eager build of intervals-once step 3); ledger Ir and the 568 objects, unchanged |

If PR1 or PR2 cannot make 2026-11-24 by 2026-11-06: the interim is a liveness dataflow after phielim behind the same API, numbered DEVIATE LV3, exit date written then.

## Why the interim (DEVIATE LV3), and the pass order
The pin has PHIElimination directly before TwoAddress and runs its x86 pre-RA passes (`X86PassConfig::addPreRegAlloc`) before PHIElimination, on SSA. Ours runs eight passes between them, each rewriting the body, so a `LiveVariables` built before phielim is stale at TwoAddress unless all eight maintain it. The order is a pin-faithfulness row of its own:

| Ours (between phielim and twoaddr) | Pin slot | Moves before phielim |
|---|---|---|
| `pressuresink` | none (the scheduler orders loads) | gone with the pre-RA machine scheduler (w-2, row 65), 2026-11-20 at the earliest |
| `fixupsetcc` | `addPreRegAlloc`, before PHIE | blocked: its F1 defines a value twice and the SSA check refuses (`value#4 is defined 2 times`); needs sub-register values in LIR, the C1 lanes of `row63-66-plan.md`. Dates (coordinator, 2026-10-24): parity green 2026-10-27; C1 lanes plan 2026-11-06 and first PR 2026-11-17 (splitkit-port); the `fixupsetcc` move 2026-11-20 (w-1) |
| `callframe` | `addPreRegAlloc`, before PHIE | done (#4275) |
| `flagscopy` | `addPreRegAlloc` (`X86FlagsCopyLowering`), before PHIE | done (this PR) |
| `floatfold` | none (folded at selection) | now if it reads no phi copy; else with the selector's folding |
| `pushfold` | inside `X86CallFrameOptimization`, before PHIE | done (this PR), right after `callframe` |
| `FloatPrep`, `FloatAlloc` | `X86FPStackifier`, after register allocation | with machine-cleanup row 9 (w-2 step 9, code 2026-11-20) |

DEVIATE LV3 (owner w-1): until those are out of the gap, `LiveVariables` is filled right before TwoAddress by a liveness dataflow on the non-SSA body (`alive_blocks` = live in and live out; a kill = the last use in a block where the value is not live out; a dead definition = its definition), through the same API. Exit: phielim sits directly before twoaddr, builds it before lowering the phis and maintains it (the pin's `PHIElimination` calls); 2026-12-11 at the latest, and the PR that removes the dataflow is dated with it.
