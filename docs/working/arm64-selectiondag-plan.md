# arm64 selection from the pin's AArch64 TableGen: plan

Repo `llrm-mac2` @ `alim/spike/arm64-parity-next` (HEAD 4d8d9f338), pin d1106deb71cc (`~/work/other/llvm-pin`, build has `llvm-tblgen`, `llc`, `clang`, targets AArch64;X86). Read-only survey 2026-10-28. "unverified" marks what was not measured.

## Decision first

"The way x86 is done" is **not** `.instr`/`.isel`/`.peep` + `llrm-iselgen`. That is the *old* x86 selector, being retired (`docs/working/old-selector-retirement.md`: `family.isel`, `isel/matcher.rs`, the `-select` crates go after the flip). The table-driven x86 path is the **SelectionDAG port**: the pin's own `llvm-tblgen -gen-dag-isel` table, decoded and interpreted in Rust. arm64 should join that path. Writing more `patterns.isel` (55 lines today, read only by `tests/dialect.rs`) builds on the format x86 is deleting.

Consequence: the x86 leaks in `arm64-spike.md` (RegId = iced, Flag = RflagsBits) are **not on the critical path**. The DAG path keeps its own virtual values until its emitter; arm64 gets its own emitter into `mfunc::Op`, as the spike already decided (own backend from MIR). The shared-layer work is in the DAG crates, not in LIR.

## (1) x86 today: .td to selection at run time

| Step | Where | Size |
|---|---|---|
| a. pin `llvm-tblgen -gen-dag-isel X86.td` (+ our `m16.td`) -> `X86GenDAGISel.inc` | `tools/ci/dagisel/regen.sh` (HEAD: `tools/dagisel/`, deleted in tree; untracked copy now in `tools/ci/dagisel/`) | .inc 11.3 MB, table 639,106 B |
| b. `-gen-instr-info` -> `instrinfo.txt` (`instrinfo.py`) | same | |
| c. symbol numbers: compile `ids.cpp.in` against the pin's **X86** headers, run it -> `ids.txt` (+ `own_ids.txt`, `extra_symbols.txt`) | same | |
| d. `llrm-dagisel-gen`: `inc` parse -> `decode` to `Matcher` tree (HwModes) -> `prune` by `features.toml` CPU -> `flatten` -> `emit` Rust | `crates/backend/llrm-dagisel-gen/src` (77 KB, 10 files) | |
| e. checked-in tables | `crates/backend/llrm-dagisel/src/generated/` `x86_i486.rs` 603 KB (15,108 items), `x86_p5.rs`, `x86_m16.rs`, `x86_x64.rs` 791 KB (20,208), `ids.rs` 48 KB, `instrinfo.rs` 770 KB | 3.4 MB |
| f. stale test regenerates and diffs | `llrm-dagisel-gen/tests/stale.rs` (still calls `tools/dagisel/regen.sh`: broken by the move) | |
| g. run time: MIR -> per-block DAG (`builder.rs`, SD1) -> combine (`dag_combiner.rs` + visitors) -> `legalize_types.rs` -> `legalize_dag.rs` + `x86_lowering.rs` -> select: `dag_select.rs` -> `dagisel_adapter.rs` (codegen DAG as `llrm_dagisel::Dag`) -> `interp::Selector` over the table, with `Hooks` (`x86_hooks.rs`, `x86_addr.rs` 40 KB, `x86_rmw`, `x86_mul`, ...) -> `schedule.rs` (BURR) -> `emit.rs` (machine node -> x86 LIR via `opcodes.rs` name map to `x86.instr` forms) | `crates/codegen/llrm-codegen/src/backend/selectiondag/` (~54 K lines incl. tests), `llrm-sdag/src/` (ISD, MVT, actions, X86ISD; moved out of `llrm-x86` by P1) | |

Reusable for arm64 as is: `matcher.rs`, `interp.rs` (SelectCodeCommon), `dag.rs` traits (minus 6 x86 methods on `Hooks`), decoder/flatten/emit of the generator, the codegen DAG, builder (MIR in), combiner core, both legalizers, scheduler, `dump.rs`. MVT/ISD numbers are the pin's and target-independent, and now live in `llrm-sdag/src/{mvt,isd}.rs` (P1).

x86-specific: `ids.cpp.in` (X86 headers), `features.toml`, CPU lists hard-coded in `main.rs`, one flat `generated::ids` mixing ISD/MVT/X86 numbers, `instr::operand_type::X86_COND_CODE`, `Hooks::{has_sse_unaligned_mem, should_avoid_immediate_inst_forms_for_size, is_unneeded_shift_mask, is_sext_absolute_symbol_ref, has_no_carry_flag_uses, is_def32}`, `predicates.rs` (x86 PatFrag bodies), all `x86_*.rs`, `x86_lowering.rs`, `x86_carry.rs`, `x86_preprocess.rs`, `BuilderTarget` comments/defaults (`shift_amount: i8`), `emit.rs` (x86 LIR, 5-operand addresses, `x86_addr::AddrMode`), `opcodes.rs`, `TargetLowering` fields `cmov`/`keeps_f64_constants`.

## (3) How much of AArch64 is mechanical (measured)

`llvm-tblgen --dump-json AArch64.td` on the pin (103 MB, 8 s). Categories are a coarse filter on `Predicates` and on vector types in the pattern; upper bounds, not exact.

| | total | SVE/SME | NEON/vector | scalar int/mem/control | scalar FP | scalar extensions (LSE, RCPC, CSSC, PAuth, CRC, ...) | pseudo/codegen-only |
|---|---|---|---|---|---|---|---|
| `Instruction` | 9,252 | 4,024 | 2,006 | 589 | 275 | 590 | 1,768 |
| ...with in-def `Pattern` | 2,118 | 175 | 1,332 | 246 | 235 | 130 | |
| `Pat` | 12,803 | 8,059 | 3,252 | 451 | 597 | 444 | |

`AArch64GenDAGISel.inc` (already in the pin build): 10.5 MB, table 625,704 B (x86: 639,106); 154 pattern predicates, 179 node predicates, 173 complex-pattern cases, 43 xforms. AArch64ISD SDNodes: 382, numbered by the generated `AArch64GenSDNodeInfo.inc` (no hand header needed, unlike X86ISD). Scalar pruned table size: **unverified**, estimated 4-8 K items (x86 i486 kept 15 K).

Mechanical (the table does it once hooks exist): every scalar ALU form incl. shifted/extended operands (`SelectArith/LogicalShiftedRegister`, `SelectArithExtendedRegister`), immediates (`SelectArithImmed`, `SelectNegArithImmed`, logical-imm xforms), `madd/msub/smaddl/umulh`, `csel/csinc/csinv/csneg` from `AArch64ISD::CSEL` (incl. `cinc` shapes), `fnmul/fmadd/fnmadd` (contract-gated), `ubfx/sbfx/ubfiz` from shift+and Pats, `cbz/tbz` from `brcond` Pats, loads/stores in all modes via 21 scalar `SelectAddrMode*` complex patterns, FP conversions incl. fixed-point (`SelectCVTFixedPos*`), atomics with LSE.

By hand in the pin, must be hooks (translate, permalink, pin order):

| Pin code | Lines (pin) | Covers today's hand file |
|---|---|---|
| `AArch64DAGToDAGISel` scalar part: complex patterns `SelectArith*`, `SelectAddrMode{Indexed,Unscaled,WRO,XRO}` (L777-1865), `tryIndexedLoad`, bitfield `tryBitfieldExtractOp*`/`tryBitfieldInsertOp*`/`getUsefulBits` (L2853-4145), `tryShiftAmountMod`, `SelectCVTFixedPos*`, `SelectCMP_SWAP`, `Select` (L5194) scalar arms, `PreprocessISelDAG` | ~2,500 of 8,382 | `index.rs`, `shifted.rs`, `mem.rs`, `fixedpoint.rs`, `atomic.rs` |
| `AArch64TargetLowering` ctor (actions) + `LowerOperation` scalar arms: `LowerSETCC`, `LowerBR_CC`, `LowerSELECT_CC`, `getAArch64Cmp`/`emitComparison`/`emitConditionalComparison`/`emitConjunction[Rec]` (L4025-4709, ccmp), `LowerGlobalAddress` (Darwin GOT / ELF adrp), `LowerVASTART`/`LowerVAARG` (Darwin vs AAPCS), `LowerFunnelShift`, `LowerXALUO`, i128 parts | ~4,000 of 36,535 | `conjunction.rs`, `chain.rs`, `control.rs`, `val.rs` |
| `LowerCall`/`LowerFormalArguments`/`LowerReturn` (+ `AArch64CallingConvention.td`, 814 lines; `-gen-callingconv` exists in the pin build) | ~1,500 | `call.rs` (826), `agg.rs` |
| `PerformDAGCombine` scalar visitors: `performMulCombine` (L21531, mul by constant -> shifts/adds), `performAddSubCombine`, `performAddCSelIntoCSinc`, `performNegCSelCombine`, `performANDCombine`/`performORCombine` (BFI), `performCSELCombine`, `performBRCONDCombine`, `performSETCCCombine`, `performCONDCombine`/`performSubsToAndsCombine`, `performTBZCombine`, `performLOADCombine`/`performSTORECombine`, `performExtendCombine`, `performTruncateCombine` | ~3,000 | `int.rs` (1,187), `widemul.rs`, `divide.rs` (BuildSDIV is generic DAG code, already ported for x86: `div_const.rs`) |
| Not DAG at all in the pin: `AArch64ConditionalCompares` (1,098), `MachineCombiner` (madd: the pin's finalize-isel has `MADDWrrr ..., $wzr` + `ADDWrr`, fused later), early if-conversion (ARM64-EIC-1), `AArch64LoadStoreOptimizer` (`pairs.rs`), `AArch64ExpandPseudoInsts` (`expand_imm.rs`) | | stays post-isel; the hand selector does some of these *at* isel, so a pure DAG port loses them until the machine pass exists |

## (2) Shared-layer changes, ordered small PRs

Each: fail-first test, measurable exit. No behaviour change for x86 in P0-P4 (gate: x86 `stale.rs`, `dagisel_tests`, `select_shadow` m32 bucket table identical).

| # | Change | Test / oracle | Exit |
|---|---|---|---|
| P0 | Fix `stale.rs` and `regen.sh` paths for the `tools/ci/dagisel` move (coordinate with the session that moved it) | stale test runs | test passes against the pin, fails with a hand-edited table |
| P1 | Move `mvt.rs`, `isd.rs`, generic `actions.rs`/`lowering.rs` out of `llrm-x86` to a target-neutral home (`llrm-target` or a new `llrm-selectiondag` crate); `TargetLowering`'s `cmov`/`keeps_f64_constants` become generic fields named as the pin names them | pure move; `cargo build` both targets | `llrm-arm64` names no `llrm_x86` |
| P2 | `llrm-dagisel`: `generated::{common (MVT, ISD, OPC), x86, aarch64}`; per-target `instrinfo`; `operand_type` per target | x86 tables regenerate byte-identical | diff of regenerated x86 tables empty |
| P3 | `Hooks` split: generic trait + `X86Hooks` extension for the 6 x86 methods; predicate dispatch by table module | x86 tests | `dag.rs` names no x86 concept |
| P4 | `llrm-dagisel-gen --target {x86,aarch64}`: CPU lists and HwModes from `features.toml` per target, ids program built against the target's headers (`AArch64GenInstrInfo.inc`, `AArch64GenSDNodeInfo.inc`) | x86 byte-identical; aarch64 decodes | `aarch64_<cpu>.rs` checked in; stale test covers both |
| P5 | AArch64 `features.toml`: `apple-m1` / `generic` with FPARMv8, NEON on (scalar patterns use FPR forms and some NEON ones: `CTPOP`, i64<->f64 bitcasts), SVE/SME/streaming atoms false | `TestDag`: `(add i32, i32)` -> `ADDWrr`, `(add x, (shl y, 3))` -> `ADDXrs` once the shifted-register hook exists, no SVE opcode in the pruned table | count of SVE opcodes in table = 0 |
| P6 | Emitter interface: `emit.rs` behind a `MachineSink` trait (x86 LIR impl unchanged); `llrm-arm64` implements it into `mfunc::Op` with a pin-opcode -> `Inst` table (from `instrinfo` operand lists + a family map: `ADDWrs` -> `AddSubShifted{W}`) | every opcode the pruned table emits maps or is listed unmapped; the encoder's `llvm-mc` byte tests extend to one row per mapped opcode | unmapped list empty for the scalar subset |

## (4) Migration without regressing

Mirror x86 Phase 0: `llrm-arm64-llc --isel=dag` (shadow), hand selector default.

1. **Pin oracle per function**: port `select_shadow.py` to `-mtriple=arm64-apple-macos` / `aarch64-linux-gnu`: our machine nodes after selection vs the pin's `llc -O2 -stop-after=finalize-isel` (works on the pin build, checked). Buckets: refused (by reason), unselected node, opcode differs, equal. Target: equal >= 90% of functions in `tests/programs` + `tests/darwin` before any flip.
2. **Self oracle**: both selectors on every function; `tools/targets/arm64_compare.py` (code bytes, executed instructions under qemu) per program for hand vs dag. Rows where dag is worse are numbered, not hidden.
3. **Correctness**: the 22 `tests/programs` and the `tests/darwin` fixtures under `--isel=dag` (qemu and Mac), plus the 21 crate tests (`conditional_compare.rs`, `shifted_operands.rs`, `index_modes.rs`, `multiply_constants.rs`, `signed_divide.rs`, ...) run with both selectors: they are the regression tests for each hook.
4. **Fallback inside shadow only**: a block the DAG refuses selects through the hand selector, counted. Flip when refusals = 0, miscompiles = 0, executed-instruction geomean dag <= hand (the caller's 1.014x vs clang must not rise; that number is **not in the repo**: `arm64-spike.md` shows 1.14x backend / 1.62x whole, `arm64-patterns.md` 1.13x/1.21x; reconcile which instrument is current before using it as the gate).
5. **Per hand file**: when its hook lands and its tests pass on dag, the file goes in the same PR as the flip of that shape (deleting code is the evidence). Machine-level wins the hand selector makes at isel (madd fusion, early if-conversion, ccmp chains beyond `emitConjunction`) stay as hand pre/post passes until the pin's machine pass is ported; list each as a numbered DEVIATE.

## (5) Risks, effort, first step

Risks:
- **Machine passes**: the pin gets `madd`, `csel` chains (early ifcvt), `ldp/stp` after isel. A DAG-only port measured against the hand selector will look worse; compare at finalize-isel against the pin, end-to-end against hand. Doubt any regression number until the machine pass on both sides is the same.
- **Shared-crate churn**: `selectiondag/` is the busiest x86 area (sdag-a..d). P1-P3 touch it; land as pure moves, regenerate-identical, small.
- **Emitter mapping**: `Inst` is an encoding-class enum, not pin opcodes; the map is hand data (~300 scalar opcodes, unverified). Alternative: generate encoding from `AArch64GenMCCodeEmitter.inc`; bigger, decide after P6 counts.
- **Calls/varargs/Darwin**: `call.rs` is 826 lines of measured behaviour; the pin's `LowerCall` is larger and Darwin-specific. Highest regression risk.
- **Compile time**: the DAG path is heavier than one forward walk (spike: backend 5.8 M instr vs llc 25.5 M). Measure per function at P6.
- **Docs stale**: `arm64-patterns.md` still says `llrm_target::instr` names x86 and patterns wait for `.isel` operand kinds; the dialect (`dialect.toml`, `h`/`e`/`c` shapes, NZCV) has landed. Update both arm64 docs with the decision above (rule 8).

Effort, in PRs: P0-P6 shared plumbing **7**; first function end to end (actions ctor, `LowerOperation` minimum, scalar complex patterns, emitter) **5**; parity hooks (conjunction/ccmp, mul/add/csel combines, bitfield, addressing WRO/XRO + indexed, calls + Darwin varargs, FP + fixed point, atomics, i128) **15-20**; shadow tooling + flip + deletions **4**. About **30-35 PRs**. Unverified: the pruned table size and the opcode-map count, which move the middle estimate.

Do first: **P4 dry run, no commit**: run `llrm-dagisel-gen`'s decoder on `AArch64GenDAGISel.inc` with an AArch64 ids file and the scalar feature set; record the pruned item count, the scalar opcode set the emitter must map, and which predicates/complex patterns survive. That one measurement sizes P6 and the hook list and decides between the hand opcode map and a generated encoder. Then P0 (the stale test is broken in the tree today).

## Dry run (measured 2026-10-28, `examples/a64_dry.rs`, `aarch64/ids.sh`, `features_aarch64.toml`)

The generator's decoder reads the pin's `AArch64GenDAGISel.inc` (625,704 B; 154 pattern predicates, 179 node predicates, 173 complex patterns, 43 xforms) after two additions: `OPC_CaptureDeactivationSymbol` and `OPC_EmitCopyToRegTwoByte` (registers past 255). Pruned for an Apple M1 scalar subset:

| features | items | operand entries | distinct emitted opcodes |
|---|---|---|---|
| NEON on, vector value types pruned | 80,202 | 530 | 1,134 |
| NEON, SVE, SME off (scalar) | 22,506 | 446 | 930 |

The scalar table is 1.5x x86 i486's 15 K, not the 4-8 K guessed; 930 emitted opcodes include COPY_TO_REGCLASS, REG_SEQUENCE and register-class ids, so the opcode map to write is smaller. 14 atoms needed a verdict beyond `hasX()`; five are per-function (`shouldOptForSize`, `hasOptNone`, `branchTargetEnforcement`, `branchProtectionPAuthLR`, `hardenSlsBlr`) and sit as constants `false` until hooks exist. Still to do: which of the 173 complex patterns and 179 node predicates survive in the scalar table.

Decision (user, 2026-10-28): SelectionDAG from the pin's `AArch64.td`; GlobalISel and a GCC-style machine description were considered and not taken (GlobalISel is `-O0` only in the pin and 1.07x larger at `-O2`; GCC's `.md` would reproduce GCC's choices, not the pinned clang's).

## GlobalISel alternative

Answer: no. The pinned clang does not use GlobalISel at -O2. At -O2 the pin's GlobalISel output is measurably different from, and larger than, the code we are trying to match.

### (1) The pin's AArch64 GlobalISel path

The default is set in `AArch64TargetMachine.cpp` L160: `aarch64-enable-global-isel-at-O` = 0. GlobalISel is used at -O0 only, unless `-global-isel` is passed. Checked with the pin's tools on the MIR properties after `-stop-after=finalize-isel`:

- `llc -O0`: `legalized/regBankSelected/selected: true`.
- `llc -O1/-O2/-O3`: `false` (SelectionDAG).
- `clang -O2 -target arm64-apple-macos`: `false`.

So the earlier `MADDWrrr …, $wzr` finalize-isel output came from **SelectionDAG**. The IRTranslator pass is listed in the -O2 pass structure, but it does not select those functions.

Sizes, pin lines:

| Part | Lines |
|---|---|
| `AArch64InstructionSelector.cpp` | 8,477 |
| `AArch64LegalizerInfo.cpp` | 2,873 |
| `AArch64CallLowering.cpp` | 1,603 |
| `AArch64RegisterBankInfo.cpp` | 1,527 |
| `AArch64PostLegalizerLowering.cpp` | 1,416 |
| `AArch64PostLegalizerCombiner.cpp` | 1,011 |
| `AArch64PreLegalizerCombiner.cpp` | 937 |
| `AArch64PostSelectOptimize.cpp` | 327 |
| `AArch64GlobalISelUtils.cpp` | 223 |
| `AArch64/GISel/` total | 19,044 |
| generic `CodeGen/GlobalISel` (IRTranslator, Legalizer + LegalizerHelper, RegBankSelect, InstructionSelect, CombinerHelper, CallLowering, ...) | 42,036 |

Generated files:

- `AArch64GenGlobalISel.inc`: 18.2 MB. `MatchTable0` is 608,051 B, against the DAG table's 625,704 B.
- Combiner tables: PreLegalize 582 KB, PostLegalize 269 KB, PostLegalizeLowering 50 KB, O0 47 KB.

### (2) Same code as clang?

Method: pin `llc -O2` with and without `-global-isel -global-isel-abort=1` on all 84 `.ll` files in `crates/target/llrm-arm64/tests/{darwin,data}`. Darwin files used `arm64-apple-macos`, data files `aarch64-linux-gnu`. GlobalISel aborted on none.

Counts are static instructions in the `.s` files, not executed instructions (**executed not measured**).

- Totals: SelectionDAG 7,872 instructions, GlobalISel 10,049.
- Geomean ratio GlobalISel/SelectionDAG: 1.072. Per file: 1 better, 36 equal, 47 worse.
- Worst files: `division` 1.65 (2,421 → 3,999), `bits` 1.55, `memory_expansion` 1.43, `arith` 1.21, `multiply_constants` 1.19 (`mul` 4 → 14).

Per-feature counts, SelectionDAG/GlobalISel:

| Instruction | SelectionDAG/GlobalISel |
|---|---|
| ccmp | 35/11 |
| csel | 28/18 |
| cset | 11/38 |
| cinc | 2/0 |
| ubfiz | 4/0 |
| msub | 108/2 |
| madd | 18/127 |
| cbz | 30/30 |
| cbnz | 12/12 |
| tbz | 3/4 |
| ldp | 249/263 |
| stp | 378/373 |

How the gaps split:

- **SelectionDAG-only:** multiply-by-constant (`performMulCombine`), `emitConjunction`/ccmp, csinc/cinc combines and bitfield matching. GlobalISel does each weaker or not at all.
- **The same on both (post-isel machine passes):** cbz/tbz and ldp/stp. madd fusion is the MachineCombiner, shared by both paths.
- **Addressing modes:** both have them (complex patterns vs `selectAddrMode*` in the InstructionSelector). Not compared per form.

### (3) What llrm has

llrm has no GlobalISel-shaped layer: no LLT, no generic MIR (G_*), no legalizer rule sets, no register banks. A search for `globalisel|gisel|regbank` found only comments.

The SelectionDAG port exists and is the x86 path: `llrm-codegen/src/backend/selectiondag` (~54 K lines with tests) and `llrm-dagisel`.

A GlobalISel port would reuse:

- MIR as input (the IRTranslator maps nearly 1:1, as the DAG builder's SD1 does).
- The decode/interpret approach for `MatchTable0`, which is a different opcode set (`GIM_*`/`GIR_*`), so a new decoder and interpreter.
- The arm64 emitter and encoder.

It would build from scratch:

- Generic MIR with LLT.
- The legalizer: `LegalizerHelper` alone is a large part of the 42 K generic lines.
- RegBankSelect.
- The combiner engine and its generated rule tables.
- `CallLowering`.

On rule 5: generic G_* MIR plus register banks is a second machine-flavoured IR beside MIR. It is not a "MIR in, MIR out" pass set, so it needs its own numbered decision.

x86 gets nothing from this: the pin's X86 GlobalISel is partial and not default either. Two selection frameworks would then be maintained.

### (4) Effort and convergence

- Effort: about 45–60 PRs (**estimate, unverified**). That is the generic GlobalISel core (~20) plus the AArch64 parts (~25–35) plus tooling. Against 30–35 for SelectionDAG, which reuses the existing x86 port.
- Convergence risk is high. The goal is the pinned clang's output, and that is SelectionDAG. Every GlobalISel row that differs (47 of 84 files here) is a gap that no amount of faithful translation closes. The oracle would be a different program from the one being ported.

### (5) Recommendation

Stay on the SelectionDAG route. Do not start GlobalISel.

Revisit only if the pin's default changes: `aarch64-enable-global-isel-at-O` > 0 in a later pin. Recheck that one line at each pin bump.

First step unchanged: dry-run `llrm-dagisel-gen` on `AArch64GenDAGISel.inc`.

## Status (P6 slices B2, C1, D1)

`crates/target/llrm-arm64/src/dag/` runs the shared builder, combiner, legalizers, matcher and scheduler on the generated AArch64 table: `ids.rs` (pin registers by name), `call.rs` (AAPCS64 integers in `x0`..`x7`, `RET_GLUE`), `function.rs` (`BuilderArch`, `select_function`), `pipeline.rs` (sched info), `hooks.rs` + `addr.rs` (the matcher hooks: node and pattern predicates from the generated table via `aarch64_predicates.rs`, xforms, `SelectArithImmed`, `SelectNegArithImmed`, `SelectArithShiftedRegister`, `SelectAddrModeIndexed`/`Unscaled`/`XRO`; `WRO`, extended registers, pairs, `ADDlow`, CVT fixed-point and `FromAnd` answer false, listed as DEVIATE in the headers). `tests/dag_function.rs`: `add` selects `ADDWrr` + `RET_ReallyLR`, `sub i32` is `SUBSWrr` as the pin's `finalize-isel` has it. Loads and stores (`ui`/`UR`/`roX`), frame addresses, `MOVi*imm` and global addresses (`ADRP`+page offset, `MOVaddr`, `LOADgot`; `globals.rs`) select and emit. Integer `setcc`/`select`/`select_cc` lower in `lowering.rs` (`getAArch64Cmp`, `emitComparison`, `CSEL`/`CSINC`/`CSINV`/`CSNEG`) and emit with the flags written once (`subs wzr`). Branches (`BRCOND`/`BR_CC` to `CBZ`/`TBZ`/`Bcc`) and several blocks emit, with the phi copies placed as phi elimination does (a block on each critical edge); `tests/isel_dag_run.rs` runs loops, diamonds and every integer compare against C. The ALU family selects and emits too: logic (`ANDWri`..`BICSWrs`, `lower_xor`), variable shifts (`LSLV`..), `MADD`, `SDIV`/`UDIV`, `SBFM`/`UBFM`, the sub-register pseudos, and zero is `WZR`/`XZR` as in `Select`'s `ISD::Constant`; the legalizer takes the shift-amount type from the target (`ShiftAmount`: `i64` here). Direct and indirect calls (`ADJCALLSTACKDOWN`, `BL`/`BLR`, `ADJCALLSTACKUP`), stack arguments in both directions (Darwin's packed sizes and alignments; fixed objects are `INCOMING_ARGUMENTS`) and `i128` pairs select and emit (`call.rs`); `Out`/`In` carry `arg_vt` and `split` as the pin's `ArgVT` and `isSplit`, and a target with a calling hook no longer needs an x86-style plan. Scalar `f32`/`f64` select and emit: arithmetic, `FCMP` + `CSEL`/`FCSEL`/`BRCOND` (two conditions for `one`/`ueq`), conversions, `fmov` immediates and zero, two-`mov` constants (`isFPImmLegal`), loads and stores, float arguments and results (`s0`..`s7`/`d0`..`d7`, stack), phis. Refused: a constant that needs a pool (no pool data in `MFunc`), `f16`, vectors, `sret`, varargs, tail calls, any other Custom operation. C1: `dag/emit.rs` turns the scheduled units into `MFunc` (ADD/SUB register and immediate rows). D1: `llrm-arm64-llc --isel=hand|dag|shadow` (`LLRM_ISEL_DAG`, `LLRM_ISEL_SHADOW`); `shadow` falls back to the hand selector per function and prints `isel-shadow: @name: reason`. `tests/isel_dag.rs`: `f` is the same assembly as the hand selector's, `g` (a `select`) falls back. Next: constant pools, atomics, varargs; each row with an `llvm-mc` byte check.
