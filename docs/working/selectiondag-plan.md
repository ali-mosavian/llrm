# Translating SelectionDAG instruction selection

Pin d1106deb71cc. Codegen slot row 28 ([codegen-slot-table](codegen-slot-table.md)): "X86 DAG->DAG Instruction Selection", DEVIATE CG1, "no SelectionDAG". This is the plan to close it: structure of the pin, what replaces each part today, how the generated matcher is made, pieces in order with dates and tests. Plan only; 2026-10-22. Paths under `llvm/lib/` unless noted; line numbers are the pin's.

## 1. The pin's structure

`SelectionDAGISel::runOnMachineFunction` ([L599][isel-run]) selects each block with `CodeGenAndEmitDAG` ([L941][isel-cg]), which runs, in this order:

| Step | Pin | Size |
|---|---|---|
| build the DAG from IR | `SelectionDAGBuilder` ([visitICmp L3808][b-icmp], [visitSelect L3863][b-sel], [visitGetElementPtr L4493][b-gep], [visitLoad L4735][b-load], [visitStore L4917][b-store], [LowerCallTo L9256][b-call], [visitSwitch L12878][b-switch], [lowerWorkItem L12492][b-work]) | 13.2k lines |
| combine 1 | `DAGCombiner::Run` ([L1845][c-run]), `combine` ([L2157][c-combine]), one `visit*` per opcode (`visitADD` [L3295][c-add], `visitAND` [L7861][c-and], `visitSETCC` [L15048][c-setcc], `visitLOAD` [L22110][c-load], `visitSTORE` [L24689][c-store]) | 32.5k |
| legalize types | `DAGTypeLegalizer::run` ([L196][lt-run]) with `LegalizeIntegerTypes` (promote, expand), `LegalizeFloatTypes`; `LegalizeVectorOps` is skipped (no vectors until 32-bit parity) | 1.0k + 6.7k + 3.3k |
| combine 2, legalize DAG, combine 3 | `SelectionDAG::Legalize` ([L6301][ld-legalize]), `LegalizeOp` ([L982][ld-op]), then `X86TargetLowering` (ctor [L131][x-ctor] sets every action; `LowerOperation` [L34783][x-lower], `ReplaceNodeResults` [L34968][x-replace], `PerformDAGCombine` [L63938][x-combine], calls and arguments in `X86ISelLoweringCall.cpp` [L1710][x-formal]) | 6.4k + 65.6k + 3.2k |
| select | `DoInstructionSelection` ([L1281][isel-do]) calls `X86DAGToDAGISel::Select` ([L5513][x-select]), which tries hand matchers (`foldLoadStoreIntoMemOperand` [L3925][x-rmw], `matchBitExtract` [L4149][x-bext], `tryShiftAmountMod` [L4664][x-shamt]) then `SelectCode`; addresses by `matchAddress` ([L2053][x-addr]), `matchAddressRecursively` ([L2677][x-addrrec]), `selectAddr` ([L3202][x-seladdr]), `selectLEAAddr` ([L3329][x-lea]); `PreprocessISelDAG` ([L988][x-pre]) and `PostprocessISelDAG` ([L1643][x-post]) around it | 7.1k |
| matcher | `SelectCodeCommon` ([L3351][isel-common]) interprets `X86GenDAGISel.inc`, a byte table the pin's tblgen makes from the `.td` files (`DAGISelEmitter` [L152][tg-emit], `EmitMatcherTable` [L1602][tg-table]) | 11.3 MB, 254k opcodes in the assert build |
| schedule and emit | `ScheduleDAGSDNodes::Run` ([L55][s-run]), `BuildSchedGraph` ([L536][s-build]), the register-pressure list scheduler (`createBURRListDAGScheduler` [L3113][s-burr]; X86 asks for it at 32 bits, [L148][x-sched]), `InstrEmitter::EmitMachineNode` ([L1003][e-node]) | 1.1k + 3.2k + 1.5k |

`FastISel` (-O0) is out of scope: we have one path. The `.td` side: `X86InstrFragments.td`, `X86InstrCompiler.td`, `X86InstrArithmetic.td`, `X86InstrFPStack.td`, `X86InstrMisc.td`, `X86InstrShiftRotate.td`, `X86InstrCMovSetCC.td`, `X86InstrControl.td`, `X86InstrExtension.td`. In the pin's `--dump-json` of `X86.td`: 5,909 `Pat` and 22,770 `Instruction` records; about 800 and 3,400 carry no vector, AVX, MMX, FMA, APX or NDD predicate (a coarse filter on `Predicates`, an upper bound for the subset, not a count).

## 2. What replaces each part today

Ours selects from MIR (LLVM-IR-shaped, SSA, typed) to LIR in one forward walk: `Selector::instruction` matches each instruction against a decision automaton that `llrm-iselgen` builds from `family.isel` + `patterns.isel` + `x86.instr` ([isel.rs](../../crates/codegen/llrm-codegen/src/backend/isel.rs), [isel/matcher.rs](../../crates/codegen/llrm-codegen/src/backend/isel/matcher.rs)). Patterns are our own, flat (an opcode and up to three operands, one nested defining instruction via `cover`), the rest is `hook_` Rust. No node graph, no legalizer, no combiner, no scheduler: selection order is MIR order, and the machine phases do the rest.

| Pin part | Ours today | What is missing against the pin |
|---|---|---|
| SelectionDAGBuilder | `Selector::body` and `instruction`; `selects.rs` (select to branch), `booleans.rs` (setcc chains priced by the cost table), `compares.rs`, `bitcounts.rs`, `funnel.rs`, `fpconvert.rs` (intrinsic and wide-FP expansions in MIR); `addresssink.rs` (CGP, row 23, not this plan) | switch lowering (jump tables, bit tests, `SwitchPeelThreshold` [L133][b-peel]), `FindMergedConditions` for `&&`/`||` branches, per-block DAGs with `CopyToReg`/`CopyFromReg` across blocks |
| SelectionDAG + DAGCombiner | none as a pass. Fragments: `arithmetic.rs` (multiply by constant), `division.rs` (reciprocal multiply, = `BuildSDIV`/`BuildUDIV`), `storecombine.rs` (= `mergeConsecutiveStores`), `rmw.rs`, `comparefold.rs`, `floatfold.rs` (load folds), `peep/` and `peephole.rs` (post-isel rewrites; 2,065 + 3,154 lines) | `getNode` constant folding and CSE, the worklist combiner, `SimplifyDemandedBits`, `visitSETCC`/`visitSELECT_CC` folds, load/store combining, the X86 `PerformDAGCombine` set. The peephole groups (`groups.list`) are mostly compensations for this |
| LegalizeTypes / LegalizeDAG | `legalize.rs` (promote i2..i63 to a machine width, = `PromoteIntegerResult`), `isel/expand.rs` + `isel/wide.rs` + `lower_int64.rs` (i64 as two dwords, = `ExpandIntegerResult`), `isel/huge.rs` (huge-pointer memory intrinsics) | action tables (`setOperationAction`), `Expand`/`Custom`/`LibCall` per opcode and type, truncating stores and extending loads as actions |
| X86ISelLowering | `target.rs`, `cpu.rs`, the target descriptions (`calling.toml`, `registers.regs`, cost tables), `Convention::place` in `isel.rs` | the lowering of select/setcc/brcond to flags (`EmitCmp` [L24085][x-cmp], `LowerSETCC` [L25293][x-setcc], `LowerSELECT` [L25834][x-select2]), shifts, FP conversions, call and return lowering as DAG nodes |
| X86ISelDAGToDAG + matcher | `isel/matcher.rs` runtime, `addressforms.rs`, `addressvalues.rs`, `exactaddress.rs`, `upperzero.rs` (address operands) | the generated matcher, `X86ISelAddressMode`, LEA matching, the hand matchers above |
| ScheduleDAG / InstrEmitter | LIR is emitted in MIR order; `schedule.rs` reorders after allocation (latency, not pressure) | the pre-RA register-pressure list scheduler and the node-to-instruction emitter (three-address, virtual registers, no ties; `twoaddr.rs` and the TwoAddress port stay downstream) |

### m16 against m32

m32 (i486/P5) is the pin's x86-32 with `-sse`: GR8/GR16/GR32, x87 `RFP32/64/80`, `[base + index*scale + disp32]`. Everything above applies; the work is choosing the legal subset, not new ideas.

m16 is real mode, and the pin has no model for it. What it keeps, as numbered DEVIATEs (reason, owner, exit, review date each; the first review on 2026-11-13 with row 28):

- **SD1** MIR is the builder's input, not LLVM IR. Same shape, so `visit*` ports one to one; the difference is the opcode set (`opcode.rs`: no atomics, `indirectbr`, vectors). Exit: none.
- **SD2** `far` is a value type: selector and offset, two i16. The type legalizer expands it like an integer (`ExpandIntegerResult` shape) and `far_load`/`far_store`/`far_cast` become `les`/`lds` matches (today `farload.rs`, `hook_far_*`). The pin's address spaces 256..258 are segment overrides on an i32 pointer and do not fit.
- **SD3** Addresses are `[bx|bp] + [si|di] + disp16` with a segment, registers from the `base` and `index` classes of `registers.regs`, no scale. `X86ISelAddressMode` gets a 16-bit component set; the pin's only 16-bit branch ([L1926][x-16]) picks the 67h form of 32-bit components, which `addressforms.rs` also uses today and stays an alternative. Exit: none; the class split is the target.
- **SD4** Register classes. DAGToDAG constrains a value to `BX|SI|DI|BP` when it is an address component (a class on the virtual register, [vreg-class-field-plan](vreg-class-field-plan.md)), where the pin's GR16 has no sub-class for it. Depends on that plan landing.
- **SD5** Calling conventions come from `Convention::place` (`calling.toml`: watcall16, regparm3, pascal16), not `X86CallingConv.td`; `LowerCall`/`LowerFormalArguments` call it and build `CopyToReg` nodes from its answer.
- **SD6** x87: values are `RFP` virtual registers until our `fpstack.rs` (= `X86FloatingPoint`); selection emits the `*_Fp` pseudo-forms from `X86InstrFPStack.td` patterns, as the pin does. Long double (f80) and the `FPStack` predicates stay the family's; m16 differs only in the 16-bit `fnstsw`/`sahf` sequences.
- **SD7** i64 is two dwords at both sizes (`MVT::i64` illegal on both m16 and m32, as in the pin at 32 bits). No change; listed because m16 expands through 16-bit pairs for `far` only.
- **SD8** Cost: our `x86.instr` columns (cycles per CPU) replace `X86Sched*.td` where a pattern group competes; the pin's pattern complexity (`PatternComplexity`, the order `SelectCode` tries) stays the first key and cost only breaks ties inside our `group`s until each group is retired by a translated pattern.

## 3. The generated matcher

Requirement: from the pin's `.td`, by a tool, nothing hand-copied. Two ways, and the choice:

- Port `utils/TableGen/DAGISelEmitter` and `DAGISelMatcherGen` to Rust: a Rust `.td` reader, then `CodeGenDAGPatterns`' type inference. About 20k lines of C++ (`Common/CodeGenDAGPatterns.cpp` alone is 5k); the largest and riskiest piece of the plan, for an output the pin already makes.
- **Chosen: read the pin's own table.** The pin's `llvm-tblgen -gen-dag-isel X86.td` (built from the pin; `llvm-tblgen` exists) writes `X86GenDAGISel.inc`. A Rust tool, `llrm-dagisel-gen` (crate beside `llrm-iselgen`), decodes the byte table into a tree (`Matcher` enum: `Scope`, `RecordNode`, `CheckOpcode`, `CheckType`, `CheckInteger`, `CheckPatternPredicate`, `CheckComplexPat`, `MoveChild`, `EmitNode`, `MorphNodeTo`, ...; the opcode list is `enum BuiltinOpcodes` in `SelectionDAGISel.h`), drops what the subset excludes, and writes a Rust `static` table plus the predicate and complex-pattern dispatch (`CheckPatternPredicate`, `CheckNodePredicate`, `CheckComplexPattern`: 292 pattern predicates in the pin's file, of which the subset keeps those over `Not64BitMode`, `HasX87`, `HasCMOV`, `FPStackf32/64`, `OptForSize`, `UseIncDec` and drops the vector, AVX, BMI and APX ones). The interpreter is a translation of `SelectCodeCommon` ([L3351][isel-common]) over the enum, with its `MatchScope` stack and `RecordedNodes`.

Subset by the target description, not by hand: the CPU's feature set (i486: no CMOV before P6, so `NoCMOV`; P5: cmpxchg8b, no CMOV) selects predicates; a record whose predicates all hold at that CPU stays, the rest is pruned. m16 is the same table with SD2..SD4 applied: `far` patterns and the 16-bit address matchers are added by the target's own `.td`-shaped file (`patterns.isel` shrinks to exactly those, in the pin's pattern syntax, read by the same tool).

Regeneration is a script (`tools/dagisel/regen.sh`: build the pin's tblgen, run both tools) and the generated Rust is checked in with a test that regenerates into a temp dir and diffs (a stale table is a failing test). `ISel` patterns the `.td` lacks for our targets (segment loads, string ops with `ES`) are `.td`-syntax additions in our own file, numbered SD9 as a block.

Open until piece D1 prices it: the table for the subset in bytes, and the interpreter's compile time per function against the 1.5x clang goal. Fallback inside the plan, not a switch: if the byte table is slow to walk, the tool emits the decision tree as nested `match` (the pin's `-gen-dag-isel` has no such mode; this would be a numbered DEVIATE).

## 4. Plan

Owner groups are new sessions the coordinator names; none comes from `splitkit` or `w-1`, and none edits their files (`allocate.rs`, `splitkit*`, `spiller.rs`, `twoaddr.rs`, `phielim.rs`). The interface to them is today's: LIR over virtual registers, three-address, class-less except SD4. At most three writers at once, so the other backend rows keep their workers. Groups: **sdag-a** (DAG core, builder, calls), **sdag-b** (legalizer, X86 lowering, combiner), **sdag-c** (generator, matcher, DAGToDAG), **sdag-d** (scheduler, emitter, shadow harness, corpus). Dates are targets, reviewed weekly with the coordinator.

Rules for every piece: idiomatic Rust (`SDNode` in an arena indexed by `SDValue { node: u32, res: u8 }`, `ISD` and `MVT` enums, `Option`/`Result`, no raw-pointer emulation); permalinks and the pin's order, caps (`cl::opt` defaults) and worklist; wrong output and compile failures first; size, clock and compile rows are numbered in the PR, never blocking; fail-first test in the commit; release builds only, two target dirs; read the whole lib run of the changed crate before ready.

### Phase 0: shadow mode (new isel behind a flag, old one still the default)

| Id | Piece | Group | Due | Premise test (fail first) |
|---|---|---|---|---|
| A0 | `selectiondag` module: `MVT`/`EVT`, `ISD`, arena `SelectionDAG`, `getNode` with CSE and constant folding (`SelectionDAG.cpp`) | sdag-a | 2026-11-05 | `x + y` built twice gives one node; `3 + 4` is one constant node; `x * 1` stays a `MUL` (the identity fold is `DAGCombiner::visitMUL`, B2); fails on an empty DAG |
| A1 | Builder, scalar subset: arithmetic, casts, `icmp`/`select`/`br`, load/store, GEP, `phi` as `CopyToReg`; per-block DAG | sdag-a | 2026-11-19 | a MIR block of `a+b; store` dumps the DAG the pin's `-debug-only=isel` shows for the same IR (text compare, pin run through `pin.py`) |
| A2 | Calls, returns, arguments through `Convention::place` (SD5); switch lowering (`SwitchCG`: clusters, jump tables, bit tests, peel) | sdag-a | 2026-12-10 | a 6-way `switch` makes the pin's jump table or bit test at m32; the DAG for a `watcall16` call has `CopyToReg AX, DX, BX, CX` in that order |
| B0 | Type legalizer: promote and expand integers, `far` (SD2); `X86TargetLowering` action table for m32 from the target description | sdag-b | 2026-11-26 | an i64 add at m32 becomes `add`/`adc` nodes; the same at m16 for a `far` pair; fails with the action table empty (node stays illegal) |
| B1 | `LegalizeDAG` (`LegalizeOp`, `ExpandNode`), `LowerOperation` subset: setcc/select/brcond to `X86ISD::CMP`/`SETCC`/`BRCOND`, shifts, FP conversions | sdag-b | 2026-12-17 | `select` at i486 (no CMOV) gets the branch expansion `EmitLoweredSelect` makes; at P6-less CPU a `cmov` never appears |
| B2 | `DAGCombiner` core: worklist `Run`/`combine`, `visitADD`, `visitAND`, `visitSHL`, `visitMUL`, `visitSETCC`, `visitLOAD`/`visitSTORE`; then `PerformDAGCombine` visitors in the order the corpus counts them (first: `combineSetCC`, `combineSelect`, `combineLoad`, `combineStore`, `combineMul`) | sdag-b | first 2027-01-14, then one visitor per PR | one test per visitor: a MIR/IR shape the pin folds, the DAG after combine matches the pin's dump |
| C0 | `llrm-dagisel-gen` decoder and interpreter, m32 subset, with `regen.sh` and the stale-table test | sdag-c | 2026-11-19 | the decoded table for the subset selects `add reg, reg` and `mov reg, [base+idx*4+8]` from a hand-built DAG; the interpreter fails on a table with a bad opcode byte |
| C1 | `X86DAGToDAG`: `X86ISelAddressMode`, `matchAddress*`, `selectAddr`, `selectLEAAddr`, `tryFoldLoad`, `foldLoadStoreIntoMemOperand` | sdag-c | 2026-12-10 | `a[i+2]` at m32 gives one `[base+idx*4+8]` operand; the memory-destination `add [m], r` is one instruction (today's `rmw.rs` case) |
| C2 | m16: SD3 address mode and SD4 classes, `far` patterns, string-op and segment forms in our `.td`-shaped file | sdag-c | 2027-01-14 | `p->x` through a far pointer is one `les` + `[es:di+disp]`; a word address never uses AX or DX as base |
| C3 | x87 pseudo patterns (SD6) and the remaining hand matchers (`matchBitExtract`, `tryShiftAmountMod`) | sdag-c | 2027-01-28 | `fld [m]; faddp` is `fadd [m]` (today's `floatfold.rs` case) |
| D0 | Harness: `--isel=dag` flag, per-function LIR dump from both selectors (`ISEL_DUMP`), `tools/pairs/isel_shadow.py` (counts instructions, bytes, copies, executed instructions per function) feeding `objsweep` | sdag-d | 2026-11-05 | the report on old-vs-old is empty; a seeded one-instruction difference shows as one function |
| D1 | Scheduler (RegPressure list scheduler, `ScheduleDAGRRList`) and `InstrEmitter` to LIR; the first end-to-end function through the DAG path | sdag-d | 2026-12-03 | `x = a+b; y = x*c` returns the right value through the emulator on the new path (tests/run `c`), fails when the emitter drops a `CopyToReg` |
| D2 | Shadow runs on the corpus: objsweep, QCport -Os (65 modules, `tools/rows/qcport.py`, m16), the rows, `tests/run` (c, nib, qb, alias, huge), qbrt. First on the pieces that exist (functions the DAG path refuses fall back to the old selector only inside the shadow harness, counted) | sdag-d | from 2026-12-03, weekly | per run: correctness failures (must be 0 before the switch), refused functions, rows above main |

### B0 in detail (sdag-b)

Pin: `DAGTypeLegalizer::run` ([L196][lt-run]) over `LegalizeTypes.h` (1.3k), `LegalizeIntegerTypes.cpp` (89 `PromoteIntRes_*`-family and 57 `ExpandIntRes_*` functions, 6.7k lines), `X86TargetLowering` ctor ([L131][x-ctor]) and `computeRegisterProperties` (called at [L2934][x-regprops]). Input is A0's `selectiondag` module; B0 starts its code when A0 (#3689) lands. At both targets `i8`, `i16`, `i32` are legal, `i1` and odd widths promote, `i64` expands to two `i32`, `f32`/`f64`/`f80` are legal in the x87 classes (no `SoftenFloat`/`ExpandFloat`, nothing to translate there), and at m16 `far` expands to two `i16`. Today this is `legalize.rs` (promotion, in MIR), `isel/expand.rs`, `isel/wide.rs` (halves); they stay until Phase 2.

| Id | Piece (pin function, in the pin's order) | Due | Notes |
|---|---|---|---|
| B0a | `TargetLoweringInfo`: `ValueTypeActions`, `getTypeAction`/`getTypeToTransformTo` (what `computeRegisterProperties` and `getTypeConversion` compute), `LegalizeAction` enum, and the `X86TargetLowering` action table for m32 (`setOperationAction`, `setLoadExtAction`, `setTruncStoreAction`, `setCondCodeAction` for the i486/P5 subset), written in the ctor's order because a later call overwrites an earlier | 2026-11-12 | table is Rust data built from the target description (legal widths, CPU features); vector, SSE, AVX and 64-bit arms of the ctor are not translated and listed as N/A with their line ranges |
| B0b | `run()` and its machinery: node ids (`enum NodeId { New, ReadyToProcess, Processed, Unanalyzed(n) }`), worklist, `AnalyzeNewNode`/`AnalyzeNewValue`, `ReplaceValueWith`, `RemapValue`, the maps `ReplacedValues`, `PromotedIntegers`, `ExpandedIntegers` (arena-indexed `Vec<Option<_>>`, not hash maps), `PerformExpensiveChecks` as a debug-only verifier | 2026-11-19 | no `NodeUpdateListener`: the arena makes replacement an index rewrite |
| B0c | `PromoteIntegerResult`/`PromoteIntegerOperand`, in corpus order: constants, loads (`PromoteIntRes_LOAD`), binary arithmetic and logic, shifts (amount zero-extended), `SETCC` ([L1371][li-setcc]), `Select` ([L1350][li-select]), trunc/zext/sext, bitcast, div/rem (extension by signedness), stores (`PromoteIntegerOperand`), `BRCOND`/`CopyToReg` operands | 2026-11-26 | replaces `legalize.rs`'s cases one by one |
| B0d | `ExpandIntegerResult`/`ExpandIntegerOperand`: `ExpandIntRes_Constant` ([L4116][li-const]), `ADDSUB` ([L3725][li-addsub]) to `ADDC`/`ADDE` pairs, `MUL` ([L4540][li-mul]), logic, `ExpandShiftByConstant` ([L3296][li-shiftc]) and the unknown-amount form, `LOAD`, `SETCC` ([L3589][li-esetcc]), `Select`, trunc/ext, `UDIV`/`SDIV`/`UREM`/`SREM` as helper calls | 2026-12-03 | SD19 below |
| B0e | `far` (SD2): `MVT::Far` action `ExpandInteger`-shaped; results as (offset, selector) halves for load, store, select, `CopyToReg`/`CopyFromReg`, constants, compares, `far_cast`; operands through `BUILD_PAIR` | 2026-12-03 | m16 only; the m32 table has no `far` |

Numbered DEVIATE from B0:

- **SD19** Wide division and remainder of `i64` call our inline helper routines with their register contracts (`lower_int64.rs`, `llrm_x86::helpers`) where the pin emits a compiler-rt libcall. The legalizer returns a `CALL` node with the helper's convention (SD5). Reason: the helpers exist, are exact, and no runtime library is linked. Owner sdag-b; exit: when LegalizeDAG's `ExpandLibCall` path is translated in B1 and the helper contract is expressed as a `RuntimeLibcall` table (review 2026-12-17).

Premise tests (each seen failing before its piece lands; they dump the DAG after `LegalizeTypes` as text and, where the pin shows the same IR, compare with `pin.py`'s `llc -debug-only=legalize-types` output):

1. B0a: `getTypeAction` is `Legal` for i8/i16/i32, `Promote` for i1 (to i8), i12 (to i16), i24 (to i32), `Expand` for i64 (to i32) and, at m16 only, `far`; an empty table makes every type `Legal` and the test fail.
2. B0b: a DAG whose root uses an illegal type leaves `run()` with no illegal type in any node, and a node replaced twice resolves through `RemapValue` to its last replacement.
3. B0c: `i12 add` promotes to `i16 add` with no extension; `i12 udiv` zero-extends both operands; `i12 sdiv` sign-extends them (the signedness is the symptom: wrong results for a negative `i12` quotient).
4. B0d: `i64 add` at m32 is `ADDC` low then `ADDE` high; `i64 shl 8` is the pin's two-node funnel form (`ExpandShiftByConstant`); `i64 load` is two `i32` loads at offsets 0 and 4 with the chain joined.
5. B0e: a `far` load is two `i16` loads (offset, then selector at +2); `far == far` is two compares combined; a `far` `select` is two `select`s.
6. SD19: `i64 sdiv` produces a `CALL` to the helper named in `llrm_x86::helpers`, with its argument registers.

An end-to-end run waits for A1 (builder) and D1 (emitter); until then each test builds its DAG by hand through A0's `getNode`.

### B1 in detail (sdag-b)

Pin: `SelectionDAG::Legalize` ([L6301][ld-legalize]) orders the nodes topologically and calls `SelectionDAGLegalize::LegalizeOp` ([L982][ld-op]) on each. `LegalizeOp` asks the action table (B0c) and does one of: **Legal** (keep; `LegalizeLoadOps` [L669][ld-loads] / `LegalizeStoreOps` [L492][ld-stores] first for memory nodes), **Custom** (`TLI.LowerOperation`, [x-lower]; `ReplaceNodeResults` [x-replace] for results of an illegal type), **Expand** (`ExpandNode` [L3210][ld-expand], then `ConvertNodeToLibcall` [L4734][ld-lib] for what remains), **Promote** (`PromoteNode` [L5432][ld-promote]), **LibCall** (`ExpandLibCall` [L2115][ld-libcall]). Runs after types are legal (B0) and before the third combine.

**Correction found while reading the pin.** The i486 `select` is not expanded to a branch in the DAG. `LowerSELECT` ([L25834][x-select2]) builds `X86ISD::CMOV` on EFLAGS from a `CMP` for every CPU; with `NoCMOV` the matcher picks the `CMOV_GR8/GR16/GR32` pseudos (`X86InstrCompiler.td` L577 to L629, `usesCustomInserter`), and `EmitLoweredSelect` ([L37564][x-lowered]) turns each into a branch and a phi at instruction emission (`EmitInstrWithCustomInserter` [L38957][x-inserter], called by `InstrEmitter`). So B1's premise test is on the DAG (`select` becomes `CMOV` of a `CMP`). Handoffs:

1. sdag-c owns the `CMOV_GR8/GR16/GR32` pseudo patterns in the pruned i486/P5 table (no `CMOV32rr` there; pinned by its `neither_cpu_emits_cmov` test, descriptors in #3721).
2. sdag-d owns the emitter calling the custom inserter, and `EmitLoweredSelect` (block split, branch, phi).
3. sdag-b owns the DAG side: `LowerSELECT` to `CMOV` (B1c).

| Id | Piece | Due | Premise test (fail first) |
|---|---|---|---|
| B1a | `SelectionDAGLegalize` driver: topological walk, `LegalizeOp` dispatch by action, `Promote` via `type_to_promote_to`, results through the B0 replace machinery | 2026-12-10 | a DAG with an `i8` `add` marked Promote becomes `trunc(add i32 (anyext a), (anyext b))`; a node whose action is Expand and has no `ExpandNode` case is refused by name |
| B1b | `ExpandNode` subset: `SDIV/UDIV/SREM/UREM` to `SDIVREM/UDIVREM`, `BR_CC` to `SETCC` + `BRCOND`, `SELECT_CC` to `SETCC` + `SELECT`, `MULHS/MULHU` to `SMUL_LOHI/UMUL_LOHI`, `BSWAP` of a word, `UADDO`/`USUBO` family | 2026-12-17 | `sdiv i32` gives one `SDIVREM` and `x / y` with `x % y` share it (CSE); `br_cc` leaves no `BR_CC` node |
| B1c | `LowerOperation` core: `EmitCmp` ([L24085][x-cmp]) to `X86ISD::CMP`, `LowerSETCC` ([L25293][x-setcc]) to `SETCC`, `LowerSELECT` to `CMOV`, `LowerBRCOND` to `BRCOND`; the `X86ISD` node enum as `ISD::Target(n)` names | 2027-01-07 | `select (icmp ult a, b), x, y` at i32 is `CMOV x, y, ULT, (CMP a, b)`; `brcond (icmp eq a, 0)` tests with `CMP a, 0` (not a materialized bool); CPU has no CMOV and the DAG is the same |
| B1d | Shifts and FP: `LowerShiftParts` ([L20256][x-parts]) for `i64` shifts, `LowerSINT_TO_FP` ([L20628][x-sint]), `LowerUINT_TO_FP` ([L21136][x-uint]), `LowerFP_TO_INT` ([L22157][x-fpint]) for x87 | 2027-01-14 | `shl i64 x, y` is `SHL_PARTS` then `SHLD`/`SHL` with the `y & 32` select; `uitofp i32` at x87 goes through `i64` `FILD`; `fptoui` has the range check |
| B1e | Libcalls: `ExpandLibCall` with our helper table (SD19) for `i64` division and `fmod` | 2027-01-14 | `i64 sdiv` is a `CALL` to the `llrm_x86::helpers` routine named by the contract |

`ISD` lacks `SHL_PARTS`, `SDIVREM`, `SMUL_LOHI`, `BR_CC` forms and the `X86ISD` names; each piece adds the variants it needs to `isd.rs` in the pin's position, small and registered here.

### B0 addendum: the m32 action table and the `far` tests

**Action table (B0a2, 2026-12-03 with B0d).** The pin sets actions in `X86TargetLowering::X86TargetLowering` ([L131][x-ctor]) in one pass, and a later call overwrites an earlier one, so the Rust table is built by the same calls in the same order. Translated for i486/P5, with each arm's line range kept as a comment: scalar integer setup (L134-L178: x87 on, boolean contents `ZeroOrOne`, shift amounts and setcc results `i8`, scheduling preference RegPressure at 32 bits [L148][x-sched]); register classes (L179-L188); truncating stores and extending loads (L189-L198); `ABS`, funnel shifts, saturating ops (L205-L240, `canUseCMOV` false before P6: the CMOV arms are not taken); the `!useSoftFloat` arm for x87 (L241-L310, only the `hasX87` parts); integer divide and remainder as `SDIVREM`/`UDIVREM` and multiply-high (L327-L352); bit counts (L364-L450: `CTTZ`/`CTLZ` expand without BMI/LZCNT, `CTPOP` expands without POPCNT, so `bitcounts.rs` goes at Phase 2); `SELECT`/`SELECT_CC`/`BR_CC` custom (L469-L484, the pin expands `cmov` itself); 64-bit shifts as `SHL_PARTS`/`SRA_PARTS`/`SRL_PARTS` custom (L508-L520); atomics (L522, N/A: MIR has none); `VASTART` and friends (L559-L570); x87 FP arms (L764-L870: f32/f64 `RFP` classes when `UseX87` and no SSE, `f80` always); the common FP expansions (L935-L969); and the 32-bit-only custom lowerings at L2749-L2830. Not translated and listed as N/A with their ranges: every `hasSSE*`, `hasAVX*`, `hasAVX512`, `hasBWI`, `hasVLX`, `hasFP16`, `hasMMX`, `hasXOP`, `hasGFNI`, AMX and `is64Bit` arm (L318-L326, L437-L468 partly, L618-L1034, L1036-L2748). Premise test for the table: a line count of calls per arm against the pin's, so a dropped arm shows (the count comes from the pin's source, checked in as a list, and the test compares the table's number of set actions per category to it).

**`far` tests (B0e).** Each fails first (the type has no rule before B0e):

1. `far` constant `(0x1234:0x0010)` expands to two `i16` constants, offset first.
2. `far` load from `p` is `i16` load at `p` and `i16` load at `p+2`, chain joined by a `TokenFactor`; the pin's `ExpandIntRes_LOAD` ([L4414][li-load]) is the shape.
3. `far` store is two `i16` stores at the same offsets, one chain.
4. `far == far` and `far != far` expand to a compare of both halves combined with `and`/`or` (not a 32-bit compare).
5. `far` `select` is two `i16` selects on one condition.
6. `far` through `CopyToReg`/`CopyFromReg` is two virtual registers, offset then selector (the order `les` loads them in).
7. `far` to `i32` and back (`far_cast`) is `BUILD_PAIR`/`EXTRACT` of the halves, no arithmetic.
8. At m32 a `far` node is refused with a message naming the target (SD2: m16 only).

The ordering inside a group is the table's; groups run in parallel from 2026-10-26, so by 2026-12-03 an end-to-end function exists (A0, A1, B0, C0, D0 done; D1 joins them). Shadow means the old selector produces the output; the new one runs in `--isel=dag` and in the harness.

### Phase 1: the switch

Gate (all must hold on the head, measured by D2): zero compile failures and zero wrong outputs across the corpus; the DAG path refuses no function of QCport, tests/run or qbrt; allocator stack switched over first ; the full gate green on `feat/parity`. Size, clock and compile rows are numbered with owners and do not hold the gate. Earliest date: **2027-02-04**. Owner: sdag-d with the coordinator's go. The default flips by deleting the flag's off arm in the same PR, not by adding a second switch.

### Phase 2: delete the old selector

One PR per deleted unit, each naming its path in `Deletes:`, each after the DAG path has done its work for 1 week of corpus runs: `isel/matcher.rs`, `llrm-iselgen` and `family.isel`/`patterns.isel` (2027-02-18); the MIR pre-expansions the legalizer covers (`legalize.rs`, `isel/expand.rs`, `isel/wide.rs`, `lower_int64.rs` wide paths, `bitcounts.rs`, `funnel.rs`, `compares.rs`) once their cases are in the table (2027-03-04); fragments the combiner and matcher cover (`arithmetic.rs`, `division.rs`, `storecombine.rs`, `rmw.rs`, `comparefold.rs`, `floatfold.rs`) and each `peep/` group the ablation shows idle (2027-03-18). What stays and is retargeted, not deleted: `farload.rs`/`farcall.rs` (SD2), `addressforms.rs`/`addressvalues.rs` (SD3 alternatives), `fpstack.rs`. Row 28 becomes MATCH; CG1 closes.

### Not to starve the rest

- No sdag group takes `splitkit`, `w-1` or the workers on rows 23, 34, MachineSink, Peephole, the scheduler row and the X86 passes; the coordinator starts sdag sessions as new ones, three at a time (A and C first, then B, then D1).
- Shared files are few: `isel.rs` (the dispatch only, at the switch), `registers.regs`/`x86.instr` (read by both), the vreg class field (SD4 waits on its plan).
- If the allocator stack slips past 2027-02-04, the switch slips with it; the phases before it do not touch the allocator's inputs.

## 5. Risks, numbered

1. **The table's size and walk cost** (§3), priced at C0 (2026-10-22, callgrind on `examples/price.rs`, eight scalar node shapes): the pin's table is 639,106 bytes, 308,885 items decoded; pruned to the i486 CPU it is 15,108 items (P5 15,116), 24 bytes each, 362 KB static. The interpreter costs 3,495 Ir per selected node with reused buffers (4,819 before; 1.7k of it the walk itself). Projection on d_faces -O2 (i486, 1.28 G compile): the pin's llc makes 3,792 `Select` calls and spends 6.65 M Ir in `SelectCodeCommon` (1,755 per node, hooks included; whole isel bucket 54.5 M); the same count at 3,495 is 13.3 M, 1.0% of the compile. Our figure is a bench of simple shapes with a stub DAG, so it is a floor until A1 gives real nodes. If the walk proves slow, the `match` fallback below stays open.
2. **Combiner scale**: 32.5k lines of C++ plus X86's `PerformDAGCombine`; the plan lands visitors by corpus frequency and leaves the rest as numbered MISSING rows in this file's table (to be added at B2), not as a gap with no owner.
3. **m16 patterns the pin lacks** (SD2..SD4, SD9): the largest source of DEVIATEs; each carries a pin-dump premise test that shows the pin cannot express it.
4. **Quality regressions against the hand-tuned selector**: the shadow report is the instrument; per [pr-rules](pr-rules.md) rows above main are fixed generally or kept by a numbered DEVIATE with the row as evidence, never by a shape patch.

## 6. DEVIATE registry

One list of the `SD` numbers, so no two groups take one. Add a row in the PR that first uses a number; the next free number is the last row's plus one. The reason, exit and review date sit in the code or section named.

| SD | Owner | What | Where |
|---|---|---|---|
| SD1 | plan | MIR, not LLVM IR, is the builder's input | §2 |
| SD2 | sdag-b | `far` is a value type, expanded into two `i16` | §2, `llrm-sdag/src/lowering.rs` |
| SD3 | sdag-c | 16-bit real-mode address components | §2 |
| SD4 | sdag-c | BX/SI/DI/BP address classes on virtual registers | §2 |
| SD5 | sdag-a | calling conventions from `Convention::place` | §2 |
| SD6 | sdag-c | x87 as `RFP` pseudo-forms until `fpstack.rs` | §2 |
| SD7 | plan | `i64` is two dwords at both sizes (no change) | §2 |
| SD8 | sdag-c | our cycle tables break ties inside a `group` | §2 |
| SD9 | sdag-c | `.td`-syntax additions for patterns the pin lacks (block) | §3 |
| SD10 | sdag-a | no `SDLoc` | `selectiondag.rs` |
| SD11 | sdag-a | no FP constant folding | `selectiondag.rs` |
| SD12 | sdag-a | folds needing known bits are the combiner's | `selectiondag.rs` |
| SD13 | sdag-a | no vector types or nodes | `selectiondag.rs` |
| SD14 | sdag-b | floating types always legal (no soften/expand) | `llrm-sdag/src/lowering.rs` |
| SD15 | sdag-a | `FindMergedConditions` ported without edge probabilities | builder |
| SD16 | sdag-a | no `LiveOutRegInfo` | builder |
| SD17 | sdag-a | vreg numbers differ from the pin's | builder |
| SD18 | sdag-a | select patterns (abs, min, max) | builder |
| SD19 | sdag-b | `i64` division through our helper routines | §4 B0 |
| SD20 | sdag-c | matcher jumps count items; `ByHwMode` resolved at generation | `llrm-dagisel/src/interp.rs` |
| SD21 | sdag-c | no `mayRaiseFPException`/`NoFPExcept` in the interpreter | `llrm-dagisel/src/interp.rs` |
| SD22 | sdag-c | no `MatchStateUpdater` around complex patterns | `llrm-dagisel/src/interp.rs` |
| SD23 | sdag-c | vector immediate checks never match | `llrm-dagisel/src/interp.rs` |
| SD30 | sdag-d | scheduler: no physical-register interference, `ClusterNodes`, vreg-cycle check | `selectiondag/schedule.rs` |
| SD31 | sdag-d | scheduler: no IR order for the call tie break | `selectiondag/schedule.rs` |
| SD32 | sdag-b | action tables keyed by `MVT`; `Far` has no entry | `llrm-sdag/src/actions.rs` |
| SD33 | sdag-b | no `NodeUpdateListener`; RAUW returns what it did | `selectiondag/legalize_types.rs` |
| SD34 | sdag-c | address folds not translated: BEXTR, x32 `any_extend`, sign/zero-extend re-association | `llrm-dagisel/src/x86_addr.rs` |
| SD35 | sdag-c | no 64-bit, x32, RIP, TLS or vector address forms | `llrm-dagisel/src/x86_addr.rs` |
| SD36 | sdag-c | tail-call, `def32` and absolute-symbol predicates answer false | `llrm-dagisel/src/x86_hooks.rs` |
| SD38 | sdag-c | `isUnneededShiftMask` without known bits | `llrm-dagisel/src/x86_hooks.rs` |
| SD39 | sdag-d | emitter: no local frame indices, globals, segment overrides, inline asm or custom inserters yet | `selectiondag/emit.rs` |
| SD40 | sdag-c | `MaskedValueIsZero` is a structural subset of `computeKnownBits` | `llrm-dagisel/src/x86_addr.rs` |
| SD41 | sdag-b | no `DAGNodeDeletedListener`, no dead-node deletion in LegalizeDAG | `selectiondag/legalize_dag.rs` |

| SD200 | sdag-b | condition lowering without the reassociated/immediate bit-test forms and the carry folds | `selectiondag/x86_lowering.rs` |
| SD42 | sdag-c | `X86DAGToDAGISel::Select` has only the store and shift hand matchers so far | `llrm-dagisel/src/x86_select.rs` |
| SD43 | w9 | `AssertSext`/`AssertZext` dropped ahead of the matcher, not inside `SelectCodeCommon` | `selectiondag/dag_select.rs` |
| SD400 | sdag-d | `EmitLoweredSelect` per `CMOV_GR*`, no run or cascade look-ahead | `selectiondag/emit.rs` |
| SD403 | sdag-d | the DAG path (`--isel=dag`): no branch odds or counted-loop trip counts, calls, locals, register arguments, switch lowering or multi-register phis yet; exit D1e, review 2026-12-03 | `selectiondag/dag_function.rs` |
| SD404 | sdag-d | a rep-stos/rep-movs idiom for a word fill and a descending copy (`std; rep movs; cld`). The pin keeps both as counted loops, so this is a deviation: priced by the 486 loop at 4 instructions a trip against `rep stosw` at 5 + n clocks, and by the 5 bytes the loop's bounds check costs; unmeasured on a program. No code until priced on a program set | `selectiondag/emit.rs` |
| SD405 | sdag-d | real mode at -Os: a popped x87 argument takes an area of its own (`sub sp, n; mov bx, sp; fstp [bx]`) and the other arguments are pushes; the pin stores every argument into one area. Size: 10 bytes against 14 for one float and one word | `selectiondag/emit.rs` |
| SD442 | sdag-d | w2, m16: a constant `memset` the pin calls `memset` for (more than the inline limit, not tuned for size) and a size not known at compile time are a `rep stos` whose unit is the widest the size and alignment allow (stosw/stosd, the `66` prefix counted); m16 has no runtime `memset`. Exit: the runtime gains `memset` and both lower to the call | `selectiondag/builder/memops.rs` |
| SD201 | sdag-b | combiner: no `HandleSDNode`/`DAGUpdateListener`; reads RAUW events and the node count | `selectiondag/dag_combiner.rs` |
| SD202 | sdag-b | combiner: no `computeKnownBits`; folds see constants only | `selectiondag/dag_combiner.rs` |
| SD203 | sdag-b | `ExpandConstantFP`: no f80 shrinking (no APFloat), alignment by type | `selectiondag/legalize_dag.rs` |
| SD206 | sdag-b | division by constant: register-held types only, no exact/promoted forms, generic `sdiv` pow2 sequence (no cmov form) | `selectiondag/div_combine.rs` |
| SD207 | sdag-b | logic-of-setcc fold: `SETCC` only (no `SELECT_CC`), power-of-two query knows constants and `shl 1, x` | `selectiondag/logic_setcc.rs` |
| SD208 | sdag-b | far call through a stack cell: `X86ISD::LLRM_FARCALL`, the pin has no far call | `llrm-sdag/src/x86isd.rs` |
| SD209 | sdag-b | far access: `ES` set by a `CopyToReg` ordered by the chain only (no glue binds it to its access) | `selectiondag/legalize_types.rs` |
| SD210 | sdag-b | `reassociationCanBreakAddressingModePattern`: the vscale half, the global-address offset test and `hasUniqueMemOperand` are not translated; an offset is legal when it fits 32 bits | `selectiondag/reassociate.rs` |
| SD205 | sdag-b | ext-of-load fold: simple non-extending loads before operations are legal; every integer truncate is free (no `isTruncateFree`) | `selectiondag/dag_combiner.rs` |
| SD211 | sdag-b | `PromoteIntBinOp`/`ShiftOp`: a load operand is any-extended (the extend-of-load fold makes the extending load), no `AssertSext`/`AssertZext` look-through | `selectiondag/dag_combiner.rs` |
| SD212 | sdag-b | `SimplifyDemandedBits`: and/or/xor, shifts by a constant, zext/anyext/trunc only; no `ShrinkDemandedOp`, no narrowing rewrites, no SRA/ADD/SUB/MUL/SELECT cases | `selectiondag/demanded_bits.rs` |
| SD214 | sdag-b | `FLOG`/`FEXP`/`FLOG2`/`FEXP2`/`FSIN`/`FCOS`/`FATAN`/`FRINT` select as the x87 sequences the finalizer spells (`fln`, `fexp`, ...), not libm calls: no target has a libm (`Target::has_libm`); with one, the node is refused (the pin's libcall is not translated) | `llrm-dagisel/src/x86_llrm.rs` |
| SD215 | sdag-b | `X86ISD::FIST` of an `i16` selects `IST_Fp16m32/64/80`, which the pin has no pattern for (its `lrint` of a short is promoted); QB's `CINT` needs a word `fistp` | `llrm-dagisel/src/x86_fcmp.rs` |
| SD217 | sdag-b | `mergeConsecutiveStores` merges stores of constants and of loads: no vector-extract sources, no vector or truncating merged store, no rotate of two loads in reverse order; an address base is a frame index, a global or one node plus constant offsets (no index register); `allowsMemoryAccess` is true for every scalar the target holds | `selectiondag/combine_store_merge.rs` |
| SD218 | sdag-b | `expandBITREVERSE` is the byte-swap and mask form only (no loop of single bits for a width that is not a power of two; none is legal here) | `selectiondag/legalize_dag.rs` |
| SD219 | sdag-b | `combineAddOrSubToADCOrSBB` without the `ADOX` form (no ADX), the or/xor entry (`combineOrXorWithSETCC`) and the `ADD(ADC/SBB)`/`SUB(…, SBB)` folds | `selectiondag/x86_carry.rs` |
| SD227 | sdag-b | `visitBSWAP`: no half-width `bswap (shl x, c)` form, no `bswap (bitreverse x)` canonicalisation | `selectiondag/bswap_combine.rs` |
| SD246 | sdag-b | `visitABS`: no `foldABSToABD`, no `abs (sign_extend_inreg x)` narrowing | `selectiondag/abs_combine.rs` |
| SD245 | sdag-b | `visitCTLZ/CTTZ/CTPOP`: no `foldCTLZToCTLS`, no half-width `ctpop`; non-zero is what known bits prove | `selectiondag/ctcount_combine.rs` |
| SD254 | sdag-b | `visitRotate`: no demanded-bits simplification of the operand, no `distributeTruncateThroughAnd` | `selectiondag/rotate_combine.rs` |
| SD255 | sdag-b | w12: `-g` values under the DAG: `SDDbgValue` on the nodes of `llvm.dbg.value`, `transferDbgValues` on RAUW, `EmitDbgValue` to the notes `isel.rs::noted` makes (register locations, pieces, parameter homes); only lines, `dbg.declare` cells and parameters are carried so far | `selectiondag/dag_function.rs`, `emit.rs` |
| SD237 | sdag-b | w12, m16: a zero whose only reader is a `sub_8bit`/`sub_16bit` extract is made at that width, not as the pin's `MOV32r0` (a dword `xor` is 3 bytes at real mode, the 66 prefix) | `selectiondag/emit.rs` |
| SD221 | sdag-b | w12, m16: `shouldAvoidImmediateInstFormsForSize` does not count a call argument's store as a use (it is a `push imm` at real mode); the pin has no 16-bit target | `llrm-dagisel/src/x86_hooks.rs` |
| SD224 | sdag-b | w12, m16: tuned for size, the bytes past a copy's last dword are `movsw`/`movsb` glued after the `rep movs`, not a load and store from addresses made again (the pin's tail) | `selectiondag/builder/memops.rs`, `selectiondag/emit.rs` |
| SD225 | sdag-b | w12, m16: tuned for size, a constant copy of more than four pieces (dwords, a word, a byte; no overlap) is string moves, not the pin's four overlapping pairs | `selectiondag/builder/memops.rs` |
| SD223 | sdag-b | w12, m16: tuned for size, a constant memset whose stores take more bytes than a `rep stosb` fill is the fill; the pin counts stores (`MaxStoresPerMemsetOptSize`) | `selectiondag/builder/memops.rs` |
| SD226 | sdag-b | w11: a store of an x87 `fp_round` that nothing else reads is one truncating `fst`; the pin keeps the stack round trip (`f64 -> f32` is Expand), the old selector stores once | `selectiondag/x86_preprocess.rs` |
| SD216 | sdag-b | `visitLOAD` bypasses a dead non-volatile load and forwards a store to the load straight after it (little-endian, integer, offset 0); no alignment inference, indexed or sliced loads, no big-endian, float or same-size bitcast forwarding, and it runs at every opt level (the pin skips it at `-O0`, CC1); the root load is left alone (the pin's `HandleSDNode` holds it) | `selectiondag/combine_load.rs` |
| SD231 | sdag-b | the i64 divide routines are shared (one procedure per object) only where two or more sites call the same routine; one site pastes it inline. The pin has no such routine (a library call). Exit: a routine specialised per divisor | `backend/assemble.rs` |
| SD232 | sdag-b | the signed i64 divide by a dword divisor has routines that fix the sign of only the result the caller reads (`div64_s32q`, `div64_s32r`); the other divides keep both. The sign is a mask (`add; sbb`), not a flag test. Exit: the same for the wide and unsigned routines | `llrm-x86/src/helpers.rs` |
| SD230 | sdag-b | `visitSIGN_EXTEND_INREG` has no vector in-reg forms and no `reduceLoadWidth` narrowing of the load | `selectiondag/dag_combiner.rs` |
| SD233 | sdag-b | tuned for size, a constant memset whose stores take more bytes than the `rep stosb` setup (`FILL_BYTES`, the old selector's rule) is a fill; the pin counts stores | `selectiondag/builder/memops.rs` |
| SD229 | sdag-b | `visitBSWAP` has no half-width `shl` fold (no truncate-free query) and no `foldBitOrderCrossLogicOp` | `selectiondag/dag_combiner.rs` |
| DIV-OPTSIZE-1 | sdag-b | deliberate, size-driven: at `-Os` a constant divide stays a `div` (`isIntDivCheap` is true for `optsize` as well as `minsize`; the pin: `minsize` only). Flipping it: m32 -Os +250 B, m16 -Os +301 B, only matmul -3 and x_life -18/-13 gain. Exit: keep the divide unless the expansion forces spills (matmul's `urem i16 %x,3` +59 B, 4 spill slots) | `selectiondag/dag_function.rs` |
| SD222 | sdag-b | w12, QB: the entry block's `store 0` / `memset 0` to a local nothing has touched yet are not lowered when the runtime builds and zero-fills the frame (`B$ENRA`); the pin has no such runtime | `backend/prezeroed.rs` |
| SD220 | sdag-b | w12, m16: `SinkCast` also sinks an `addrspacecast` from the segment space to the far or huge one (the pin leaves address-space casts alone, x86 has none); left in one block its zero offset crosses every block edge in a register | `llrm-transforms/src/sinkcast.rs` |
| SD228 | sdag-b | w11, m16: an argument area split from its call by a select's branches is made again at the call (stays `push`); the pin's scheduler keeps the select out of the call sequence | `selectiondag/emit.rs` |
| SD238 | sdag-b | the vector registers' half of the variadic register save area (48..176) is left unwritten until floating point is selected at m64: a `double` passed to such a function is not read back | `selectiondag/builder.rs` |
| SD239 | sdag-b | where the pointer is a word (m16) an `i16` select stays a word: widened to `i32` every move takes an operand-size prefix. Exit: none (machine fact) | `selectiondag/x86_lowering.rs` |
| SD240 | sdag-b | `isMultiStoresCheaperThanBitsMerge` is true for two integer halves at m16 (the pin: a float and an integer half only): a far pointer or `long` is two word stores, not `shl; movzx; add`. Exit: none while the DAG keeps `BUILD_PAIR` | `x86/dag/actions.rs` |
| SD241 | sdag-b | `ExpandShiftWithKnownAmountBit`: a clear amount bit without a legal or custom funnel shift stays a parts node (the pin shifts, subtracts and ors) | `selectiondag/legalize_types.rs` |
| SD242 | sdag-b | `combineCarryThroughADD`: the `COND_A` form (a swapped `sub`) is not translated; nothing produces it at 32 bits yet. Exit: with `combineSBB`'s remaining folds | `selectiondag/x86_lowering.rs` |
| SD247 | sdag-b | `LowerFunnelShift`: the `isSHLDSlow` expansion of an `i16` funnel shift is not translated (no target of ours sets it) | `selectiondag/x86_lowering.rs` |
| SD249 | sdag-b | `reduceLoadWidth` of a constant `srl`/`sra`/`and` of a load only; no `freeze` look-through, no `shl` opening; every x86 narrow access is taken as legal (`isLegalNarrowLdSt`). Exit: none while x86 loads stay unaligned-legal | `selectiondag/reduce_load_width.rs` |
| SD243 | sdag-b | w12, m16: tuned for size, a dword constant argument with a byte-sized half is two `pushw`, not `pushd imm32`; the pin has no 16-bit push | `selectiondag/emit.rs` |
| SD244 | sdag-b | w12, m16, parked (not applied): a segment-to-far cast stays out of a loop it is made outside of. Measured: `tools/sizes.py -Os` over 315 programs 182540 -> 183693 B (+1153; deedlines +477, qbdemo +236), grep.bas 416 -> 461 lines, no program smaller; SD220's sink wins | `sinkcast.rs` |
| SD253 | sdag-b | w12: a function calling a hidden atomic other than `xchg` and an ordered store (`llrm.atomic.*`: load, cmpxchg, rmw ops) is selected by the old selector; the DAG has `ATOMIC_SWAP` only | `assemble.rs` |

| SD100 | sdag-a | a call's `TargetGlobalAddress` has no relocation flag (`[TF=7]`) | `selectiondag.rs` |
| SD101 | sdag-a | `freeze` folds away only for a constant or a `freeze` (`isGuaranteedNotToBeUndefOrPoison` is partial) | `selectiondag/fold.rs` |
| SD102 | sdag-a | an `invoke` has no `EH_LABEL`s and a `landingpad` builds nothing (QB's pad is entered by the runtime) | `selectiondag.rs` |
| SD105 | sdag-a | `visitFunnelShift` ports the constant-amount folds and the funnel of consecutive loads; the identity fold, variable-amount range folds and rotate fold are not ported | `selectiondag/combine_funnel.rs` |
| SD103 | sdag-a | `ExpandIntRes_MULFIX` takes its pieces from `forceExpandMultiply`, never `expandMUL_LOHI` | `selectiondag/legalize_types.rs` |
| SD104 | sdag-a | an unsigned `ExpandIntRes_MULFIXSAT` scale above a half shifts `HH`, where the pin shifts `HL` (upstream fixed it) | `selectiondag/legalize_types.rs` |
| SD107 | sdag-a | `lowerFaddFsub` never forms a horizontal add (SSE3 `haddps` is not described); scalar `FADD`/`FSUB` stay as they are | `selectiondag/x86_lowering.rs` |
| SD108 | sdag-a | `FAND`/`FOR`/`FXOR` of a scalar float against its pool mask select `andps`/`orps`/`xorps` directly (no `SCALAR_TO_VECTOR` node); `double` takes the `ps` rows `x86.instr` has | `crates/backend/llrm-dagisel/src/x86_fplogic.rs` |
| SD106 | sdag-a | `expandDIVREMByConstant` ports the unsigned forms; the sign-extended `MUL` shortcut of `expandMUL_LOHI` and the UDIVREM-decomposition of a missing UREM libcall are not ported | `selectiondag/expand_divrem_const.rs` |
| SD300 | sdag-c | the adapter answers conservatively: no known bits, extending loads or memory operands on machine nodes | `selectiondag/dagisel_adapter.rs` |
| SD301 | sdag-c | Select hand cases beyond shifts, stores and DIVREM (AND/OR/XOR shrink, MUL_LOHI, intrinsics, atomics) come with their patterns | `crates/backend/llrm-dagisel/src/x86_select.rs` |
| SD302 | sdag-c | the i8 DIVREM remainder is `shr ax, 8` then `movzx` of the byte, not `movzx` of AH (the LIR has no AH) | `crates/backend/llrm-dagisel/src/x86_divrem.rs` |
| SD303 | sdag-c | MUL_LOHI skips `orderRegForMul` (no live-in physical registers in the DAG) and handles i32 only | `crates/backend/llrm-dagisel/src/x86_mul.rs` |
| SD304 | sdag-c | `isKnownNeverZero` is the known-bits part only (a known set bit); the cttz `cmov` stays for values the pin proves non-zero by other means | `selectiondag/x86_lowering.rs` |
| SD306 | sdag-c | `expandABS` leaves out the `FREEZE` of its operand (selection deletes it again; same machine code) | `selectiondag/legalize_dag.rs` |
| SD307 | sdag-c | the m16 action table is the 32-bit one kept to the old selector's coverage, not derived from the pin (the pin has no 16-bit model); `tests/m16_coverage.rs` lists the functions that must stay selectable | `crates/codegen/llrm-sdag/src/actions.rs` |
| SD308 | sdag-c | the second half of a split access through a far pointer moves the offset word only (no carry into the selector); a huge pointer crossing a segment would wrap | `selectiondag/legalize_types.rs` |
| SD321 | sdag-c | `MatchRotate` matches constant shift amounts only: no `extractShiftForRotate`, disguised rotate, variable amounts (`MatchRotatePosNeg`/`MatchFunnelPosNeg`) or truncated rotate | `selectiondag/match_rotate.rs` |
| SD252 | sdag-b | `visitIMINMAX` has no FP-to-saturate folds, no `reassociateReduction`, no `vscale` fold and no flip to the opposite signedness (no min/max is legal on this target) | `selectiondag/iminmax_combine.rs` |
| SD234 | sdag-b | an x87 `fld1`/`fldz` is loaded once for its readers in a block until the next call or barrier (the pin: one `LD_F1`, `fadd st(1),st`); the old rule loaded it per reader | `selectiondag/emit.rs` |
| SD235 | sdag-b | a same-type `fpround` read only by a store of its type in the block is not rounded through a stack temp: the pin folds `store (fp_round x)` into a truncating store (`visitSTORE`) | `selectiondag/builder.rs` |
| SD236 | sdag-b | `visitBITREVERSE` has no `clmul` fold (no carry-less multiply node here) | `selectiondag/bitreverse_combine.rs` |
| SD250 | sdag-b | m16: a dword `lea` takes a scaled index (the pin's `matchAddressRecursively` has no 16-bit exclusion) only when not optimizing for size: with the 0x66 and 0x67 prefixes it is longer than the `imul` or `shl; add` (-Os +214 bytes, 26 programs). Exit: none (machine fact) | `llrm-dagisel/src/x86_addr.rs` |
| SD251 | sdag-b | `visitBITCAST`: only the scalar `(conv (load x))` fold; no vector, `freeze`, `AssertNoFPClass` or range-metadata cases | `selectiondag/bitcast_combine.rs` |
| SD256 | sdag-b | w6: at optsize an `f32` call argument that is only a memory load (one IR user) is cast to its bits so the load folds to an integer load and the argument is `push dword ptr [m]`, where the pin does `fld; fstp` to the slot; a load shared in the DAG is read again as the integer, its old chain and the new one joined by a `TokenFactor`; the store combine undoes the cast only for a volatile or extending load. QCport -Os m16 dag -1017 B | `selectiondag/builder.rs` (`pass_float_bits`), `dag_combiner.rs` (`visit_store`) |
| SD312 | sdag-c | DIV-OPTSIZE-1: `isIntDivCheap` is `size \|\| minsize`, the pin's is `minsize`: a constant divide stays `div`/`idiv` at -Os (~40 clocks on a 486 against ~13 for the multiply; div_constants_long m32 -Os 5095 B against the pin's 5463) | `selectiondag/dag_function.rs` |
| SD311 | sdag-c | a fill or copy through a huge pointer calls the target's stepped routine inline with its operands in the registers it reads (the pin has no huge pointers); a `memmove` of no proven direction, or a huge pointer beside a near one, is refused | `selectiondag/builder/memops.rs` |
| SD310 | sdag-c | a QB runtime routine's answer in the flags is `LLRM_CALL_FLAGS(CopyFromReg EFLAGS)`, read only by a compare with zero; the pin's calls answer in registers | `llrm-sdag/src/x86isd.rs` |
| SD305 | sdag-c | `llrm.ia16.*` nodes (`LLRM_ASM`/`_WINDOW`/`_CODE`) pass through selection as the pseudo opcode 30010, `SEGMENT_OF` as 30011 and `LLRM_FARCALL` (24584) as 30012 and `LLRM_CODE` (24582) as 30013; not the pin's `INLINEASM` | `crates/backend/llrm-dagisel/src/x86_llrm.rs` |
| SD443 | sdag-d | w2, ipo: a second IPSCCP runs after `promote.` (the pin runs it once, before the CGSCC walk) because our ArgumentPromotion promotes a recursive function's pointer-holding descriptor and the pin refuses (ArgumentPromotion.cpp#L581); the length folds through the recursion, queens bounds checks go. Fix-by 2026-11-07. Exit: by-value slice parameters (#6140), then #6159 merges and the extra IPSCCP goes | `llrm-transforms/src/interprocedural.rs`, `tests/nearcalls.rs` |
| SD444 | sdag-d | w2: where the CPU's multiply ends early on the r/m operand (tune: `smul_min_w4` < `smul_max_w4`, the 486 and 386), a constant operand of `UMUL_LOHI` goes to EAX so the variable is the r/m operand, the multiplier the early-out reads (Intel 240440-002 Table 10.1 note 3: `10 + max(log2|m|, n)` clocks, 13 to 42 for `MUL r32`, `m` the multiplier beside the register or memory operand, not the accumulator; positive control: r/m = 200 -> 18 clk, r/m = 0x82f63b7f -> 42, 0 -> 13). The pin keeps operand 0 in EAX ([X86ISelDAGToDAG.cpp#L6085](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L6085)). Permanent target tuning; P5+ keep the pin's order | `llrm-dagisel/src/x86_mul.rs` |
| SD445 | sdag-d | w9, m16 (and any target that prices a divide above a multiply): `isIntDivCheap` is priced per constant divisor and width, not `minsize` alone ([X86ISelLowering.cpp#L65389](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L65389)): a 16-bit `/` or `%` stays `div` where the sheet's `div r16` (i486: 24) plus 25% is below the multiply-high sequence (recsum 106017 -> 82025 clocks, 80 -> 60 B, main 82025/60; recpow 569025 -> 544225, 267 -> 220 B; matmul bq16/nib O2 -2%). A 32-bit divide stays the sequence (a two-clock sheet win lost +0.9% on switch, recmany). Exit: the sheet's `imul r32` is a midpoint of 13..42; when it is priced by operand, drop the 25% margin and re-sweep | `selectiondag/div_combine.rs` |
| SD520 | sdag-e | w15: arm64 predicates that answer false by name (`atomic_store_hint`, `exclusive_load_intrinsic`, `f16_immediate`, `sve_increment`, `sme_streaming_mode`) guard a feature or node the scalar subset never builds; false is the pin's answer on apple-m1. Exit: with that feature | `llrm-dagisel/src/aarch64_predicates.rs` |
| SD521 | sdag-e | w15: `isWorthNegatingImm` (`negated_immediate_cheaper`) and `alignedglobal` (`aligned_global_literal`) are untranslated and answer false: `add x, imm` keeps the add, a global is `adrp` + `add`. Exit: `isWorthNegatingImm` over the immediate expansion; the global's alignment as a `Dag` query | `llrm-dagisel/src/aarch64_predicates.rs` |
| SD522 | sdag-e | w15: complex patterns `SelectExtractHigh`, `SelectDupZero`, `SelectCVTFixedPos*` from a constant pool, and the extend of a vector lane answer false. Exit: vectors; the constant pool | `llrm-arm64/src/dag/addr.rs`, `hooks.rs` |
| SD523 | sdag-e | w15: compares lack the `x == 1` to `CBZ` normalization, the `CSET` + `BR_CC` fold, the zero-extending-load `CMN`, the `lookThroughSignExtension` `TST` form, float vector-compare masks and the `nsz` zero rewrite; `BRCOND` expands to `BR_CC` (the shared legalizer has no `ExpandNode` arm). Exit: each fold its own translation (B4..B6) | `llrm-arm64/src/dag/lowering.rs` |
| SD524 | sdag-e | w15: the `hasALULSLFast` multiply decompositions (apple-m1 lacks the feature), vector/SVE/`rdsvl` parts, indexed accesses in `isOnlyUsedAsMemoryAddress`, and `N - x` (`NEG`, `NOT`). Exit: vectors; indexed addressing; a pre-selection hook | `llrm-arm64/src/dag/combine.rs` |
| SD525 | sdag-e | w15: no AArch64 `PerformDAGCombine`, no `PreprocessISelDAG`; a Custom operation is refused. Exit: B3 | `llrm-arm64/src/dag/pipeline.rs` |
| SD526 | sdag-e | w15: calls without `f16`/`fp128`/vectors, `sret`, HFAs, `byval`, varargs, tail calls needing stack arguments, a GOT callee; `accepts` refuses what it sees. Exit: B3 | `llrm-arm64/src/dag/call.rs` |
| SD527 | sdag-e | w15: a floating-point compare is not a conditional-compare leaf (`FCCMP`); the tree is refused. Exit: floating-point conditional compares | `llrm-arm64/src/dag/ccmp.rs` |
| SD528 | sdag-e | w15: `emit` has rows for the integer and float families listed in its header only; any other machine opcode (the `BFM*` family, `MOVaddrJT`/`JumpTableDest32`, atomics) is the named error `machine opcode X has no arm64 instruction yet`, and `--isel=shadow` falls back. Exit: C2..Cn | `llrm-arm64/src/dag/emit.rs` |
| SD529 | sdag-e | w15: bitfield insert: an `OR` with no useful bits is not selected to `IMPLICIT_DEF`; the `any_extend`/`truncate` forms of `isBitfieldExtractOpFromAnd` and the `and (any_extend (shl ..))` form are not matched. Exit: the `Widen` helper | `llrm-arm64/src/dag/bitfield.rs` |
| SD530 | sdag-e | arm64-darwin: the pre-allocation CSE, hoist and sink over `MFunc` are private (`prepass.rs`): flag-setting add/sub merge, dead flag-setting defs become `wzr`, unread flags give the plain opcode, a value read only below its block is made at the nearest dominating block. Exit: arm64 MIR is `LirBody` and the shared MachineCSE/LICM/Sink run | `llrm-arm64/src/prepass.rs`, `flagcse.rs` |
| SD531 | sdag-e | w15: an indirect tail call moves its callee to the fixed `x9` (`sibcall::TARGET`, `TCRETURNri` by name); the pin has the allocator pick any `tcGPR64` register. Exit: register-class constraints (`tcGPR64`) in the arm64 allocator | `llrm-arm64/src/dag/emit.rs`, `sibcall.rs` |

Next free: SD44..SD99 closed; blocks: a SD100, b SD200, c SD300, d SD400 (next free per owner in their rows). A PR that takes a number adds its row in the owner's block; `tests/selectiondag_registry.rs` fails on a number declared twice, without a row, in the closed range or outside its owner's block.

[isel-run]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L599
[isel-cg]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L941
[isel-do]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L1281
[isel-common]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L3351
[b-icmp]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L3808
[b-sel]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L3863
[b-gep]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L4493
[b-load]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L4735
[b-store]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L4917
[b-call]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L9256
[b-switch]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L12878
[b-work]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L12492
[b-peel]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L133
[c-run]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L1845
[c-combine]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L2157
[c-add]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L3295
[c-and]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L7861
[c-setcc]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15048
[c-load]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L22110
[c-store]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L24689
[lt-run]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeTypes.cpp#L196
[ld-legalize]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L6301
[ld-op]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L982
[s-run]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/ScheduleDAGSDNodes.cpp#L55
[s-build]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/ScheduleDAGSDNodes.cpp#L536
[s-burr]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/ScheduleDAGRRList.cpp#L3113
[e-node]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/InstrEmitter.cpp#L1003
[x-regprops]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L2934
[x-ctor]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L131
[x-sched]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L148
[x-lower]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L34783
[x-replace]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L34968
[x-combine]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L63938
[x-cmp]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L24085
[x-setcc]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L25293
[x-select2]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L25834
[x-formal]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLoweringCall.cpp#L1710
[x-select]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L5513
[x-rmw]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3925
[x-bext]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L4149
[x-shamt]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L4664
[x-addr]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2053
[x-addrrec]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2677
[x-seladdr]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3202
[x-lea]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3329
[x-pre]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L988
[x-post]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L1643
[x-16]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L1926
[tg-emit]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/utils/TableGen/DAGISelEmitter.cpp#L152
[tg-table]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/utils/TableGen/DAGISelMatcherEmitter.cpp#L1602
[li-setcc]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L1371
[li-select]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L1350
[li-const]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L4116
[li-addsub]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L3725
[li-mul]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L4540
[li-shiftc]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L3296
[li-esetcc]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L3589
[li-load]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L4414
[ld-loads]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L669
[ld-stores]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L492
[ld-expand]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L3210
[ld-lib]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L4734
[ld-promote]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L5432
[ld-libcall]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L2115
[x-lowered]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L37564
[x-inserter]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L38957
[x-parts]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L20256
[x-sint]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L20628
[x-uint]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L21136
[x-fpint]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L22157
