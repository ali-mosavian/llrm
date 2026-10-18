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

### First table (oracle 2, `tools/aliascompare.py` of #2176)

`opt` 24.0.0git `d1106deb71cc` (`~/scratch/llvm-build/bin/opt`; inputs made by `mkinputs.sh` at main `7ac662405`, `-fno-inline-functions`), `-passes=aa-eval -print-all-alias-modref-info`; llrm side: `llrm-mir --aliases`, the answer `regions::overlapping` gives two pointers with every byte of their objects (aa-eval asks without a size). Inputs: the hoist, gvn and dse pass inputs of the replay set (`~/scratch/replay/inputs`) of `d_faces` -O1 (66), -O2 (104) and -Os (104): 274 dumps, 166 with two or more pointers. Failed rows: 0.

| ours | LLVM | pairs |
|---|---|---|
| NoAlias | NoAlias | 643,275 |
| MayAlias | MayAlias | 629,409 |
| MayAlias | NoAlias | 18,976 (precision LLVM has) |
| MayAlias | MustAlias | 323 |
| NoAlias | MayAlias | 0 (the candidate-miscompile column) |

Pairs llrm printed that aa-eval did not: 79,186 (not compared). Two earlier readings of the same inputs gave 22,815 and 282,728 NoAlias-against-MayAlias pairs; both were my instrument (a one-byte width where aa-eval has none, and raw object sets where consumers treat parameters as possibly aliasing), found by reading the pairs: every one was two pointers loaded from one cell or derived from one parameter. The method is part of the table: the column is only as good as the width and object rule of `aliases()`.

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

## 6. Open

The first table (oracle 2), the cost of a query against LLVM's, the GlobalsModRef design, and the dates, which depend on valuetracking-port's `getUnderlyingObject` and cgscc-poc's function-attrs.
