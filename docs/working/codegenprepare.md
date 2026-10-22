# CodeGenPrepare: the pin's transforms against ours

The largest codegen gap: with the pin's pass off (`llc -disable-cgp`, 77 IRs: 12 bench programs at m32, 65 QCport modules at m16, `default<O2>` IR) the code grows by **+4456 instructions / +21337 B (5.6%) at -O2** and +4061 / +19398 B at -Os (`docs/working/codegen-ablation.md`, funcattrs-port's #3628). Pin: `CodeGenPrepare.cpp` at d1106deb71cc. Owner: w-4. Plan note 2026-11-02; first code PR 2026-11-06.

## What the pass does on these inputs

`print-before/after=codegenprepare` over the same 77 IRs (`~/scratch/w4-cgp/cgpclass.py`, one row per new value name): net IR +607 instructions and **-88 blocks**; new values `%sunkaddr` **881**, unnamed 105 (compare copies, casts), `%promoted` 7, `%math` 1, `%ov` 1; removed `%scevgep` 42. So the gap is address-mode sinking (`optimizeMemoryInst`) first, then empty-block merging and compare copies. The pin's own switches explain none of it (`-disable-cgp-branch-opts`, `-delete-phis`, `-store-extract`, `-complex-addr-modes` +0 B, `-disable-cgp-select2branch` +4 B, `-disable-cgp-ext-ld-promotion` -180 B), so the share of each transform was measured by patching a scratch copy of the pin's `llc` (the assert build's objects, `CodeGenPrepare.cpp` copied with `getenv("CGP_OFF")` returns at the top of `optimizeMemoryInst`, `eliminateMostlyEmptyBlocks`, `eliminateFallThrough`, `sinkCmpExpression`, `optimizeCmp`, `dupRetToEnableTailCallOpts`, `optimizeSelectInst`; `~/scratch/w4-cgp/abl.py`, same 77 IRs; the patched llc with nothing off reproduces 370340 / 312407 B and `-disable-cgp` +21337 / +19398 B):

| transform off | -O2 bytes | -Os bytes | IRs that differ (-O2) |
|---|--:|--:|--:|
| optimizeMemoryInst (address-mode sinking) | **+20712 (+5.59%)** | **+20053 (+6.42%)** | 62 |
| sinkCmpExpression | +468 | +556 | 37 |
| optimizeCmp (all its folds) | +469 | +557 | 37 |
| optimizeSelectInst | +4 | 0 | 2 |
| dupRetToEnableTailCallOpts | -2 | +1 | 2 |
| eliminateMostlyEmptyBlocks | -302 | -222 | 41 |
| eliminateFallThrough | 0 | 0 | 0 |
| all of the above off | +21548 | +19889 | 63 |
| `-disable-cgp` (the whole pass) | +21337 | +19398 | 63 |

Address-mode sinking is 97% of the pass's worth at -O2 (the parts sum to a little over the whole because merging empty blocks is a loss: the pass is smaller without it). Compare sinking is 0.13% and ours already has it (`addresssink.rs`); nothing else is worth a PR before the sinking.

## The premise, tested on our backend (2026-10-23)

The 21 KB is the pin's `llc` without its own pass; the question for us is whether our backend loses the same. Same IR for both: our mid-end's output at the switchconv stage of the 65 QCport modules (m16), compiled by our backend and by the pin's `llc` with and without the pass (`~/scratch/w4-cgp/leacmp.py`, instructions and `lea` counted in the assembly):

| -Os | instructions | `lea` |
|---|--:|--:|
| ours | 77032 | 2092 |
| pin `llc`, pass on | 74952 | 2139 |
| pin `llc`, `-disable-cgp` | 78643 | 3983 |

| -O2 | instructions | `lea` |
|---|--:|--:|
| ours | 93856 | 2682 |
| pin `llc`, pass on | 88311 | 2598 |
| pin `llc`, `-disable-cgp` | 92783 | 4835 |

Our address folding matches the pin's with the pass on (about half the `lea` of the pin without it, 2092 against 2139 and 3983): our isel already rebuilds an address expression in the block that uses it (a probe: an address computed in the entry block and used in two later blocks is `[eax+ecx*4+8]` in both). So **address-mode sinking is not a gap of ours**, and the pass's 21 KB is not ours to translate. The instruction gap that remains (2.7% at -Os, 6.3% at -O2, with `lea` equal) is something else; it has no CodeGenPrepare transform behind it.

## Table (the pin's order in `optimizeInst`)

| transform | pin | status | ours | pin switch | switch off, -O2 | notes |
|---|---|---|---|---|---|---|
| eliminateAssumptions | [811](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L811) | N/A | llvm.assume is dropped in isel | - | - | no assume survives to isel here |
| eliminateFallThrough | [867](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L867) | not examined | block placement after isel | - | - |  |
| eliminateMostlyEmptyBlocks | [939](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L939) | partial | SimplifyCFG merges empty blocks on its own gates; CGP's rule (no phi conflict, `isMergingEmptyBlockProfitable`) is not translated | -disable-cgp-branch-opts | +0 B | census: 88 blocks net removed over 77 IRs; off: -302 B (a loss in the pin) |
| splitBranchCondition | [9350](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L9350) | missing | - | -disable-cgp-branch-opts | +0 B | `(a && b)` branch split into two branches, only when profitable |
| SinkCast / OptimizeNoopCopyExpression / optimizeBitCast | [1432](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L1432) | missing | - | - | - | casts of values used in other blocks |
| sinkCmpExpression (in optimizeCmp) | [1924](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L1924) | same (x86: one condition register) | `addresssink.rs` re-creates a compare in each user block | - | - | census: ~105 unnamed new values include these |
| combineToUAddWithOverflow / combineToUSubWithOverflow | [1747](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L1747) | missing | - | - | - | census: `ov`/`math` 2 sites in 77 IRs |
| replaceMathCmpWithIntrinsic | [1634](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L1634) | missing | - | - | - |  |
| unfoldPowerOf2Test / foldICmpWithDominatingICmp / swapICmpOperands / foldFCmpToFPClassTest | [1853](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L1853) | missing | - | - | - |  |
| optimizeURem / foldURemOfLoopIncrement | [2227](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L2227) | missing | - | - | - |  |
| OptimizeExtractBits | [2526](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L2526) | missing | - | - | - | x86 BEXTR is not on a 486 |
| despeculateCountZeros | [2629](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L2629) | missing | - | - | - | needs `ctlz`/`cttz` cheap-zero TTI |
| optimizeCallInst | [2707](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L2707) | not examined | - | - | - |  |
| dupRetToEnableTailCallOpts | [3014](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L3014) | missing | - | - | - | tail calls through a return block |
| optimizeMemoryInst (address mode sinking, AddressingModeMatcher) | [5903](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L5903) | missing: the largest piece | `addresssink.rs` merges address phis only; no sinking of address arithmetic into the user blocks | -disable-complex-addr-modes | +0 B | census: 881 `sunkaddr` values over 77 IRs; off: **+20712 B** (-O2), +20053 B (-Os) |
| optimizeGatherScatterInst / vector | [6332](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L6332) | N/A | no vectors on these targets | - | - |  |
| optimizeExt / tryToPromoteExts / optimizeExtUses / optimizeLoadExt | [7250](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7250) | missing | - | -disable-cgp-ext-ld-promotion | -180 B (off is smaller) | census: `promoted` 7 sites |
| mergeSExts | [6828](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L6828) | missing | - | - | - |  |
| splitLargeGEPOffsets | [6902](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L6902) | missing | - | - | - | needs `-cgp-split-large-offset-gep` |
| optimizePhiTypes | [7168](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7168) | missing | - | - | - | `-cgp-optimize-phi-types` |
| optimizeShiftInst / optimizeFunnelShift | [7696](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7696) | missing | - | - | - |  |
| optimizeSelectInst | [7766](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7766) | partial | `backend/selects.rs` decides branch or select at isel (opposite direction on in-order i486) | -disable-cgp-select2branch | +4 B (2 IRs) |  |
| tryToSinkFreeOperands | [7994](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7994) | N/A | x86 has no free shifted operands | - | - |  |
| optimizeSwitchInst / optimizeSwitchType / optimizeSwitchPhiConstants | [8192](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L8192) | partial | `switchlower.rs` lowers; the type widening and phi-constant rewrites are not translated | - | - |  |
| splitMergedValStore | [8608](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L8608) | missing | - | - | - |  |
| optimizeBranch (x&m)==0 and friends | [8878](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L8878) | missing | - | -disable-cgp-branch-opts | +0 B |  |
| makeBitReverse | [9139](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L9139) | missing | - | - | - |  |

## Plan

None for this pass on the evidence above: no transform is worth a PR before the tuning phase. Open: where the remaining 2.7% / 6.3% of instructions against the pin on the same IR comes from (`leacmp.py` is the instrument; the rest of the pin's `llc` passes, per `docs/working/codegen-ablation.md`).
