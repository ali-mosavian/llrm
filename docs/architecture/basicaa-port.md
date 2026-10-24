---
architecture-index: BasicAA as a translation of LLVM's
architecture-order: 290
---
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
| `getModRefInfo(Call, Loc)`, `getArgModRefInfo`, `MemoryEffects` | `call_mod_ref`, `get_memory_effects`, `get_arg_mod_ref_info` in `basicaa.rs` (`CallEffects` is deleted); `getModRefInfo(Call1, Call2)` since #3799 (DEVIATE AA-D1) | S3: per-parameter `nocapture` from function-attrs (cgscc-poc) still blocks the inferred ones |
| `GlobalsModRef` | `GlobalsAA` | S4 |
| TBAA, scoped noalias | `MemRef.typed`, restrict roots | with S2 |
| `MemoryLocation::get`, `LocationSize` | `MemRef` width | S1 |

## 3. Oracles

1. **Captures** (done, #2146): every alloca's uses read directly; the points-to facts must list what is captured. On the gate corpus 1,583 objects, 0 differ; module report at -O2: 526 callee parameters verified in the callee's own body, 584 external, 370 captured by the body, 0 under-escapes. It caught #2132's class on main.
2. **LLVM's answers** (not run): `opt -passes=aa-eval -print-all-alias-modref-info` with the pinned `opt` (24.0.0git `d1106deb71cc`), on llvm-difftest's replay of the dumped pass inputs, pair by pair, every difference classified against today's. The first table of this page is the count of NoAlias-vs-MayAlias, MayAlias-vs-NoAlias and MustAlias differences; wherever today says NoAlias and BasicAA says MayAlias is a candidate miscompile and goes to a program test and a fix PR ahead of the perf work.
3. **Walker equality** (memoryssa-port): `check_clobber_sanity` on every load and store of 167 `d_faces` bodies must hold with the new AA.

### Second table: load and store pairs, with the type tags (`--memops`)

`aa-eval -evaluate-aa-metadata` asks `alias(MemoryLocation::get(load), MemoryLocation::get(store))` with each access's tags, so TBAA shows here (it cannot in the pointer pairs). Same 274 `d_faces` dumps; ours = `regions::overlapping` of the two `MemRef`s with provenance and type tags; pairs matched by position in aa-eval's order (each load against each store, then each store against the stores before it), not by text. Failed rows 0; functions whose pair counts differ: 0.

| ours | LLVM (tbaa, scoped-noalias, basic) | pairs | ours (no tags) | LLVM (basic) | pairs |
|---|---|---|---|---|---|
| NoAlias | NoAlias | 3,769,726 | NoAlias | NoAlias | 3,513,180 |
| MayAlias | MayAlias | 119,562 | MayAlias | MayAlias | 350,196 |
| MayAlias | NoAlias | 4,472 | MayAlias | NoAlias | 20,106 |
| MayAlias | MustAlias | 41,105 | MayAlias | MustAlias | 41,105 |
| NoAlias | MayAlias | 7,119 | NoAlias | MayAlias | 17,397 |

All 7,119 and all 17,397 `NoAlias`-against-`MayAlias` pairs are accesses through pointers loaded from one slot (the class of the first table; the slots are stored once and used by nothing else), so they are not a miscompile column. The 41,105 are equal addresses that the harness prints as MayAlias. The type tags move LLVM from 3.51 M to 3.77 M NoAlias pairs and ours from 3.51 M to 3.77 M: the tags agree. The other direction (ours MayAlias, LLVM NoAlias: 4,472 with tags, 20,106 without) has a global variable on one side in every pair: against a parameter pointer (2,891 with tags, 11,187 without), a pointer loaded from memory (1,581; 8,558), a phi (304, without tags) or an inttoptr (57, without tags). The cause, by one-variable experiments on LLVM's side over every third `d_faces` dump (6,018 candidate pairs, BasicAA alone, both sides reading the dumps with the machine's `-m32` data layout, which the dumps lack and which moved 1,104 pairs that LLVM's default 64-bit pointers had made look like size-rule cases): 4,936 (82%) flip to MayAlias when `inbounds` is dropped from the GEPs: `aliasGEP`'s rule "if an inbounds GEP would have to start from an out of bounds address for the two to alias" (BasicAliasAnalysis.cpp, after `subtractDecomposedGEPs` cancels a variable index both pointers share, the constant remainder puts one access wholly past the other's base object); 607 (10%) flip when the globals are enlarged: `isObjectSmallerThan`; 0 flip when every global's address is captured (BasicAA applies capture tracking only to function-local identified objects); 475 (8%) are not yet explained. Both rules are S2 (`aliasGEP`, the object-size check of `aliasCheck`), not capture tracking, which an earlier version of this page named and which is withdrawn. A third direction of the same experiment is the data layout: the dumps carry none, and the harness now gives both sides the `-m32` string, so the size rule sees 4-byte pointers as the machine does.

### Third table: call mod/ref pairs (`--calls`)

Each call against each load/store pointer (one entry per pointer and size), then each call against every other call, matched by position; ours = what `CallEffects` says (Mod if its stores or fills overlap the access, Ref if its loads do). All 428 replay dumps (d_faces -O1/-O2/-Os and the other programs). Failed rows 0; functions whose counts differ: 0.

| ours | LLVM (default pipeline, and BasicAA alone: identical) | pairs |
|---|---|---|
| ModRef | ModRef | 213,832 |
| NoModRef | NoModRef | 121,566 |
| Ref | Ref | 7,635 |
| Mod | Mod | 1,445 |
| NoModRef | ModRef | 18 |

The 18 are call-against-call pairs in `floats-O2`: `bench_floats_` (declared `memory(inaccessiblemem: readwrite)`) against `report_` (no attributes). LLVM says ModRef. They were a hazard when `CallEffects` listed no inaccessible memory; that table is gone, `call_mod_ref` handles `InaccessibleMem`, and the pair is ModRef as in the pin (checked against the pinned `opt -passes=aa-eval -aa-pipeline=basic-aa`, 2026-10-22). The real gap was `AAResults::getModRefInfo(Call1, Call2)`, which answered ModRef for every pair; it is translated in #3799 (readnone, both read-only, argument-pointee and inaccessible-only pairs).

### First table (oracle 2, `tools/aliascompare.py --basic` of #2176)

`opt` 24.0.0git `d1106deb71cc`, `-aa-pipeline=basic-aa -passes=aa-eval -print-all-alias-modref-info`. Inputs: the hoist, gvn and dse pass inputs of the replay set (the replay inputs, made by `mkinputs.sh` at main `7ac662405`, `-fno-inline-functions`) of `d_faces` -O1 (66), -O2 (104) and -Os (104): 274 dumps. Both sides list what aa-eval lists: the pointer operand of each load and store with the size of the access, one entry for each pair of pointer and size; llrm's answer is `regions::overlapping` of the two accesses with their provenance, as the consumers ask it, without the type tags (aa-eval's pointer pairs carry none, so TBAA cannot show in them). Failed rows: 0. Pairs one side printed that the other did not: 0.

| ours | LLVM BasicAA | pairs |
|---|---|---|
| NoAlias | NoAlias | 677,473 |
| MayAlias | MayAlias | 612,017 |
| MayAlias | NoAlias | 11,470 (precision LLVM has) |
| MayAlias | MustAlias | 323 |
| NoAlias | MayAlias | 22,815 |

The 22,815 are the candidate-miscompile column; it is closed for the `d_faces` set by two checks and for the other replay programs (below). (1) Every one is a pair of pointers loaded from one slot (or a gep of one) whose slot is stored exactly once, before both loads in program text, with no use other than that store and the loads (all 738 pointer slots of the 274 dumps have no gep, cast or call of the slot), so no store or call lies between the loads on any path: 0 pairs with an intervening write. (2) Width does not matter for the set: all 22,815 are disjoint at the access sizes (2 or 4 bytes, offsets at least 6 apart in the 20 pairs read, e.g. `gep(%p, 8)` against `gep(%p, 80)`) and at one byte; the count equals the earlier one-byte count because it is the same set of pairs, not because the harness ignores the size (it prints each access's size, and those are the sizes aa-eval lists). llrm knows the two loads hold one pointer, BasicAA on this pre-GVN input does not; that precision is what LLVM gets from SROA and mem2reg turning the two loads of a once-stored slot into one SSA value, so once real SROA is on these pairs become BasicAA base-pointer answers and the mirror loses nothing: nobody keeps the flow-sensitive slot solve for them. Limits: stores are counted by text, not by dominance, and only `d_faces` hoist/gvn/dse inputs were read. The 323 are all two geps of one base at one offset (same size): the address is the same and `overlapping` says it overlaps, which the harness prints as MayAlias (it prints only No or May), so they are not a precision gap. Of the 11,470 pairs where ours is MayAlias and LLVM NoAlias, 11,375 have a global variable on one side (global/loaded pointer 6,267, global/parameter 4,747, global/phi 304, global/inttoptr 57); 95 do not (load/load 57, phi/phi 38) and are not yet classified.

Other replay programs (154 dumps: nbody, matmul, floats, fpbench, the mem, loop and diamond ladders; failed rows 0, pairs one side printed that the other did not: 0): ours NoAlias / LLVM MayAlias is 10,140,870, mostly the mem ladders (the same same-slot class) and 6,649 pairs of a slot against a gep of a pointer loaded from it in loop400/800. Closed for these by the slot-level check over all 154 dumps and a pair-level class count on the mem400 and loop400 hoist inputs: every pointer-typed slot of the nbody, matmul, floats, fpbench and ladder dumps (6 each in the mem and loop ladders, none elsewhere) is stored once or never and has no use but that store and direct loads; the 676,862 pairs counted are 674,634 two loads of one slot (disjoint offsets) and 2,228 a slot against a gep of a pointer loaded from it (the slot's address is never taken, so what is loaded cannot be the slot). So no store or call lies between the loads, and the slot is not an escape. Not read pair by pair: the other 9.46 M pairs of the sweep (the same two classes by the slot-level result, but counted only on those two inputs).

The harness refuses a set its reference answers in one way only (it needs both NoAlias and MustAlias among LLVM's answers, else FAILED PREMISE): an instrument that cannot answer more than one way agrees with anything.

How the column was found right: two earlier readings of the same inputs gave 22,815 and 282,728 NoAlias-against-MayAlias pairs because of my instrument (a one-byte width, then raw object sets that treat parameters as possibly aliasing), and a third (393,184) compared llrm's answers with the type tags against a pipeline that cannot see them. Each was found by reading the pairs. With the type tags on and LLVM's default pipeline the table is not comparable: aa-eval's pair queries carry no TBAA tag.

## 4. Stages and gates

| stage | content | gate |
|---|---|---|
| S0 | the interface | #2163 merged |
| S1 | `MemoryLocation`/`LocationSize`; BasicAA over identified objects; `CaptureTracking`; `isNonEscapingLocalObject` | oracle 1 equal; oracle 2 table filled |
| S2 | `aliasGEP`, `aliasPHI`, `aliasSelect`, TBAA, scoped noalias | oracle 2 differences classified; no new NoAlias that LLVM says May |
| S3 | call ModRef and `MemoryEffects` | needs function-attrs; `capturecheck` agrees with each derived `nocapture` |
| S4 | `GlobalsModRef` | the 19 `d_faces` -O2 hoists LLVM does and ours does not all hoist, no hoist LLVM does not do; rows per metric and level, worse counts first. Cost: GlobalsAA is 42 M on `d_faces` (glue partition) against LLVM's 1.1 M; target at most ~1.5x LLVM per instruction, one linear pass over the module's globals and call graph, no per-function rebuild, slope test at 2N |
| S5 | clients, one PR each: the MemorySSA walker, MemDep (gvn-port), LICM, DSE | each PR states "LLVM structure: X; ours: Y; mirror or deviate because Z"; objects identical or classified; the per-IR-instruction cost of the memory path against the 3.9 K instructions of `Accesses::plain` plus `Shape` today |
| S6 | delete the tower and its tests | no caller left; dense-key ratchet lowered |

## 4b. Adoption: who calls it, in what order

Each step is its own PR with "LLVM structure: X; ours: Y; mirror or deviate because Z", the failed-row line first and per-metric worse counts, and objects identical or classified.

| step | consumer (owner) | what moves | gate |
|---|---|---|---|
| A0 | the trait, #2163 | `aa.rs`; names fixed with memoryssa-port and gvn-port | merged |
| A1 | the MemorySSA walker and updater (memoryssa-port) | a `StandIn: AAResults` adapter over today's `AliasQuery`, walker on `BatchAAResults`; dated 2026-10-22 | walker answers equal on 167 `d_faces` bodies (`check_clobber_sanity`) |
| A2 | S1 BasicAA (basicaa-port) | replaces the stand-in under A1 without touching the walker | oracle 2 tables no worse; the 22,815 same-slot pairs stay NoAlias or become base-pointer answers once SROA is on |
| A3 | MemDep (gvn-port) | `MemoryDependenceResults` calls the trait and no private alias logic | gvn objects identical; per-IR-instruction cost of the memory path against the 3.9 K of `Accesses::plain` plus `Shape` |
| A4 | LICM (memoryssa-port) | `LICM.cpp` on MemorySSA and the trait; `get_mod_ref_info_mask` with `ignore_locals`, `only_writes_memory`, `does_not_access_memory` | the 19 `d_faces` -O2 hoists of S4 |
| A5 | DSE (dse-poc) | `DSE.cpp` on MemorySSA and the trait | stores dropped equal or more, none LLVM keeps |
| A6 | call CSE in EarlyCSE (llvm-order-poc) | read-only call numbering | waits for #2219 (the unknown callee's footprint omits inaccessible memory) |
| A7 | delete the tower | `alias.rs` facts, `frameescape.rs`, `llrm-mir/alias.rs` | no caller left |

### 4c. Retiring `MemRef`: the readers

`MemRef::of` (and `point_values`) go when no reader is left. One owner moves every reader, one PR per file, each with bench's M for the step. Fields read are from a grep of `.field` in the file, so a count can include another struct's field.

| reader | `of` calls | fields read | what it does | replacement | reviewer |
|---|---|---|---|---|---|
| `references.rs` | 3 | `lineage` | the cache of `MemRef::of` | goes with `MemRef` | none needed |
| `slotstores.rs` | 3 | root, disp, base, width | cells of stored slots | `Decomposed` (object, offset, index) plus a size | unclaimed |
| `floatloop.rs` | 3 | pointer, width | two accesses name the same address | `alias() == MustAlias` through `AAQueryInfo` | unclaimed |
| `floatfacts.rs` | 5 | object | latch stores as cells | `Decomposed` plus a size | unclaimed |
| `floatbounds.rs` | 1 | disp, base, scale, segment, width | builds a `MemRef` for a cell | `Decomposed`; the segment field is DEVIATE F2 below | unclaimed |
| `consts.rs` | 4 | root, disp, base, segment, object, space, width, provenance | known values per object cell | `Decomposed`, underlying objects, `unit.space()` | unclaimed |
| `observers.rs` | 1 | object, width, provenance | which accesses a kind of object reaches | underlying object and captured-before | unclaimed |
| `induction.rs` | 1 | pointer, space, index_bits, width, inbounds | access stride | the pointer's SCEV | scev-createscev-2239 |
| `isel.rs` | 1 | typed | which accesses carry `!tbaa` | the access's `AAInfo` | lir-cleanup-port |
| `manager.rs` | 1 | pointer, root, base, segment, object | holds `IndexMap<InstId, MemRef>` | goes with `Accesses` | none needed |
| `ranges.rs`, `regions.rs` | 1, 1 | disp, base, scale, width | the old ranges/regions engine | deleted with it | parity-pipeline |
| `alias.rs` | 7 | all of them | the old points-to tower | deleted with the tower (funcattrs-port's call effects, then A7) | funcattrs-port |
| `memory.rs` | 5 | all of them | defines `MemRef`, `ObjectRef`, `classes_may_alias` | last; deleted when the rest are gone | none needed |

Fields with no LLVM counterpart, decided (2026-10-24):

- **F1 `typed`, `lineage` (TBAA).** Home: a translation of `TypeBasedAliasAnalysis.cpp` as an `AAResults` provider over `AAInfo::tbaa`, which every load and store already carries (the C front end emits clang's scalar `!tbaa` tree; `llrm_mir::tbaa::Tbaa` is that tree). It needs the aggregate LLVM's `AAResults` has (`addAAResult`: the first answer that is not MayAlias wins, mod/ref intersect), which is the first step. DEVIATE F1: scalar tags only (type, parent, offset 0); LLVM's struct-path tags are not emitted by any front end here. Exit: a front end that emits struct-path tags. `isel.rs` reads `AAInfo::tbaa.is_some()` meanwhile.
- **F2 `segment`, `selector`, `origin`, `linear` (far pointers and integer addresses).** Home: the datalayout, as LLVM states it: a far pointer is its own address space with pointer size 32 and index width 16 (`pN:32:16:16:16`; the m16 layout already has `p1:32:16:16:16`). The index width is what `GEP`, `ptrtoint` and offset arithmetic wrap at, so they follow the per-address-space paths with no special case, and BasicAA's decomposition uses that index width as at the pin. The selector/offset split of a pair-typed integer (`pair_constant`) becomes a `DataLayout` method beside `is_pair` and `offset_bits`; `Spaces` keeps only which space is far or fixed, and `linear` (one flat address space). DEVIATE F2: LLVM has no segment arithmetic; the split of a pair constant is a target fact stated once, tested at the boundary. No exit: a property of the DOS targets. Acceptance test of the first reader PR that needs it: a far-pointer `GEP` whose offset wraps at 16 bits. Oversight: agreed (2026-10-24).
- **F3 `provenance`, `object` (object sets).** Home: `getUnderlyingObjects` (ValueTracking), which is not translated; only `getUnderlyingObject` is. Translate it with its lookup depth (`MaxLookup`, 6), and take "is a root an object's own address" from `isIdentifiedObject`. Exit: none; the old sets go with the tower.

### 4d. Exposure by CaptureTracking: the verdict on the 286 and the 2

Corpus probe over every alloca of `crates/opt/llrm-analysis/corpus` (3,987 allocas, `feat/parity` at 86be6bc37 plus a test-only probe, not committed): the old `frameescape::exposed_allocas` against `pointer_may_be_captured(ALL, any component, 150 uses)`, return captures counted. 3,699 agree.

- **286 exposed only by the old walk.** Every one is the destination (operand 0) of `llvm.memset.p0.i16` and nothing else: the old walk counts any call argument as exposing; `DetermineUseCaptureKind` reads the callee's `captures(none)` on the memset destination. Pin check, `opt` 24.0.0git `-passes=function-attrs` on a `memset` of a pointer argument: `captures(none)`. Verdict: precision, not a bug. The call still writes the slot, which `getModRefInfo(Call, Loc)` answers by its argument effects (#2899); the exposure answer feeds only whether an unknown pointer may hold the address.
- **2 captured only by the new walk** (`demo-deedlines` `@COPPER`, `@CREATEOBJECT`). Their uses are loads and stores only: 273 and 151, over the 150-use cap, so the walk says captured. The pin has the same cap (§5 item 6: 148 loads NoAlias, 149 MayAlias). Verdict: conservative for the pin's reason.
- **Causes I expected and did not find in the corpus:** `icmp`-only (pin: `captures(address)`, no provenance), lifetime markers (pin: `captures(none)`) and the phi-join rule (pin: `captures(none)`). Pin checks on small functions agree with the new walk for all three.
- **Cost: not measured.** bench's callgrind on a94491462 (d_faces -O2): the old `exposed_frames` is 301 calls, 30.7 M inclusive. A per-alloca walk's cost has no measurement yet; it is not claimed to be worse or equal.

### Item 5: the global-variable cells (S2)

The 4,472 / 16,914 / 9,418 cells of the tables (tags on, BasicAA alone memops, BasicAA alone pointer pairs, with the machine's data layout) have a global on one side; 82% are `aliasGEP`'s inbounds rule, 10% the object-size rule, 8% open. Consumers: the hoist (a load of an internal global past char-tagged stores through a loaded pointer: the 19 `d_faces` -O2 hoists LLVM does and ours does not, 15 in -O1 and 19 in -Os) and GVN's load forwarding and DSE through the same query. Before: 19 hoists missing (memoryssa-port's L1 shadow). After: not measured; the number of those 19 that hoist once S2 is in, rows line first, is the acceptance of S4 and whether these cells are their cause is itself unproven.

## 5. Deviations (numbered)

1. Pointers are `Operand`s and instructions `InstId`s, not `Value*`; the DOS targets have segment and near pointers (`MemRef.segment`, `index_bits`), which the translation keeps as part of the location.
2. `ModRefInfo` has no `Must` variants (the pinned header has none); `AliasResult::PartialAlias` carries an optional offset.
3. No `invariant.group`, fences or atomics (MIR has none).
4. The capture oracle trusts a callee another module defines when one module is compiled alone; the module report counts them.
5. The first implementation answers from the existing provenance and `MemRef` where LLVM walks `GEP` chains; each place is replaced by the translated walk at S2 and listed in that PR.
6. `CaptureTracking` stops at 150 uses of an object (`getDefaultMaxUsesToExploreForCaptureTracking`) and answers captured. Seen: a slot with about 200 loads in `loop400` (replay input 01/0011-in-hoist) is NoAlias against a loaded pointer for llrm and MayAlias for LLVM; the mirror takes the cap (precision lost, rows to be counted), a deviation takes none. Verified with the pinned opt (24.0.0git d1106deb71cc): a slot with 148 loads is NoAlias against a gep of a pointer loaded from it, with 149 it is MayAlias (the slot has two more uses). Default: mirror the cap (LLVM's limits at its defaults); an uncapped answer stays only as a numbered deviation if a row needs it, with its compile cost; the d_faces and mem-ladder compile numbers with and without the cap come with S1.
7. `getModRefInfo(Call1, Call2)` is BasicAA's guard-intrinsic case only; MIR has no `experimental.guard`, so it answers ModRef (B2). The effects-based call-vs-call form is `AAResults`'.

## 6. Open

The first table (oracle 2), the cost of a query against LLVM's, the GlobalsModRef design, and the dates, which depend on valuetracking-port's `getUnderlyingObject` and cgscc-poc's function-attrs.
