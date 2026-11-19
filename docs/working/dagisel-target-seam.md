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
- S2 as built on `feat/m68k-spike` (10-29): the shared boundary is IR -> scheduled DAG; emission stays per target (m68k emits into its own machine function, not x86 LIR). Done: `PinRegisters` (adapter) and `SchedInfo::pin_register` take the target's registers; `do_instruction_selection_with` takes the selector; `llrm-m68k` `pipeline.rs` runs combine, legalize, select and schedule with its own tables, and a hand-built `a + b` selects `MOV32rp, ADD32dp, RET` as the pin does (x86 defaults unchanged, schedule/dag_select/dagisel tests green).
- Left for S2: the builder's call, return and formal-argument code is X86's `LowerCall`/`LowerReturn` (x86isd::CALL, m16 views); it needs a `BuilderTarget` hook to `llrm-m68k` `call.rs` (thin dispatch lines in `builder.rs`), and a `BuilderTarget::m68k`. The `DagTarget` trait that replaces `selected_in`'s bitness gate is extracted from the two implementations after that (rule of two), not before. `emit.rs` is not part of S2.
- Root `.td` may live in the target crate (6502: `crates/target/llrm-6502/*.td`, include dir = pin's `llvm/include`); `own_ids.txt` lives in the target crate, read by regen and by the lowering (one source).
- Left for S2 (2026-10-29): the shared legalizer has no libcall conversion (`RuntimeLibcalls`: mul/div/rem/shift without the instruction become calls in legalize). 6502 emits runtime-call pseudos at emission instead; m68k needs it for 68000 i32 mul/div, wasm does not. After M5. The call/return/argument hook is done: `selectiondag::target_calling::TargetCalling` (a2bcbe44f6).
- Left for S2 (2026-10-29): m68k runs the mid-end as loop strength reduction only (as `llc`), so `llrm-m68k` loses to gcc by one instruction where gcc peels a loop's first test (`bench/kernels/strlen.ll`, -O2: 45 vs 44) and where it rotates a loop (`gcd.ll`: 17 vs 16, loop rotation belongs to the neutral-LIR task). Owner: whoever takes the full `-O2` pipeline once the legalizer takes `i33` (next entry).
- Left for S2 (2026-10-29): the shared legalizer cannot expand an `i64` add or subtract on a 32-bit target without a carry operation. The full `-O2` closes the `sum` loop (`bench/kernels/sum.ll` in llrm-m68k) into `i33` arithmetic (`opt -O2`, pin d1106deb71cc, shown here trimmed); `llc` promotes `i33` to `i64` and expands the add to a carry pair (`UADDO_CARRY`, M68k `ADDX`). Ours stops at the add. Repro, any 32-bit target with `n8:16:32`:
  ```
  define i32 @k(i32 %n) {
    %smax = tail call i32 @llvm.smax.i32(i32 %n, i32 1)
    %0 = zext nneg i32 %smax to i33
    %1 = add nsw i33 %0, -1
    %2 = add nsw i32 %smax, -2
    %3 = zext i32 %2 to i33
    %4 = mul i33 %1, %3
    %5 = lshr i33 %4, 1
    %6 = trunc nuw i33 %5 to i32
    %7 = add i32 %smax, %6
    %8 = add i32 %7, -1
    ret i32 %8
  }
  declare i32 @llvm.smax.i32(i32, i32)
  ```
  `cargo run -p llrm-m68k --example asm -- FILE k` (with the datalayout line) fails: `Legalize("Unsupported { op: Add, ty: Simple(I64), what: \"expand add or subtract without a carry operation\" }")`. Not an m68k matter: the target marks no carry op, and the legalizer should expand through `UADDO_CARRY`/`ADDC`/`ADDE` as `llc` does. m68k runs LSR only until this lands (`llrm-m68k` `compile::mid_end`).

## Probe for M3 (2026-10-28)
- The DAG vocabulary (`MVT`, `ISD`, `TargetLoweringInfo`, `TargetLowering` action tables) lives in `llrm-x86/src/dag`, re-exported by codegen's `selectiondag.rs`. It is generic in shape (`TargetLowering::empty(types)`, `set_operation_action`, ...) with x86 constructors beside it. A non-x86 target must depend on `llrm-x86` for it until it moves to a neutral crate (layout item for S2).
- `legalize_dag::Hooks::lower_operation` and `CombineHooks` are pub traits: the m68k lowering can be a new module in llrm-m68k with no edit to the sdag files; only the binding in `selected_in` needs S2.

## Numbered rows
SD500 (llrm-m68k depends on llrm-x86 for the DAG types) is closed: the types moved to llrm-sdag (aarch64 branch P1), merged 2026-10-28.

## Order
S1a generator + regen for M68k alone (new files, no x86 file touched) → S1b x86 through the record, byte-identical → S2 trait. m68k work (lowering, hooks) proceeds on S1a.

## Open
- aarch64: complex patterns and SD403 gaps (mac2 to check). 6502: no hw modes, own ids. wasm: no hw modes.

## S1a result (2026-10-28)
- `llrm-dagisel-gen --target crates/target/llrm-m68k/dagisel.toml` decodes the M68k table (6710 items m68000, 6778 m68020); `add` selects ADD32dr, the pin's llc agrees.
- x86 regen output byte-identical (diff -rq). Stale test for every `crates/target/*/dagisel.toml`.
- Found: `Hooks` carries x86-only methods (`has_sse_unaligned_mem`, `is_def32`, `check_tc_ret_enough_regs`, ...); every target implements them. S1b: move them into an x86 extension trait or give defaults (dag.rs of llrm-dagisel).

## Pin bug: `a[i]` of a stack array is stored and loaded as `a[0]` (checked 2026-10-29)

Pin d1106deb71cc, `llc -mcpu=M68000`, `-O0` and `-O2` give the same result. Input:

```llvm
define i32 @f(i32 %i) {
  %a = alloca [4 x i32], align 4
  ; four stores of 10, 20, 30, 40 to a[0..3]
  %q = getelementptr [4 x i32], ptr %a, i32 0, i32 %i
  %v = load volatile i32, ptr %q
  ret i32 %v
}
```

Output (`-O2`): the four stores to `(0..12,%sp)`, then `move.l (0,%sp), %d0`. `%i` is never read.

Cause: `M68kISelAddressMode::hasIndexReg` is false for a frame-index base
([L103](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/M68k/M68kISelDAGToDAG.cpp#L103),
it requires `RegBase`), so the matcher accepts an index beside the frame index, and `getFrameIndexAddress`
([L323](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/M68k/M68kISelDAGToDAG.cpp#L323),
used at [L827](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/M68k/M68kISelDAGToDAG.cpp#L827))
returns the displacement and frame index without the index, before the `hasIndexReg` rejection at L832.
No lit test under `llvm/test/CodeGen/M68k` indexes a stack array. Ours: `match_address_base` in
`target/llrm-m68k/src/dagisel/addr.rs` (DEVIATE); the run test is `a_local_array_is_a_frame_object` in
`tests/results.rs`. Whether to report it upstream is the user's call.

## Open: shared code that still assumes little-endian (grep 2026-10-29)

`DagConfig::big_endian` (default false, so x86 is unchanged) covers `trunc (load)`, `srl/and (load)` narrowing
(`reduce_load_width.rs`) and store-to-load forwarding (`combine_load.rs`). Not yet:

- `legalize_types.rs` `expand_load` / `expand_store` (and the `Mem` offsets of `expand_*` for i64): low half at +0,
  high at +4. An i64 load on m68k would swap its halves.
- `combine_store_merge.rs` (L383): merges adjacent stores with the lowest address as the lowest bits.
- `dag_combiner.rs` L841: a load of the same address read as little-endian when a store is forwarded.
- Not looked at: `match_rotate.rs`, `bswap_combine.rs` and `combine_funnel.rs` (load-based forms).

Each is a bug on m68k the moment a kernel uses i64 or merged stores; none is hit by the two bench sets.

## S2 plan: `DagTarget` (2026-10-30)
**Goal.** One shared IR -> scheduled-units driver, generic over a target; x86 is the first implementation, output byte-identical. Emission stays per target.

**Why now.** Two drivers do the same walk: `dag_function.rs::selected_in` (x86) and `llrm-m68k/src/dagisel/{function,pipeline}.rs`. Both look the function up, parse the datalayout, build the convention, `build_block` each block (and its `extra` blocks), then combine, legalize, select and schedule it.

**Trait** (in `selectiondag/`, one file): `lowering_info() -> TargetLowering`, `builder() -> BuilderTarget`, `selector() -> Selector`, `hooks() -> impl Hooks`, `legalizer() -> impl legalize Hooks + CombineHooks`, `pins() -> &dyn PinRegisters`, `sched_info()`, and `preprocess(dag, frame)` with a no-op default (x86: `x86_preprocess`).

**Shared driver.** `select_units(dag, &T)`: today's `select_block` minus `emit_in` (legalize, scratch slots, preprocess, assertions, select, schedule). x86's `select_block` becomes `select_units` + `emit_in`. `selected_in` keeps alloca layout, frame, emission and the bitness gate, which picks `X86Target::{m16,m32,m64}`.

**Files.** New: `selectiondag/dag_target.rs`. Edited: `dag_select.rs` (generic `select_units`), `dag_function.rs` (build the `X86Target`, call the driver; about 60 lines move), `x86_lowering.rs` (impl the trait pieces; no logic change). Not touched: `emit.rs`, `dagisel_adapter.rs` (no S2 hunk).

**Second implementation.** The m68k crate is not on release/next, so the PR ships the trait and x86 only. The m68k impl is checked on `feat/m68k-spike` (it replaces `pipeline.rs`'s body) and lands with the m68k backend PR.

**Overlap.** w15's unpushed `w15/shared-legalizer` (fc2230f1f3, stack #2-#4) edits `dagisel_adapter.rs`, `builder.rs`, `target_calling.rs`, `dag_combiner.rs`, `legalize_types.rs`, `llrm-sdag/src/{isd,lowering}.rs`. S2 does not touch those files; `dag_select.rs` and `dag_function.rs` are in neither diff (checked with `git diff --stat`). Textual conflict: none expected. Semantic: `ShiftAmount::Fixed` and the compare-type hook are read from `TargetLowering`, which the trait only forwards.

**Proof.** The sweep (`the sweep directory of the handoff`, 1472 rows) against the parent, md5-stamped. Control: same tree with the x86 scheduler preference flipped (`Info { source_order }`), which must move rows. Tests: `llrm-codegen` lib tests and a unit test that `select_units` over a hand-built DAG equals today's `select_block` units for x86.
