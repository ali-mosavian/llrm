# BasicAA as a translation of LLVM's: the plan

**TL;DR.** Today alias answers come from a whole-function tower (`alias.rs` 3,852 lines, `frameescape.rs` 703, `llrm-mir/alias.rs` 216, tests 1,640): points-to sets, a flow-sensitive escape phase and a join rule per case. Two soundness bugs in one day (#2132: a join lost an arm; #2125: edges not re-derived) are a class LLVM's per-query design does not have. The plan is to translate `AliasAnalysis.cpp`, `BasicAliasAnalysis.cpp`, `CaptureTracking.cpp` and `MemoryLocation.cpp` (pinned `d1106deb71cc`, about 4,100 C++ lines) behind the interface of [#2163](https://github.com/ali-mosavian/llrm/pull/2163), move the clients over one PR each, and delete the tower. Nothing below is measured yet unless a number is given.

## 1. Interface (written, #2163)

`crates/opt/llrm-analysis/src/aa.rs`: `AAResults` (`alias`, `get_mod_ref_info_mask`, `get_arg_mod_ref_info`, `get_memory_effects`, `get_mod_ref_info` with an optional location and the call form, `call_captures_before`; `is_no_alias`, `is_must_alias`, `points_to_constant_memory`, `does_not_access_memory`, `only_reads_memory` provided) and `BatchAAResults` owning one `AAQueryInfo` (cache, cross-iteration switch, dominator-tree switch). Names are LLVM's in snake case. One `AAQueryInfo` lives for one walker or MemDep run and dies with the body version.

## 2. Translation map

| LLVM | llrm today | stage |
|---|---|---|
| `getUnderlyingObject` (ValueTracking.cpp) | `object_of`, `MemRef::of` decomposition | valuetracking-port's API, used from S1 |
| `BasicAAResult::aliasCheck`, `aliasGEP`, `aliasPHI`, `aliasSelect` (each arm checked) | `Apart`, `regions::overlapping`, provenance sets with join rules | S2 |
| `CaptureTracking.cpp`: `PointerMayBeCaptured`, `isNonEscapingLocalObject`, `EarliestEscapeAnalysis` | `escapes`, `EscapedBefore`, `ExposedFrames` | S1; the independent check is `capturecheck.rs` (#2146) |
| `getModRefInfo(Call, Loc)`, `getArgModRefInfo`, `MemoryEffects` | `CallEffects`, `Summaries`, `calls_annotated` | S3, needs per-parameter `nocapture` from function-attrs (cgscc-poc) |
| `GlobalsModRef` | `GlobalsAA` | S4 |
| TBAA, scoped noalias | `MemRef.typed`, restrict roots | with S2 |
| `MemoryLocation::get`, `LocationSize` | `MemRef` width | S1 |

## 3. Oracles

1. **Captures** (done, #2146): every alloca's uses read directly; the points-to facts must list what is captured. On the gate corpus 1,583 objects, 0 differ; module report at -O2: 526 callee parameters verified in the callee's own body, 584 external, 370 captured by the body, 0 under-escapes. It caught #2132's class on main.
2. **LLVM's answers** (not run): `opt -passes=aa-eval -print-all-alias-modref-info` with the pinned `~/scratch/llvm-build/bin/opt` (24.0.0git `d1106deb71cc`), on llvm-difftest's replay of the dumped pass inputs, pair by pair, every difference classified against today's. The first table of this page is the count of NoAlias-vs-MayAlias, MayAlias-vs-NoAlias and MustAlias differences; wherever today says NoAlias and BasicAA says MayAlias is a candidate miscompile and goes to a program test and a fix PR ahead of the perf work.
3. **Walker equality** (memoryssa-port): `check_clobber_sanity` on every load and store of 167 `d_faces` bodies must hold with the new AA.

### First table (oracle 2, `tools/aliascompare.py --basic` of #2176)

`opt` 24.0.0git `d1106deb71cc` (`~/scratch/llvm-build/bin/opt`), `-aa-pipeline=basic-aa -passes=aa-eval -print-all-alias-modref-info`. Inputs: the hoist, gvn and dse pass inputs of the replay set (`~/scratch/replay/inputs`, made by `mkinputs.sh` at main `7ac662405`, `-fno-inline-functions`) of `d_faces` -O1 (66), -O2 (104) and -Os (104): 274 dumps. Both sides list what aa-eval lists: the pointer operand of each load and store with the size of the access, one entry for each pair of pointer and size; llrm's answer is `regions::overlapping` of the two accesses with their provenance, as the consumers ask it, without the type tags (aa-eval's pointer pairs carry none, so TBAA cannot show in them). Failed rows: 0. Pairs one side printed that the other did not: 0.

| ours | LLVM BasicAA | pairs |
|---|---|---|
| NoAlias | NoAlias | 677,473 |
| MayAlias | MayAlias | 612,017 |
| MayAlias | NoAlias | 11,470 (precision LLVM has) |
| MayAlias | MustAlias | 323 |
| NoAlias | MayAlias | 22,815 |

The 22,815 are the candidate-miscompile column; it is closed for the `d_faces` set by two checks and OPEN for the other replay programs (below). (1) Every one is a pair of pointers loaded from one slot (or a gep of one) whose slot is stored exactly once, before both loads in program text, with no use other than that store and the loads (all 738 pointer slots of the 274 dumps have no gep, cast or call of the slot), so no store or call lies between the loads on any path: 0 pairs with an intervening write. (2) Width does not matter for the set: all 22,815 are disjoint at the access sizes (2 or 4 bytes, offsets at least 6 apart in the 20 pairs read, e.g. `gep(%p, 8)` against `gep(%p, 80)`) and at one byte; the count equals the earlier one-byte count because it is the same set of pairs, not because the harness ignores the size (it prints each access's size, and those are the sizes aa-eval lists). llrm knows the two loads hold one pointer, BasicAA on this pre-GVN input does not; that precision is what LLVM gets from SROA and mem2reg turning the two loads of a once-stored slot into one SSA value, so once real SROA is on these pairs become BasicAA base-pointer answers and the mirror loses nothing: nobody keeps the flow-sensitive slot solve for them. Limits: stores are counted by text, not by dominance, and only `d_faces` hoist/gvn/dse inputs were read. The 323 are all two geps of one base at one offset (same size): the address is the same and `overlapping` says it overlaps, which the harness prints as MayAlias (it prints only No or May), so they are not a precision gap. The 11,470 are not yet grouped by cause.

Other replay programs (154 dumps: nbody, matmul, floats, fpbench, the mem, loop and diamond ladders; failed rows 0, pairs one side printed that the other did not: 0): ours NoAlias / LLVM MayAlias is 10,140,870, mostly the mem ladders (the same same-slot class) and 6,649 pairs of a slot against a gep of a pointer loaded from it in loop400/800. The column is OPEN for these until the store-count check on the mem ladder is done.

The harness refuses a set its reference answers in one way only (it needs both NoAlias and MustAlias among LLVM's answers, else FAILED PREMISE): an instrument that cannot answer more than one way agrees with anything.

How the column was found right: two earlier readings of the same inputs gave 22,815 and 282,728 NoAlias-against-MayAlias pairs because of my instrument (a one-byte width, then raw object sets that treat parameters as possibly aliasing), and a third (393,184) compared llrm's answers with the type tags against a pipeline that cannot see them. Each was found by reading the pairs. With the type tags on and LLVM's default pipeline the table is not comparable: aa-eval's pair queries carry no TBAA tag.

## 4. Stages and gates

| stage | content | gate |
|---|---|---|
| S0 | the interface | #2163 merged |
| S1 | `MemoryLocation`/`LocationSize`; BasicAA over identified objects; `CaptureTracking`; `isNonEscapingLocalObject` | oracle 1 equal; oracle 2 table filled |
| S2 | `aliasGEP`, `aliasPHI`, `aliasSelect`, TBAA, scoped noalias | oracle 2 differences classified; no new NoAlias that LLVM says May |
| S3 | call ModRef and `MemoryEffects` | needs function-attrs; `capturecheck` agrees with each derived `nocapture` |
| S4 | `GlobalsModRef` | the 19 `d_faces` -O2 hoists LLVM does and ours does not (`~/scratch/s28-work/licm_llvm_only_d_faces-O2.txt`) all hoist, no hoist LLVM does not do; rows per metric and level, worse counts first |
| S5 | clients, one PR each: the MemorySSA walker, MemDep (gvn-port), LICM, DSE | each PR states "LLVM structure: X; ours: Y; mirror or deviate because Z"; objects identical or classified; the per-IR-instruction cost of the memory path against the 3.9 K instructions of `Accesses::plain` plus `Shape` today |
| S6 | delete the tower and its tests | no caller left; dense-key ratchet lowered |

## 5. Deviations (numbered)

1. Pointers are `Operand`s and instructions `InstId`s, not `Value*`; the DOS targets have segment and near pointers (`MemRef.segment`, `index_bits`), which the translation keeps as part of the location.
2. `ModRefInfo` has no `Must` variants (the pinned header has none); `AliasResult::PartialAlias` carries an optional offset.
3. No `invariant.group`, fences or atomics (MIR has none).
4. The capture oracle trusts a callee another module defines when one module is compiled alone; the module report counts them.
5. The first implementation answers from the existing provenance and `MemRef` where LLVM walks `GEP` chains; each place is replaced by the translated walk at S2 and listed in that PR.
6. `CaptureTracking` stops at 100 uses of an object (`getDefaultMaxUsesToExploreForCaptureTracking`) and answers captured. Seen: a slot with about 200 loads in `loop400` (replay input 01/0011-in-hoist) is NoAlias against a loaded pointer for llrm and MayAlias for LLVM; the mirror takes the cap (precision lost, rows to be counted), a deviation takes none. Decided at S1 by the rows.

## 6. Open

The first table (oracle 2), the cost of a query against LLVM's, the GlobalsModRef design, and the dates, which depend on valuetracking-port's `getUnderlyingObject` and cgscc-poc's function-attrs.
