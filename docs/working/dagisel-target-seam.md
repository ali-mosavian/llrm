# llrm-dagisel target seam — design (m68k spike owns it), 2026-10-28
Shared by m68k, 6502, wasm, aarch64. x86 outputs byte-identical (regen diff + stale test). Not built.

## Findings (x86 coupling today)
- `interp.rs`, `matcher.rs`, `dag.rs` traits (`Dag`, `Hooks`) are target-free. Generic ids (`ISD::`, `MVT::`, `TargetOpcode::`) are the pin's, equal for every target.
- X86-only: `llrm-dagisel-gen` (`main.rs` CPUS/M16/X64 consts, `X86::`/`X86ISD::` prefixes in `emit.rs:133,311`, `ids_rs.rs:29-78`, `features.toml`), `regen.sh` (X86.td, `ids.cpp.in`, `own_ids.txt`), `generated/` (one `ids.rs` for all, `x86_<cpu>.rs`), `x86_*.rs` hooks/matchers inside llrm-dagisel, generated code calls `crate::predicates`.
- Codegen side (second seam): `selectiondag/dag_select.rs` and `dag_function.rs` name `X86Hooks` and `x86_*` tables; `dagisel_adapter.rs` maps ISD and registers with `ids::x86`; `emit.rs` maps machine nodes to `x86.instr` forms.

## Seam 1: tables (generator)
1. **Target record** `crates/target/llrm-<t>/dagisel.toml`: `name`, root `td` + include dirs, `isd_prefix` (`M68kISD::`), `ns` (`M68k::`), ISD header for ids (`M68kISelLowering.h`), `own_ids` (6502: no LLVM header), `hw_modes`, `cpus` (name, features), `[atom]` rows, node-predicate map, `out_dir`.
2. `llrm-dagisel-gen --target <record>` replaces the consts and prefixes by record fields; no `--target` = today's x86 path, unchanged.
3. Output goes to the **target crate** (`llrm-<t>/src/dagisel/generated/`): `ids.rs` (target ISD + machine ids + registers), `instrinfo.rs`, `<t>_<cpu>.rs`. Generated code takes the generic paths from `llrm_dagisel::{dag,matcher}` and the target predicates from the target crate (`--crate-path`). llrm-dagisel never depends on a target.
4. `tools/dagisel/regen.sh <t>` (default x86): runs the pin's tblgen `-gen-dag-isel`, `-gen-instr-info`, builds ids by the `ids.cpp.in` trick from the record; stale-table test per target.
5. Per target by hand: a `Hooks` impl (pattern predicate dispatch, complex patterns, xforms, fold legality) and its matchers, in the target crate. Written once per target; m68k: `SelectARI/ARIPI/ARIPD/ARID/ARII/AL/PCD/PCI/PCIBD`.

## Seam 2: codegen binding (needs the sdag owners)
A small trait in `llrm-target`/codegen (`DagTarget`: pin ids of target nodes, register id map, `Hooks` factory, selector table per cpu, form map for the emitter) replaces direct `X86Hooks`/`x86_*` use in `dag_select.rs`, `dag_function.rs`, `dagisel_adapter.rs`, `emit.rs`. x86 implements it first with no output change. Edits to files other sessions own: order and scope to be set by oversight/coordinator before any edit.

## SD403 and what S2 carries (checked 2026-10-28 in the tree)
- The SD403 header in `dag_function.rs` is stale on x86: `builder.rs` has `lower_call`, `builder/{switch,tailcall,codecall}.rs`, and `dag_function_{calls,switch,tail,frame}_tests.rs` exist. Refusals left are specific shapes (landing pads, non-constant allocas, register args outside the entry block).
- The gap for other targets is the hard-wired x86 gate: `selected_in` refuses any bitness but 16/32/64 and builds `TargetLowering::x86_*`, `X86Hooks`, `x86_*` tables; `x86_lowering.rs` (2.2k lines) is the x86 `X86TargetLowering`.
- So S2 = the trait + per-target action table (`TargetLowering` rows from the record) + target `LowerCall/FormalArguments/Return` over `Convention::place` + emitter form map. Not new multi-block/call/switch machinery, if the builder's call path is target-free; verify with an m68k probe at S2 start. Size guess: 4 edited files (~300 changed lines) + trait (~200) + m68k lowering (~1.5k, mine, new file).
- Root `.td` may live in the target crate (6502: `crates/target/llrm-6502/*.td`, include dir = pin's `llvm/include`); `own_ids.txt` lives in the target crate, read by regen and by the lowering (one source).

## Probe for M3 (2026-10-28)
- The DAG vocabulary (`MVT`, `ISD`, `TargetLoweringInfo`, `TargetLowering` action tables) lives in `llrm-x86/src/dag`, re-exported by codegen's `selectiondag.rs`. It is generic in shape (`TargetLowering::empty(types)`, `set_operation_action`, ...) with x86 constructors beside it. A non-x86 target must depend on `llrm-x86` for it until it moves to a neutral crate (layout item for S2).
- `legalize_dag::Hooks::lower_operation` and `CombineHooks` are pub traits: the m68k lowering can be a new module in llrm-m68k with no edit to the sdag files; only the binding in `selected_in` needs S2.

## Order
S1a generator + regen for M68k alone (new files, no x86 file touched) → S1b x86 through the record, byte-identical → S2 trait. m68k work (lowering, hooks) proceeds on S1a.

## Open
- aarch64: complex patterns and SD403 gaps (mac2 to check). 6502: no hw modes, own ids. wasm: no hw modes.

## S1a result (2026-10-28)
- `llrm-dagisel-gen --target crates/target/llrm-m68k/dagisel.toml` decodes the M68k table (6710 items m68000, 6778 m68020); `add` selects ADD32dr, the pin's llc agrees.
- x86 regen output byte-identical (diff -rq). Stale test for every `crates/target/*/dagisel.toml`.
- Found: `Hooks` carries x86-only methods (`has_sse_unaligned_mem`, `is_def32`, `check_tc_ret_enough_regs`, ...); every target implements them. S1b: move them into an x86 extension trait or give defaults (dag.rs of llrm-dagisel).
