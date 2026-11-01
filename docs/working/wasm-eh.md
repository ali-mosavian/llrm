# Wasm exception handling (ON ERROR landing)

One PR, one commit per stage; the pin d1106deb71cc, legacy EH, `llc -mtriple=wasm32 -O0 -fast-isel=false
-exception-model=wasm -mattr=+exception-handling` is the oracle. Each stage compares to it on the same IR, instantiates
its fixtures under node, has a fail-first test and states its cost (O(n)).

| # | Stage | Shared crates |
|---|-------|---------------|
| 0 | `ExceptionModel` on the wasm driver; `keep_landing_pads` skips `ehprepare`; `--wasm-exceptions` | llrm-codegen (flag) |
| 1 | `llvm.wasm.*` target intrinsics in the reader and builder | llrm-mir, llrm-codegen |
| 2 | WasmEHPrepare: dropped. The pin pass rewrites `catchpad`/`cleanuppad` funclets and cleans up after `llvm.wasm.throw`; a `landingpad` function (QB's) gets no `catch` intrinsic from it (pin `llc` on `invoke`/`landingpad`: `try`, `catch_all`, no `catch`), and QB writes `unreachable` after its throw | no |
| 3 | Unwind edges and EH pads in the wasm MIR. The builder's DAG is already the pin's (the pin skips `EH_LABEL` on wasm, `lowerStartEH`), so no shared edit: `emit` adds the pad as a successor and an `EH_LABEL` at its head | no |
| 4 | `WebAssemblyExceptionInfo`, `SortRegionInfo` (`exception_info.rs`, `sort_region.rs`) | no |
| 5 | LateEHPrepare: `catch_all`, stack pointer write-back, `needsPrologForEH` in the frame | no |
| 6 | CFGSort: EH pads first, regions are loops or exceptions; removes DEVIATE W1 | no |
| 7 | CFGStackify `placeTryMarker`, `removeUnnecessaryInstrs`; `fixCallUnwindMismatches` and `fixCatchUnwindMismatches` stay out (DEVIATE W2: a call in a `try` that does not unwind to it is refused) | no |
| 8 | Object and text: `try`/`catch_all`/`end_try` encodings, `exception-handling` feature, the table the pin keeps for a personality | no |
| 9 | QB: every call of a handler function is an `invoke` or `nounwind`; `qb_land` throws; the ON ERROR programs run; the `setjmp` stub goes | no |

Stages 3 to 8 are measured on `tests/eh/*.ll`: text and object byte-identical to the pin.

Stages 1 and 3 carry an x86 and arm64 identical-output sweep, with binary md5s and a live control, as their own commits.
Stage 0's line: `ExceptionModel::None` and `keep_landing_pads = false` leave every object as it was.
