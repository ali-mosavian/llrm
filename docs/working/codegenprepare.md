# CodeGenPrepare: the pin's transforms against ours

The largest codegen gap: with the pin's pass off (`llc -disable-cgp`, 77 IRs: 12 bench programs at m32, 65 QCport modules at m16, `default<O2>` IR) the code grows by **+4456 instructions / +21337 B (5.6%) at -O2** and +4061 / +19398 B at -Os (`docs/working/codegen-ablation.md`, funcattrs-port's #3628). Pin: `CodeGenPrepare.cpp` at d1106deb71cc. Owner: w-4. Plan note 2026-11-02; first code PR 2026-11-06.

## What the pass does on these inputs

<<<<<<< HEAD
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
=======
`print-before/after=codegenprepare` over the same 77 IRs (`~/scratch/w4-cgp/cgpclass.py`, one row per new value name): net IR +607 instructions and **-88 blocks**; new values `%sunkaddr` **881**, unnamed 105 (compare copies, casts), `%promoted` 7, `%math` 1, `%ov` 1; removed `%scevgep` 42. So the gap is address-mode sinking (`optimizeMemoryInst`) first, then empty-block merging and compare copies. The pin's own switches explain none of it: `-disable-cgp-branch-opts`, `-delete-phis`, `-store-extract`, `-complex-addr-modes` are +0 B, `-disable-cgp-select2branch` +4 B, `-disable-cgp-ext-ld-promotion` -180 B (off is smaller). The sinking cannot be switched off alone, so its share of the 21337 B is not measured separately; the census says it dominates.
>>>>>>> origin/feat/parity

## The whole effect, per function (2026-10-23)

Oversight's point: a `lea` count covers address folding only. Per function, on the same IR (`~/scratch/w4-cgp/perfn.py`: instructions of ours and of the pin's `llc` with the pass, with `-disable-cgp`, and with each patched sub-feature off), the table gives per corpus: ours / pin with the pass / pin without it; the functions where the pass removes instructions in the pin (ours / with / without); the functions where ours has more than the pin with the pass (the excess) and how much of it lies in functions where the pin's address-mode sinking is worth that much (capped per function); and the functions where ours has fewer (the deficit).

| corpus | functions | all: ours / pin+CGP / pin-CGP | functions CGP changes: ours / with / without | excess over pin+CGP | deficit |
|---|--:|---|---|---|--:|
| m32, 71 programs, -Os (like-for-like) | 161 | 4912 / 5048 / 5119 | 16: 1287 / 1271 / 1343 | 41 fn, excess 151, of which the pin's address effect covers 20 | 287 |
| m32, 71 programs, -O2 | 158 | 9247 / 8414 / 8573 | 17: 3540 / 3317 / 3489 | 64 fn, excess 1020, of which the pin's address effect covers 76 | 187 |
| m16, 65 QCport modules, -Os (trend) | 701 | 76457 / 74952 / 78643 | 296: 56789 / 55574 / 59317 | 366 fn, excess 4132, of which the pin's address effect covers 1544 | 2627 |
| m16, 65 QCport modules, -O2 | 575 | 93233 / 88311 / 92783 | 299: 76419 / 72296 / 76805 | 357 fn, excess 7051, of which the pin's address effect covers 2218 | 2129 |

Reading it. At **m32, like-for-like**, CGP changes 16-17 of 160 functions in the pin; there ours is at the pin's level with the pass at -Os (1287 against 1271 and 1343 without) and at the pin's level without it at -O2 (3540 against 3317 and 3489): the whole effect is small there, and the -O2 excess of the corpus (1020) is the multiply chains (`docs/working/instruction-gap-vs-pin.md`, DEVIATE M1), with 76 of it in functions the address sinking touches. At **m16** the pass changes 296 of 701 functions at -Os; ours is between the two (56789 against 55574 and 59317) and 1544 of the 4132 instructions of excess sit in functions where the pin's address sinking is worth that much (37%), against a deficit of 2627 elsewhere. The m16 comparison is a trend: the pin's code16 addresses with 32-bit scaled registers; the 8086 forms (`[bx|bp]+[si|di]+disp`, no scale) limit what sinking can give us, so the 1544 is an upper bound on what a sinking pass could take.

Top 20 functions by (ours - pin with CGP), m16 -Os; the last column is the pin's own gain from each sub-feature in that function (instructions):

| module | function | ours | pin+CGP | pin-CGP | pin sub-feature value (off minus on) |
|---|---|--:|--:|--:|---|
| host.c | _host_console_result@3 | 963 | 717 | 719 | addr 1 |
| ent.c | _ent_load_world@3 | 1388 | 1247 | 1372 | addr 134 |
| triggers.c | _ent_move_trigs@3 | 982 | 850 | 998 | addr 147 |
| sc.c | _sc_selftest@3 | 2006 | 1877 | 1860 | addr 31, empty -37 |
| dl.c | _dl_selftest@3 | 595 | 518 | 557 | addr 39 |
| savegame.c | _savegame_load@3 | 1081 | 1005 | 1053 | addr 46, empty 7 |
| weapons.c | _pl_fire@3 | 1356 | 1285 | 1360 | addr 75 |
| d_faces.c | _clip_w@3 | 228 | 162 | 164 | empty 2 |
| doors.c | _ent_move_doors@3 | 542 | 476 | 528 | addr 51 |
| weapons.c | _pl_select_weapon@3 | 176 | 112 | 130 | addr 17 |
| sc.c | _sc_alloc@3 | 491 | 429 | 452 | addr 23 |
| mod.c | _mod_unload@3 | 302 | 242 | 242 | none |
| d_faces.c | _d_draw_faces@3 | 1328 | 1275 | 1384 | addr 114 |
| item.c | _item_take_weapon@3 | 108 | 57 | 60 | none |
| mdl_ai.c | _mdl_tick@3 | 155 | 104 | 117 | addr 13 |
| triggers.c | _ent_use_targets@3 | 256 | 208 | 238 | addr 30 |
| d_alias.c | _d_mdl_visible@3 | 343 | 298 | 310 | addr 17, empty -4 |
| pl_trace.c | _pl_trace@3 | 233 | 189 | 268 | addr 79 |
| savegame.c | _savegame_save@3 | 537 | 494 | 502 | addr 8 |
| mod_tex.c | _mod_load_textures@3 | 442 | 400 | 410 | addr 11 |

Where the sub-feature column says `addr`, the pin's address sinking explains up to that many of the function's excess; `none` or a small number means the excess is something else (copies, far pointers: `docs/working/instruction-gap-vs-pin.md`).

Decision (open, to be confirmed): **m32** is matched, so for m32 the slot is a numbered DEVIATE C1 "CodeGenPrepare's work is done in isel (address forms rebuilt in the user's block), `addresssink.rs` (compares, address phis) and the select/switch lowering"; owner w-4, review 2026-12-01. **m16**: a row C2 with the 1544-instruction upper bound; the next step is an experiment (sink where the 8086 form is legal) before any PR, because the pin cannot say what the 8086 form is worth.

## Table (the pin's order in `optimizeInst`)

| transform | pin | status | ours | pin switch | switch off, -O2 | notes |
|---|---|---|---|---|---|---|
| eliminateAssumptions | [811](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L811) | N/A | llvm.assume is dropped in isel | - | - | no assume survives to isel here |
| eliminateFallThrough | [867](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L867) | not examined | block placement after isel | - | - |  |
<<<<<<< HEAD
| eliminateMostlyEmptyBlocks | [939](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L939) | partial | SimplifyCFG merges empty blocks on its own gates; CGP's rule (no phi conflict, `isMergingEmptyBlockProfitable`) is not translated | -disable-cgp-branch-opts | +0 B | census: 88 blocks net removed over 77 IRs; off: -302 B (a loss in the pin) |
=======
| eliminateMostlyEmptyBlocks | [939](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L939) | partial | SimplifyCFG merges empty blocks on its own gates; CGP's rule (no phi conflict, `isMergingEmptyBlockProfitable`) is not translated | -disable-cgp-branch-opts | +0 B | census: 88 blocks net removed over 77 IRs |
>>>>>>> origin/feat/parity
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
<<<<<<< HEAD
| optimizeMemoryInst (address mode sinking, AddressingModeMatcher) | [5903](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L5903) | missing: the largest piece | `addresssink.rs` merges address phis only; no sinking of address arithmetic into the user blocks | -disable-complex-addr-modes | +0 B | census: 881 `sunkaddr` values over 77 IRs; off: **+20712 B** (-O2), +20053 B (-Os) |
=======
| optimizeMemoryInst (address mode sinking, AddressingModeMatcher) | [5903](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L5903) | missing: the largest piece | `addresssink.rs` merges address phis only; no sinking of address arithmetic into the user blocks | -disable-complex-addr-modes | +0 B | census: 881 `sunkaddr` values over 77 IRs; whole-pass off +21337 B (-O2) |
>>>>>>> origin/feat/parity
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

<<<<<<< HEAD
## Plan

1. Confirm C1 (m32) and C2 (m16) with the coordinator; the DEVIATE text goes into `addresssink.rs`'s header and the codegen table.
2. C2 experiment: address sinking restricted to the legal 8086 forms on a few of the top-20 functions, measured in bytes at -Os; a PR only if it pays.
3. The rest of the table: unchanged (below 0.15%).
