---
architecture-index: the translation queue: old slots ranked by cost
architecture-order: 277
---
# The translation queue

The 52 `Old` slots of the parity registry, ranked by what replacing each saves on `d_faces` -O2. Middle-end rows are measured; Backend rows are not (the cost source covers only the function pipeline) and are listed after, unranked.

**Key** = own cost (M instructions) + old-analysis misses x 0.165. 0.165 M is the average cost the manager charged per miss to these slots (their inclusive minus own cost, 100.9 M, over 610 misses). Misses are deduplicated: `shape` counts once (its `loops` and `dominators`), `exposed-frames` once (its `registers`). Own and inclusive are LLRM_DEBUG=runs instructions summed over the 12 bodies and rounds (inclusive charges a shared analysis to its first asker); the whole compile is 1,611.1 M. Misses are the `ask <slot> <analysis>` lines of `LLRM_DEBUG=counts`. Owner is the port named in the registry; `none` in the LLVM column marks a delete or DEVIATE candidate.

## Function pipeline, ranked

| # | Slot | Owner | LLVM counterpart | Own M | Incl. M | Misses | Key |
|---|---|---|---|---|---|---|---|
| 1 | `gvn@787` | gvn-port | [GVN.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp) | 37.5 | 37.5 | 0 | 37.5 |
| 2 | `decide@1178` | simplifycfg-port | [SimplifyCFG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp) | 19.8 | 25.6 | 91 | 34.9 |
| 3 | `decide@675` | simplifycfg-port | [SimplifyCFG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp), [JumpThreading.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp), [CorrelatedValuePropagation.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp) | 23.1 | 27.3 | 55 | 32.2 |
| 4 | `decide@1244` | simplifycfg-port | [SimplifyCFG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp) | 23.8 | 26.2 | 47 | 31.6 |
| 5 | `decide@809` | simplifycfg-port | [JumpThreading.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp), [CorrelatedValuePropagation.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp) | 23.9 | 26.8 | 46 | 31.5 |
| 6 | `fold@801` | instcombine-port | [InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp) | 12.1 | 31.1 | 109 | 30.2 |
| 7 | `decide@766` | simplifycfg-port | [SimplifyCFG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp) | 22.7 | 25.7 | 40 | 29.3 |
| 8 | `decide@832` | simplifycfg-port | [SimplifyCFG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp) | 23.2 | 23.4 | 20 | 26.5 |
| 9 | `decide@690` | simplifycfg-port | [SimplifyCFG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp) | 22.1 | 23.7 | 21 | 25.6 |
| 10 | `indvars@738` | scev-port | [IndVarSimplify.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp) | 21.4 | 24.3 | 18 | 24.4 |
| 11 | `algebraic@696` | reassociate-port | [Reassociate.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp) | 11.7 | 16.6 | 26 | 16.0 |
| 12 | `fold@837` | instcombine-port | [InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp) | 11.8 | 19.1 | 6 | 12.8 |
| 13 | `fold@677` | instcombine-port | [InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp) | 1.9 | 21.4 | 52 | 10.5 |
| 14 | `fold@1242` | instcombine-port | [InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp) | 2.1 | 19.5 | 50 | 10.3 |
| 15 | `fold@768` | instcombine-port | [InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp) | 1.8 | 11.0 | 21 | 5.3 |
| 16 | `lcssa@823` | lcssa-port | [LCSSA.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LCSSA.cpp) | 3.1 | 3.1 | 0 | 3.1 |
| 17 | `lcssa@764` | lcssa-port | [LCSSA.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LCSSA.cpp) | 3.1 | 3.1 | 0 | 3.1 |
| 18 | `lcssa@772` | lcssa-port | [LCSSA.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LCSSA.cpp) | 3.0 | 3.0 | 0 | 3.0 |
| 19 | `loopidioms@737` | loop-idiom-port | [LoopIdiomRecognize.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp) | 0.6 | 0.6 | 4 | 1.3 |
| 20 | `dead@677` | instcombine-port | [InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp) | 1.0 | 1.0 | 0 | 1.0 |
| 21 | `dead@1242` | instcombine-port | [InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp) | 1.0 | 1.0 | 0 | 1.0 |
| 22 | `dead@801` | instcombine-port | [InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp) | 1.0 | 1.0 | 0 | 1.0 |
| 23 | `dead@837` | instcombine-port | [InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp) | 1.0 | 1.0 | 0 | 1.0 |
| 24 | `loopsimplify@772` | loopsimplify-port | [LoopSimplify.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopSimplify.cpp) | 0.7 | 0.8 | 2 | 1.0 |
| 25 | `dead@768` | instcombine-port | [InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp) | 1.0 | 1.0 | 0 | 1.0 |
| 26 | `loopsimplify@823` | loopsimplify-port | [LoopSimplify.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopSimplify.cpp) | 0.7 | 0.9 | 1 | 0.9 |
| 27 | `loopsimplify@764` | loopsimplify-port | [LoopSimplify.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopSimplify.cpp) | 0.7 | 0.7 | 0 | 0.7 |
| 28 | `floatloop@724` | licm-port | [LICM.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp) | 0.0 | 0.4 | 1 | 0.2 |
| 29 | `inferspace@787` | unowned | [InferAddressSpaces.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/InferAddressSpaces.cpp) | 0.1 | 0.1 | 0 | 0.1 |
| 30 | `tailrec@688` | tailcallelim-port | [TailRecursionElimination.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp) | 0.1 | 0.1 | 0 | 0.1 |
| 31 | `phiopt@832` | simplifycfg-port | [SimplifyCFG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp) | 0.0 | 0.0 | 0 | 0.0 |
| 32 | `phiopt@766` | simplifycfg-port | [SimplifyCFG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp) | 0.0 | 0.0 | 0 | 0.0 |
| 33 | `phiopt@690` | simplifycfg-port | [SimplifyCFG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp) | 0.0 | 0.0 | 0 | 0.0 |
| 34 | `phiopt@675` | simplifycfg-port | [SimplifyCFG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp) | 0.0 | 0.0 | 0 | 0.0 |
| 35 | `phiopt@1244` | simplifycfg-port | [SimplifyCFG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp) | 0.0 | 0.0 | 0 | 0.0 |
| 36 | `phiopt@1178` | simplifycfg-port | [SimplifyCFG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp) | 0.0 | 0.0 | 0 | 0.0 |
| 37 | `loopsimplify@728` | loopsimplify-port | [LoopSimplify.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopSimplify.cpp) | 0.0 | 0.0 | 0 | 0.0 |
| 38 | `lcssa@728` | lcssa-port | [LCSSA.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LCSSA.cpp) | 0.0 | 0.0 | 0 | 0.0 |

## Backend, not measured

| Slot | Owner | LLVM counterpart |
|---|---|---|
| `twoaddr@9` | lir-cleanup-poc | two-address-instruction |
| `ssaspill@3` | unowned | none (spiller route) |
| `schedule@16` | unowned | post-RA scheduler |
| `prologue@13` | unowned | prologepilog |
| `pressuresink@5` | lir-cleanup-poc | machine-sink |
| `phielim@4` | lir-cleanup-poc | phi-node-elimination |
| `phiconst@2` | unowned | none (shared phi constants) |
| `peephole@14` | unowned | peephole-opt, machine-cse, machine-dce |
| `parcopy@12` | unowned | phi-node-elimination copies, virtregrewriter |
| `loopslots@15` | regalloc-core-poc | greedy region splitting (SplitKit) |
| `floatfold@6` | unowned | x86 isel float folding (target) |
| `floatassign@7` | regalloc-core-poc | x87 stack slots (regalloc, target) |
| `farcall@1` | unowned | x86 far-call lowering (target) |
| `coalesce@10` | lir-cleanup-poc | register-coalescer |

Backend cost needs the machine phases of `LLRM_DEBUG=time`, which the per-slot table does not cover.
