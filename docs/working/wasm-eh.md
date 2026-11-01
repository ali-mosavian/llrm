# Wasm exception handling (ON ERROR landing)

One PR (#6295), one commit per stage. Pin d1106deb71cc, legacy EH. Oracle:
`llc -mtriple=wasm32 -O0 -fast-isel=false -exception-model=wasm -mattr=+exception-handling [-filetype=obj]`.
Every stage is O(n) in the blocks and instructions of a function (one pass over them, no nested rescans).

| # | Stage | Pin (permalinks) | Fail-first test |
|---|-------|------------------|-----------------|
| 0 | `ExceptionModel` on the wasm driver; `wasm_exceptions` (`--wasm-exceptions`) keeps the invokes for the backend | | `the_wasm_exception_model_refuses_unwinding_and_leaves_the_rest_alone`, `wasm_exceptions_keep_the_landing_pads_for_the_backend` |
| 1 | `INTRINSIC_*` ISD nodes and `TargetCalling::intrinsic`; wasm names the 7 `llvm.wasm.*` EH intrinsics (shared: llrm-sdag `isd.rs`, llrm-codegen selectiondag) | | `a_function_the_target_names_an_intrinsic_is_an_intrinsic_node` |
| 2 | Dropped. [WasmEHPrepare.cpp:207:207](https://github.com/ali-mosavian/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/WasmEHPrepare.cpp#L207) collects `catchpad`/`cleanuppad` blocks and returns at [L223:223](https://github.com/ali-mosavian/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/WasmEHPrepare.cpp#L223) when there are none: a `landingpad` function (QB's) is untouched. Pin `llc` on `tests/eh/one_invoke.ll` prints `try`, `catch_all`, no `catch`, no `wasm.landingpad.index` | | |
| 3 | Unwind edges and EH pads in the wasm MIR. The builder's DAG is already the pin's (the pin skips `EH_LABEL` on wasm, [lowerStartEH:9133](https://github.com/ali-mosavian/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L9133)): `emit` adds the pad as a successor and an `EH_LABEL` at its head | | `an_unwind_edge_is_not_an_edge` |
| 4 | `WebAssemblyExceptionInfo`, `SortRegion` | [ExceptionInfo.cpp:86:86](https://github.com/ali-mosavian/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/WebAssembly/WebAssemblyExceptionInfo.cpp#L86), [SortRegion.h:32:32](https://github.com/ali-mosavian/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/WebAssembly/WebAssemblySortRegion.h#L32) | `an_invoke_in_a_loop_puts_the_try_around_the_loop` |
| 5 | LateEHPrepare: `catch_all`, stack pointer restore; `needsPrologForEH` in the frame | [LateEHPrepare.cpp:124:124](https://github.com/ali-mosavian/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/WebAssembly/WebAssemblyLateEHPrepare.cpp#L124), [FrameLowering.cpp:151:151](https://github.com/ali-mosavian/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/WebAssembly/WebAssemblyFrameLowering.cpp#L151) | `one_invoke_is_a_try_with_a_catch_all` |
| 6 | CFGSort: EH pads first, regions are loops or exceptions | [CFGSort.cpp:397:397](https://github.com/ali-mosavian/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/WebAssembly/WebAssemblyCFGSort.cpp#L397) | `two_invokes_share_a_try` |
| 7 | CFGStackify `placeTryMarker`, `removeUnnecessaryInstrs` | [CFGStackify.cpp:511:511](https://github.com/ali-mosavian/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/WebAssembly/WebAssemblyCFGStackify.cpp#L511), [L982:982](https://github.com/ali-mosavian/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/WebAssembly/WebAssemblyCFGStackify.cpp#L982) | `a_call_inside_a_try_that_does_not_unwind_to_it_is_refused_by_name` |
| 8 | Text and object: `try`/`catch_all`/`end_try`, `exception-handling` feature, the table the pin keeps for a personality | | `every_fixture_object_is_the_pins` (llrm-wasm `tests/object.rs`) |
| 9 | QB: `ehprepare` always runs; a handler lands by `llrm_host_land` (a JS throw) into the function's `catch_all`, which re-enters the body through a dispatch loop; host endings are traps, so `END` is not caught | | `on_error_lands_with_wasm_exceptions`, `a_host_ending_gets_through_a_catch_all` |

## Evidence at edaaadf5cd

Fixtures `crates/target/llrm-wasm/tests/eh/*.ll`; `llrm-wasm-llc -filetype=obj` with the same flags vs the pin's `llc`
(object md5, first 10 hex). Each linked with `wasm-ld --no-entry --export-all --allow-undefined --import-memory` and
instantiated under node (imports: memory, `g`).

| Fixture | Pin | Ours | Node |
|---------|-----|------|------|
| `one_invoke.ll` | 91add861b2 | 91add861b2 | instantiates |
| `two_invokes.ll` | 308860c0f5 | 308860c0f5 | instantiates |
| `loop_invoke.ll` | d185cb6d82 | d185cb6d82 | instantiates |

`call_in_try.ll` is a refusal fixture (pin: `try`/`delegate`; ours stops by name, DEVIATE W2).

Non-EH code is unchanged. `llrm-qb` over 693 `.bas` at m16, `-m32`, `-m64`, `--arm64=linux`, `--arm64=darwin`: object md5s
of the base build (385a3b1c92, binary md5 7317e9ccc1) and this head (0d5f765952) differ in 0 of 693 files on each target
(ok: 680, 665, 593, 648, 648). Live control, the head binary with `-Os`: 477, 419, 315, 132, 132 files differ.

Shared files touched outside `crates/target/llrm-wasm`: llrm-sdag `isd.rs` (+10, stage 1); llrm-codegen
`selectiondag/{builder,dagisel_adapter,target_calling}.rs`, `tests/wasm_hooks.rs`, `driver/mod.rs` (the
`wasm_exceptions` flag); the QB frontend `cli.rs`, `wasm32.rs`; `runtime/qb/posix/startup.c`, `runtime/wasm/*`, tests.
No `llrm-mir` type changed; the wasm MIR (`mir.rs`, DEVIATE W15) is the spike's own.

## DEVIATE

| Id | Difference | Exit |
|----|-----------|------|
| W2 | No `TRY_TABLE`, `DELEGATE`, `RETHROW`, so no `fixCallUnwindMismatches`/`fixCatchUnwindMismatches`: a call in a `try` that does not unwind to it is refused by name | port those passes |
| W18 | No `hoistCatches`, `catchret`/`cleanupret` replacement, `THROW_REF`, `try_table` | a frontend with typed catches or funclet pads |
| W19 | `Options::calls_unwind = false` for QB: its mid-end makes every call that can raise an `invoke`, so a plain call in a `try` never throws | port `fixCallUnwindMismatches` |

## ON ERROR programs (28 under `tests/run/qb`, built with `--wasm-exceptions` and each header's flags, run under node)

- Match their `.out` (22): bounds divmod erlnum farchk hugechk intrinsics_errors_{pds71,qb45,vbdos} onerr onrange
  ownf_err_{pds71,qb45,vbdos} range_exp range_log_trig range_on_error range_power resdiv resnext resume0
  resume_next_print_last resumelocals
- Need #6293 (`B_OEGP` import; link error now) (3): ownf_local_pds71, ownf_local_vbdos, resumelocals_local
- Do not build, unrelated (3): currency_arith_pds71, currency_convert_pds71 (x86_fp80 `LRint` in the mid-end),
  palette_using_pds71 (`Target(24586)` legalize). None touches EH.

The `setjmp` stub goes once wasm32 uses exceptions by default (after #6293 and a full wasm sweep).

## Open items

1. `unwinding()` (crates/frontends/qb/src/wasm32.rs) rewrites QB's IR text. Plan, due 2026-11-04: an IR pass `eh_reentry` in llrm-wasm on the parsed module, between the reader and selection, for `ExceptionModel::Wasm`. For each function with a `landingpad`, it wraps the body in the dispatch loop (`resume` slot, one re-entry block per pad successor that lies in the invoking region) on the module's blocks, not text. The pass keys on the CFG (a pad successor dominated by an invoke that unwinds to it), not on QB names. Then the text transform and W16's EH part go. Test: pad fixtures vs the pin's accepted forms plus the 22 ON ERROR programs and the 693-file md5 sweep. O(n).
2. `fixCallUnwindMismatches` port (removes W2 and W19). Due 2026-11-07, after item 1.
