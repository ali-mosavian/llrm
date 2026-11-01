# Wasm exception handling (ON ERROR landing)

One PR, one commit per stage; the pin d1106deb71cc, legacy EH, `llc -mtriple=wasm32 -O0 -fast-isel=false
-exception-model=wasm -mattr=+exception-handling` is the oracle. Each stage compares to it on the same IR, instantiates
its fixtures under node, has a fail-first test and states its cost (O(n)).

| # | Stage | Shared crates |
|---|-------|---------------|
| 0 | `ExceptionModel` on the wasm driver; `wasm_exceptions` (`--wasm-exceptions`) keeps the invokes for the backend | llrm-codegen (flag) |
| 1 | `llvm.wasm.*` target intrinsics in the reader and builder | llrm-mir, llrm-codegen |
| 2 | WasmEHPrepare: dropped. The pin pass rewrites `catchpad`/`cleanuppad` funclets and cleans up after `llvm.wasm.throw`; a `landingpad` function (QB's) gets no `catch` intrinsic from it (pin `llc` on `invoke`/`landingpad`: `try`, `catch_all`, no `catch`), and QB writes `unreachable` after its throw | no |
| 3 | Unwind edges and EH pads in the wasm MIR. The builder's DAG is already the pin's (the pin skips `EH_LABEL` on wasm, `lowerStartEH`), so no shared edit: `emit` adds the pad as a successor and an `EH_LABEL` at its head | no |
| 4 | `WebAssemblyExceptionInfo`, `SortRegionInfo` (`exception_info.rs`, `sort_region.rs`) | no |
| 5 | LateEHPrepare: `catch_all`, stack pointer write-back, `needsPrologForEH` in the frame | no |
| 6 | CFGSort: EH pads first, regions are loops or exceptions; removes DEVIATE W1 | no |
| 7 | CFGStackify `placeTryMarker`, `removeUnnecessaryInstrs`; `fixCallUnwindMismatches` and `fixCatchUnwindMismatches` stay out (DEVIATE W2: a call in a `try` that does not unwind to it is refused) | no |
| 8 | Object and text: `try`/`catch_all`/`end_try` encodings, `exception-handling` feature, the table the pin keeps for a personality | no |
| 9 | QB: `ehprepare` always runs; a handler lands by `llrm_host_land` (a JS throw) into the function's `catch_all`, which re-enters the body through a dispatch loop; host endings are traps, so `END` is not caught. 22 of 28 ON ERROR programs match, 3 more by tail; 3 do not build (`LRint` x2, `palette_using`); `ON LOCAL ERROR` needs the runtime PR. The `setjmp` stub goes after | no |

Stages 3 to 8 are measured on `tests/eh/*.ll`: text and object byte-identical to the pin.

Stage 1 carries an x86 and arm64 identical-output sweep, with binary md5s and a live control, as their own commits.
Stage 0's line: `ExceptionModel::None` and `wasm_exceptions = false` leave every object as it was.

DEVIATE W19: `calls_unwind = false` for QB. Its mid-end makes every call that can raise an `invoke`, so a plain call in a `try` never throws and the pin's `fixCallUnwindMismatches` has nothing to fix. Exit: port that pass.
