# MemorySSA Accesses: BasicAA per query (exit of DEVIATE AN1)

Owner w-3, with funcattrs-port (swaps `Annotated`'s range source meanwhile). Pin: [MemorySSA.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp), [BasicAliasAnalysis.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BasicAliasAnalysis.cpp) at d1106deb71cc.

## Why

d_faces -O2 (board command, release profile; funcattrs-port's dist read on 62d45c8880 agrees): `MemorySsa::run` is 40 builds, 59.8 M Ir; `PlainAccesses` 51.8 M of it (`Accesses::plain` 15.9, `Annotated` 8.1, `Writes` 6.8). The pin spends 3.0 M on MemorySSA in the same compile. The build count is already one per function, so the cost is each build: we make a `MemRef` for every load and store of the function and a footprint for every call before the first query; the pin makes none.

## Pin structure

- Build ([`buildMemorySSA` L1522](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1522), [`createNewAccess` L1761](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1761)): an instruction is a Def, a Use or nothing from `mayWriteToMemory`/`mayReadFromMemory` and, for a call, `AA->getModRefInfo(Call, nullptr)`: the call's effects, never a pair of locations.
- `OptimizeUses` ([L1307](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1307)) is the one place the build asks alias questions, one walk per use with a step limit.
- Queries ([`instructionClobbersQuery` L320](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L320), [`ClobberWalker::findClobber` L932](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L932)): `AA.alias(MemoryLocation(ptr, size), ...)` through BasicAA ([`alias` L934](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L934), [`aliasGEP` L1118](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L1118)), the pointer decomposed when asked and cached in the `AAQueryInfo` of the batch.

## Ours today

`Accesses` holds `references` (a `MemRef` per load/store from `Annotated`), `touched`/`fills` (per call, from `CallFootprints`); `mssa/alias.rs::AliasQuery` answers from them. `BasicAA` (`basicaa.rs`, `AAResults` in `aa.rs`) exists with `aliasGEP` (#3799) and `getModRefInfo(Call)`; MemorySSA does not use it.

## Stages

| # | Change | Gate | Date |
|---|---|---|---|
| S1 | `Accesses` keeps only the kind of each instruction (Def, Use, none) and, per call, `BasicAA::get_mod_ref_info` asked on first use; `PlainAccesses` stops asking `Annotated` and `CallFootprints` | structure identical (`LLRM_CHECK_CLOBBERS`); d_faces Ir pair; objsweep unchanged | PR 11-20 |
| S2 | `AliasQuery` asks `AAResults::alias(MemoryLocation::get(inst))` per pair with an `AAQueryInfo` cache; `MemRef` leaves the walker | walker answers vs the shadow (`answer_is_safe`) on all corpus IRs; Ir pair; objsweep and QCport objects | 12-04 (estimate) |
| S3 | Other readers of `Annotated` (`memory.rs` `Unit::annotated`, `references.rs`, `licm/mod.rs`, `loopaccessinfo.rs`, `lazyvalueinfo.rs`) move to `AAResults` or `Unit` queries; `Annotated` deleted | no reader left; parity registry row | 12-11 (estimate; depends on funcattrs-port's range-source swap, date theirs) |

Interface with funcattrs-port: S1 and S2 do not touch `Annotated`'s result type or ranges; S3 starts after their swap lands. Open until S1: whether `OptimizeUses` (the optimized-use names the shadow ignores, `optimized_uses`) is needed at build or only on first walk; the pin does it at build, so S1 keeps it at build over `AAResults`.

## Status 10-23 and the deviations still open

S1a (#3991) and the AliasQuery switch of `mod_set_loc` (#4155) are in. Open, owner w-3:

| # | Deviation | Why it is there | Exit | Date |
|---|---|---|---|---|
| AQ1 | `AliasQuery::mod_set_loc` keeps the capture stand-in (`unreachable_from`) after the AA stack answers | BasicAA has no `aliasPHI`/`aliasSelect` ([BasicAliasAnalysis.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BasicAliasAnalysis.cpp), basicaa.rs L735 "not yet"), where the pin proves a phi of loaded pointers apart from an unexposed local | translate `aliasPHI` and `aliasSelect` in the pin's order, then delete the stand-in | design note and first PR 11-24; exit 11-27 |
| G10 | `GlobalsAAResult::alias` treats a global the unit states unaddressed (`-oa`, INIT `sw`; `Globals::tracked`) as non-address-taken | the Watcom `-oa` switch is a promise the pin has no analog of; `test_relaxed_alias_checking_keeps_unaddressed_objects_across_a_store` pins it | state the promise through the one target description (memory-space/AA model) instead of the `Globals` side table; tuning phase | row 10-30; code undated |
| TQ1 | an absolute-segment volatile store (video memory) is apart from program objects | target knowledge the pin cannot say; the pin keeps both loads of `farvolatile` | tuning row via the target description, `farvolatile` as its regression | undated |
