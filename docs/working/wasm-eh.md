# Wasm exception handling (ON ERROR landing)

One PR, one commit per stage; the pin d1106deb71cc, legacy EH, `llc -mtriple=wasm32 -O0 -fast-isel=false
-exception-model=wasm -mattr=+exception-handling` is the oracle. Each stage compares to it on the same IR, instantiates
its fixtures under node, has a fail-first test and states its cost (O(n)).

| # | Stage | Shared crates |
|---|-------|---------------|
| 0 | `ExceptionModel` on the wasm driver; `keep_landing_pads` skips `ehprepare`; `--wasm-exceptions` | llrm-codegen (flag) |
| 1 | `llvm.wasm.*` target intrinsics in the reader and builder | llrm-mir, llrm-codegen |
| 2 | WasmEHPrepare: dropped. The pin pass rewrites `catchpad`/`cleanuppad` funclets and cleans up after `llvm.wasm.throw`; a `landingpad` function (QB's) gets no `catch` intrinsic from it (pin `llc` on `invoke`/`landingpad`: `try`, `catch_all`, no `catch`), and QB writes `unreachable` after its throw | no |
| 3 | `invoke`/`landingpad` built as the pin (`EH_LABEL`, EH pads); removes DEVIATE SD102 for the model | llrm-codegen |
| 4 | CATCH/THROW/RETHROW/TRY nodes, `WebAssemblyExceptionInfo` | no |
| 5 | LateEHPrepare | no |
| 6 | CFGSort EH regions; removes DEVIATE W1 | no |
| 7 | CFGStackify `try`/`catch`/`delegate`; removes DEVIATE W2 | no |
| 8 | Tag section, `__cpp_exception`, `try` encodings and text | no |
| 9 | QB: `qb_land` throws; the ON ERROR programs run; the `setjmp` stub goes | no |

Stages 1 and 3 carry an x86 and arm64 identical-output sweep, with binary md5s and a live control, as their own commits.
Stage 0's line: `ExceptionModel::None` and `keep_landing_pads = false` leave every object as it was.
