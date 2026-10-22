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

## Plan, in the pin's order

1. **Address-mode sinking** (`optimizeMemoryInst`, `AddressingModeMatcher`): a GEP, add or shift used as an address in another block is rebuilt in the user's block as the x86 base + index*scale + displacement the target accepts (`isLegalAddressingMode`), so isel folds it. Needs the TTI's addressing-mode legality for m16 (BX/SI/DI, no scale) and m32. Premise test: 20712 B at -O2 and 20053 B at -Os over the 77 IRs when off; 881 sunk values. First code PR 2026-11-06.
2. Everything else in the table is below 0.15% on these inputs (compare folds +0.13%, select 0, dupRet 0): after the tuning phase, each priced first. Merging empty blocks is a loss in the pin (-302 B when off): not translated until a row says otherwise.

Rows, QCport and compile Ir with every PR; this file's table is updated by each.
