# Retiring the old selector after the flip

Surveyed 2026-10-25 on feat/parity (parity fae9a64b3), by grep and reading; nothing built or run. "check" = not settled by reading. Gates for the flip itself: [selectiondag-default-readiness](selectiondag-default-readiness.md). Plan of record: `selectiondag-plan.md` Phase 2.

Premise: `isel.rs` is also the home of shared types, and `m64` still selects through it. What refuses at m64 under dag (w11, 2026-10-25, tests/llc: 27 C programs, native run against clang -O2): 26 ok, 0 miscompiles, 1 refusal (`vla`: DynamicStackAlloc is a libcall at m64 and at m32). Waiting on #4597 (CALL64r) and w11's va_start save area. Not covered by the 27: float/double (SysV xmm args and returns, SSE ops: refused until D64-4), aggregates by value or return, `al` for variadic calls. So integer and pointer code can flip once vla and varargs land; float code keeps the old selector, and that is what holds the deletions in section 4.

## 1. What the flip lets us delete

Lines are `wc -l`. "Callers" are non-test callers outside the file's own tests.

| Unit | Lines | Why old-only (callers) |
|---|---|---|
| `backend/isel.rs` (after the move below) | 8114 | `Selector::body`; called only by `assemble.rs:890` `selected_with` on the `Selector::Old` branch (`assemble.rs:847`, `:888`) |
| `isel/matcher.rs` | 1796 | runs `Compiled`; no DAG caller (`llrm_dagisel` has its own interpreter) |
| `isel/wide.rs`, `huge.rs`, `unwind.rs`, `expand.rs`, `combined.rs` | 1071, 177, 131, 86, 426 | called from `isel.rs` only; DAG has `legalize_types.rs`, `builder/memops.rs` (SD311 names `isel/huge.rs`), `ehprepare` |
| `backend/rmw.rs`, `comparefold.rs`, `farload.rs`, `stepflags.rs` | 691, 416, 309, 250 | only `isel/combined.rs:73`. The plan lists `farload` as "retargeted, not deleted"; no DAG caller exists today |
| `backend/addressforms.rs` | 179 | only `isel.rs:5015`; same plan note |
| `backend/valuefacts.rs` | 301 | only `isel.rs:174` and `isel_tests.rs`; `lir/src/lib.rs` mentions it in a doc (check) |
| `backend/division.rs` | 979 | only `isel.rs:5063` and `timing.rs` tests; DAG has `div_const.rs`. `arithmetic.rs` stays (`x86_lowering.rs`, `abi/qb.rs`) |
| `backend/lower_int64.rs` | 82 | `_helper` is used by `wide.rs`, `huge.rs`, `unwind.rs` and `dag_function.rs:1131` (landing pad): keep `_helper`, drop `runtime_registers` (check) |
| `backend/calleefacts.rs` read side | 369 | `isel.rs:168` reads; `assemble.rs:175,197` records for every selector. Keep the file, drop the read (check) |
| `backend/pointers.rs` | 266 | `pub mod` only; no caller at all, its header says so. Dead now, not tied to the flip (check) |
| `llrm-x86/src/isel/family.isel` + `patterns.isel` (m16 28, m32 1, m64 9) | 497 + 38 | input to `Compiled`; DAG uses `llrm-dagisel` tables |
| `llrm-x86-m{16,32,64}-select` crates, `llrm-driver/src/lib.rs:62-64`, `llrm-codegen/build.rs` selectors | - | bind `SELECTOR`; DAG never reads `Compiled`. `llrm-iselgen` stays: arm64 uses it (`llrm-arm64/Cargo.toml:24`, `tests/dialect.rs:34`) and the m16/m32/m64 `build.rs` call `build::forms` and `effect_rows` for `effects.rs` |
| `switchlower` (`llrm-transforms`, 991 + 431 test) and `Pipeline::lower_switches` | 1422 | `pipeline.rs:186` says it is for the old selector; `driver/mod.rs:456` turns it off under dag. arm64 use: check |
| `float_across_call` | - | no symbol of that name in `crates/`; readiness means a DAG refusal, not an old pass (check) |
| `legalize.rs` (skipped under `--isel=dag` since 2026-10-27: the DAG type legalizer promotes odd widths), `bitcounts`, `funnel`, `compares`, `bytes`, `fpconvert` | - | run in `driver::optimized` for every selector, gated by `arch.expands`. Plan says they go once the DAG table covers them; **unverified** |

**What the DAG path uses from `isel.rs` and must move first** (to a `convention.rs` / `selected.rs`; a pure move):

| Symbol | DAG callers (production / test) |
|---|---|
| `convention` | `dag_function.rs:591` / ~29 sites; also `examples/select_shadow.rs:94`, `builder_refusals.rs:43` |
| `convention_for`, `Convention`, `Parameter` | `builder.rs:29`, `dag_function.rs:235,257,483`, `memops.rs:17`, `tailcall.rs:13` (5 files) |
| `Selected`, `Unselected` | `dag_function.rs:58` (return type) / 4 test files; also `assemble.rs`, `frame.rs` |
| `natural_align` | `dag_function.rs:678`, `tailcall.rs:71` |
| `far` | `dag_function.rs:834`; also `assemble.rs` x3, `driver/basic.rs:1121` |
| `insn` x2, `semantics` x3 | `dag_function.rs:960-979,1138` |
| `reads_invariant_memory` | `builder.rs:1999` |
| `_read`, `_written` | `emit.rs:61` |
| `Compiled`, `m16()`, `selector()` | no DAG caller; `Options.selection` (`driver/mod.rs:58`), `flags.rs:478`, `assemble.rs:160`, `peep/tests.rs:161` |
| `FLOAT_FUNCTIONS`, `HOOKED` | **not** used by the DAG: `pricing_tests.rs:193,234` and `isel_tests.rs:5882` only |

## 2. Tests that pin old output

| Test | Verdict |
|---|---|
| `isel_tests.rs` (6917 lines, 481 tests, `isel::selected`) | delete with `isel.rs`. Port first the ones whose symptom is not selector-shaped: `far_branch_body` growth, `test_every_relocated_field_moves_with_its_instruction`, `test_dbg_lines_become_linnum`, inline asm, near globals, initializers (they go through `assemble`) |
| `assemble::assembled` (`assemble.rs:150`, hardcodes `Selector::Old`) | re-point to dag; ~80 call sites in `llrm-nib`, `llrm-qb`, `llrm-x86/src/asm.rs` pin old output |
| `regalloc_input.rs:134` (`isel::selected`) feeding `regalloc_total_tests`, `jumps_tests`, `coalesce` | port to `dag_function::selected`; their inputs change, so re-check each premise |
| `shadow_tests.rs`, `flags_tests.rs:275`, `peep/tests.rs:161`, `lifetimes_tests.rs` | port; `test_isel_takes_old_and_dag_and_refuses_the_rest` changes with the flag |
| `memsetpattern_tests.rs:36-38` | delete the "old keeps the intrinsic" assertion |
| `selectiondag/**` tests (`select_m32`, `select_f` helpers return `isel::Selected`) | keep; they depend only on the section-1 move |
| `selectiondag/golden/*.pin.txt` (52 files) | keep: they are the pin's output |
| `tools/dosbatch` (`with_isel`, `LLRM_ISEL`, `classify_isel.py`, `test_with_isel.py`), `tools/nib-build.sh:26` | keep until `--isel` is removed; then drop `old` from the choices |
| `tools/dagisel/isel_compare.py`, `m16_bytes.py`; `tools/pairs/isel_shadow.py` (old against dag) | delete at the end: they need `--isel=old`. `qcport_shadow.py`, `select_shadow.py`, `builder_corpus.py` stay |
| `codegen_slots_tests.rs:292` pins the CG1 text | change with `codegen-slot-table.md:30` |

## 3. DEVIATE rows and comments naming the old selector

| Where | Row | Exits with the deletion? |
|---|---|---|
| `isel.rs:6942` (I1, AM-I1), `:6958` (I2), `:6986` (memmove) | `rep movs` past 128 B, `-Oz`, memmove refusal | I1 is priced and live (`memcpycall.rs:12`, `above-main-rows.md:45`): the DAG expansion must carry it first (check). I2 and memmove go with the file |
| `isel/wide.rs:121` D-SB1 (`parity-pipeline.md:98`) | divisions keep the old sign-bit count | yes, if the DAG's own division lowering is the pin's. Verified 2026-10-27: `long long f(long long a,int b){return a/b;}` at m16 -Os is two inline `div` in old and a `$div64_s` call in dag; the pin llc on the same IR calls `__divdi3`. |
| `isel/huge.rs:20` | loop emitted inline, exit #2284 | yes; SD311 (`memops.rs:554`) stays but must stop citing `isel/huge.rs` |
| `llrm-x86/src/isel/family.isel:275` F1 | `arithmetic_cell` guard | yes |
| `precse.rs:55`, `precse.md:61` CSE3 | "both selectors", old reads args as memory operands | no: the DAG half stands; rewrite the text; its 2026-11-08 date may pass first |
| `precse.md:48` CSE1 | "exit with isel.rs I2" | exit condition vanishes with I2; give it a new one |
| `codegen-slot-table.md:4,30,34,87,88` (rows 2, 28, 32, 85, 86) | CG1 and "executed by isel.rs" | yes: row 28 becomes MATCH; the others cite `isel.rs` and need a DAG owner |
| `pushfold.rs:24` CF1, `pushfold.md:33` | "ours pushes at selection already" | Verified 2026-10-27 at m16 -Os: `g(*p,h,*q,k,7)` pushes `pushw 7` and `push word ptr _k` from memory in both selectors, the same instructions; the DAG carries it. |

## 4. Order

1. Delete `pointers.rs` if still unreferenced (check). Move the section-1 symbols out of `isel.rs`. Pure move; lands before the flip. Blocks nothing.
2. Re-point test helpers (`assembled`, `regalloc_input`, `peep/tests`, `shadow_tests`) to the DAG path. Needs the readiness gate (413 / 426 today) so no test falls to a refusal.
3. The flip. Blocked by readiness; m64 keeps `--isel=old`, so everything below step 4 waits on it.
4. Make m64 select through the DAG by default. This is the real blocker: it holds every deletion in `isel.rs`, `matcher.rs`, `family.isel` and the `-select` crates. The m64 refusals are those listed in the premise (xmm class D64-4, aggregates, variadic `al`, vla).
5. One week after step 4: delete `rmw`, `comparefold`, `farload`, `stepflags`, `addressforms`, `valuefacts`, `division`, `isel/{combined,expand,huge,unwind,wide,matcher}.rs`, then `isel.rs`, `isel_tests.rs`. Drop `Options.selection` and the `-select` crates; trim `llrm-iselgen` to arm64 and `build::forms`.
6. Reword the "old selector stays the default" comments (`selectiondag.rs:44`, `schedule.rs:29`, `emit.rs:35`). Remove `Selector::Old`, `--isel=old`, the `expands_for` gate (`memsetpattern.rs:44`; the pass stays), `shadow.rs` down to the report, `lower_switches`/`switchlower` (arm64: check), `LLRM_ISEL_ONLY`; delete the old-vs-dag tools. Update CG1 / row 28 and the DEVIATE rows of section 3.
7. If the DAG table covers them: `legalize.rs`, `bitcounts`, `funnel`, `compares` (plan's 2027-03 group). Unverified.
