# Pin pipeline slots against llrm's pipeline

Owner: funcattrs-port. Rows: see Counts. MISSING rows carry the owner the coordinator assigned on 10-22 (plan rows or N/A with evidence by 11-09).

Output of the slot test (funcattrs-port, 2026-10-22; first table 1442783c8e6d, corrected here): the pin's `-print-pipeline-passes` at -O1, -O2, -O3, -Os
(tests/fixtures/pin-pipeline, tools/pairs/pin_pipeline.py; LLVM d1106deb71cc, opt md5 e8430a02d9bc7c91c31a8fcb5f37d0a1; target m32: i386 -march=i486, no SSE).

## Instrument checks

1. Source of each row. Function-list slots (Early, Cleanup, Simplify, Late) come from the REGISTRY (`parity::REGISTRY`, the slots `llvm_order` builds at that level). Module-level passes are not in the registry, so the first table read them all `none`; they are now matched against the EXECUTED order: `--dump` of d_faces -O1, -O2 and -Os (LLRM_* unset, `llrm-cc -m32 -march=i486`), whose stage files are numbered in the order the manager ran them: `globalopt, ports, stamp, pipeline (early list), ipo-early, sccp, pipeline (cleanup list), walk, pipeline (late list), interprocedural, writeonly, globaldce, available-externally, calleepop` and then llrm's own backend-facing passes (rotate, lsr, window, ...); -Os drops the rotate/fold/algebraic/indvars/dead/licm rounds. This is also what pipeline.rs `recorded` adds, in that order. Rows of the Module phase say `EXECUTED` and name the stage.
2. -Os. The pin has no -Os pipeline: `opt -passes='default<Os>'` stops with `The optimization level "Os" is no longer supported. Use O2 in conjunction with the optsize attribute instead.` (run at d1106deb71cc). The other route, `clang -Os -mllvm -print-pipeline-passes`, prints the same list as `clang -O2` (cross-checked in the fixture's header, md5 c28bf2ee...). So the -Os fixture is the -O2 list with optsize on the IR, and -Os counts equal -O2's.

## Counts (pin slots)

Every pin slot is in exactly one state (the test refuses a row without one; `none` is not a state): MATCH (ours stands at that position), N/A (cannot apply: vectors deferred, coroutines, target; with the reason), EXECUTED (a stage of ours outside the registry runs it: the module stages of the d_faces --dump order), DEVIATE (a stated difference: `ipsccp`, `deadargelim` and the ours-only slots), MISSING (not translated, owner and date), ORDER (stands out of the pin's order, by position).

| level | MATCH | N/A | EXECUTED | DEVIATE | MISSING | ORDER | slots |
|---|---|---|---|---|---|---|---|
| -O1 | 38 | 20 | 10 | 11 | 24 | 1 | 104 |
| -O2 | 50 | 25 | 10 | 5 | 27 | 0 | 117 |
| -O3 | 50 | 26 | 10 | 6 | 29 | 0 | 121 |
| -Os | 50 | 25 | 10 | 5 | 27 | 0 | 117 |

At -O2 the 66 slots the first table called `none` are 10 EXECUTED + 29 MISSING + 25 N/A + 2 DEVIATE (ipsccp, deadargelim); its 2 `ours-only` are DEVIATE rows; 48 + 5 + 10 + 29 + 25 = 117 slots. -Os is the -O2 list. (-O1: the one ORDER slot is memcpyopt.) MISSING counts slots, not passes: the three late `instcombine` runs are three rows of one pass.

## MISSING rows ranked by effect

`tools/pairs/pin_ablate.py`: the pin's -O2 pipeline (fixture) over 12 bench C programs (crc, nbody, bintree, queens, shellsort, matmul, sieve, quicksort, mandel, fib, hanoi, histo; the pin's clang -O0 IR, m32), with one instance of a pass removed at a time; effect = change in instructions of the pin's output. The instrument sees effects (without gvn +50 instructions in 5 of 12 IRs, indvars +100 in 7, loop-unroll-full +77 in 4, early-cse +63 in 2). Result for the pass instances ours has no slot for (the full list is the table below): late `tailcallelim` +1 instruction in 1 of 12; every other MISSING pass 0 of 12. The corpus is small C without libm, memcmp, assumes or SSE: a 0 is not evidence the pass never matters (QB/QCport/Nib corpora were not measured: the pin cannot parse llrm's QB IR), only that these 12 do not need it. Order for owners: (1) late instcombine/tailcallelim/loop-rotate/loop-unroll/sroa/loop-deletion (the late list is where the pin cleans up after the loop passes; ours runs none of them, measured effect small on this corpus), (2) rpo-function-attrs, argpromotion (-O3), constmerge (-Os data bytes), (3) the rest.

Rows with owner `unassigned` go to the coordinator on 10-26; the test `every_pin_slot_is_matched_or_accounted_for` fails on a slot without a row, on a row nothing needs, and on an inversion not in an ORDER row.

## N/A evidence

Every N/A row says why the slot has no work for us, shown at the slot: `tools/pairs/pin_noop.py` runs the pin's PRINTED pipeline
(`default<O2>`, the same list on optsize IR for -Os, `default<O3>`; the fixtures) whole and with every instance of one pass removed
(`-passes=<list minus X>`) over 76 IRs, and compares the final IR: identical on every IR is the evidence, one that differs would make the row
MISSING. The IRs: 12 bench C programs and 64 of the 65 QCport modules, each the pin's own clang -O0 IR (`-m16 -march=i386` and the macros
of qcport.py; the struct-size asserts of the headers removed in a copy, one module, savegame, still fails on a static assert), the pin
d1106deb71cc, opt md5 e8430a02d9bc7c91c31a8fcb5f37d0a1. (An earlier version ran each pass alone over IR already optimized by `default<O2>`: a
weak test, since the pipeline had done the work first; replaced.)

Controls: removing `sroa` changes the final IR of 70 of 76 IRs at O2 (71 at Os, 70 at O3); with SSE on (pentium4), removing
loop-vectorize and slp-vectorizer changes 47 of 76 at O2 (54 at Os, 46 at O3): so the vectorizer no-ops are the target's (i486, no SSE),
and the user's ruling of 10-20 (no vector MIR until 32-bit parity) is what keeps them out. Coroutines, OpenMP, profile data, `-pg`,
`-ftrivial-auto-var-init`, remarks and `#pragma comment` have no producer in our frontends (QB, C through Open Watcom, Nib). IRs whose final IR differs without the pass:

| pass | O2 | Os | O3 |
|---|---|---|---|
| ee-instrument | 0 | 0 | 0 |
| speculative-execution | 0 | 0 | 0 |
| vector-combine | 0 | 0 | 0 |
| move-auto-init | 0 | 0 | 0 |
| coro-elide | 0 | 0 | 0 |
| inject-tli-mappings | 0 | 0 | 0 |
| loop-vectorize | 0 | 0 | 0 |
| slp-vectorizer | 0 | 0 | 0 |
| transform-warning | 0 | 0 | 0 |
| annotation-remarks | 0 | 0 | 0 |
| chr | 0 | 0 | 0 |
| memprof-remove-attributes | 0 | 0 | 0 |
| annotation2metadata | 0 | 0 | 0 |
| forceattrs | 0 | 0 | 0 |
| coro-early | 0 | 0 | 0 |
| coro-split | 0 | 0 | 0 |
| coro-annotation-elide | 0 | 0 | 0 |
| coro-cleanup | 0 | 0 | 0 |
| openmp-opt | 0 | 0 | 0 |
| openmp-opt-cgscc | 0 | 0 | 0 |
| recompute-globalsaa | 0 | 0 | 0 |
| alloc-token | 0 | 0 | 0 |
| cg-profile | 0 | 0 | 0 |
| rel-lookup-table-converter | 0 | 0 | 0 |
| lower-comment-string | 0 | 0 | 0 |

`chr` is in the O3 list only; it is 0 there too. No N/A row changed state.

## MISSING list (for owners)

| section | pass | levels | ablation (pin -O2, 12 bench C IRs, one instance removed) / note |
|---|---|---|---|
| Early | lower-expect | -O1,-O2,-O3,-Os | llvm.expect reaches MIR (hir/mir.rs) and nothing lowers it; ablation 0 of 12 IRs |
| Simplify | libcalls-shrinkwrap | -O1,-O2,-O3,-Os | errno-setting libm calls; ablation 0 of 12 IRs |
| Simplify (inside instcombine) | SimplifyLibCalls (`LibCallSimplifier`) | -O1,-O2,-O3,-Os | MISSING, priority after the m16 -Os growers. `copysign`, `fabs`, `floor`, `ceil`, `trunc`, `rint` calls become `llvm.*` intrinsics and `sqrt` one without errno; ours keeps the call (`double copysign(double,double)` at m16 -O2 --isel=dag: `jmp copysign`). Evidence for low weight: 0 of the c corpus calls libm, and x86 expands FSIN/FREM/FCOPYSIGN back to the same libcalls, so the DAG builder's `visitUnaryFloatCall`/`visitBinaryFloatCall` rows (selectiondag-builder-gaps.md) change nothing until this pass exists. Owner: unassigned |
| Late | drop-unnecessary-assumes | -O1,-O2,-O3,-Os | ablation 0 of 12 IRs |
| Late | lower-constant-intrinsics | -O1,-O2,-O3,-Os | ablation 0 of 12 IRs |
| Late | loop-rotate | -O1,-O2,-O3,-Os | the late loop nest's rotate; ablation 0 of 12 IRs (the early one: +51 instructions, matched) |
| Late | loop-deletion | -O1,-O2,-O3,-Os | ablation 0 of 12 IRs |
| Late | loop-distribute | -O1,-O2,-O3,-Os | ablation 0 of 12 IRs |
| Late | infer-alignment | -O1,-O2,-O3,-Os | translated (inferalignment.rs); DEVIATE 1: globals not raised |
| Late | instcombine | -O1,-O2,-O3,-Os | the three late runs; ours has no combine slot in the late list |
| Late | loop-unroll | -O1,-O2,-O3,-Os | the late loop-unroll<O2>; ablation 0 of 12 IRs |
| Late | sroa | -O1,-O2,-O3,-Os | the late sroa<preserve-cfg;aggregate-to-vector>; ablation 0 of 12 IRs |
| Late | alignment-from-assumptions | -O1,-O2,-O3,-Os | ablation 0 of 12 IRs |
| Late | loop-sink | -O1,-O2,-O3,-Os | ablation 0 of 12 IRs |
| Late | div-rem-pairs | -O1,-O2,-O3,-Os | ablation 0 of 12 IRs |
| Late | mergeicmps | -O1,-O2,-O3,-Os | ablation 0 of 12 IRs |
| Late | expand-memcmp | -O1,-O2,-O3,-Os | ablation 0 of 12 IRs |
| Module | called-value-propagation | -O1,-O2,-O3,-Os | indirect calls only; not measured |
| Module | globaldce | -O1,-O2,-O3,-Os | globaldce (once). The pin's second globaldce, after the Late list, has no stage of ours: 0 B on all 65 QCport modules at -Os m16 (pin ablation, 2026-10-23), so N/A until a row moves (owner w-6) |
| Module | rpo-function-attrs | -O1,-O2,-O3,-Os | no stage of ours is known to do it; unverified |
| Module | constmerge | -O1,-O2,-O3,-Os | merging identical constants: -Os m16 on 65 QCport modules through the pin (llc obj sizes, constmerge removed): +58 B of 399336 (0.015%), 2 modules (game/mdl_ai +44, ui/sbar +14); a row of low weight |
| Simplify | jump-table-to-switch | -O2,-O3,-Os | x_funcptr: pin -O2 i486 makes 3 switches, none with -enable-jump-table-to-switch=false; the 12-IR ablation (0 differ) has no function-pointer table |
| Simplify | extra-simple-loop-unswitch-passes | -O2,-O3,-Os | ablation 0 of 12 IRs |
| Simplify | mldst-motion | -O2,-O3,-Os | ablation 0 of 12 IRs |
| Early | callsite-splitting | -O3 | -O3 only; not measured |
| Module | argpromotion | -O3 | -O3 only; argpromotion.rs exists, where it runs is not in the registry: unverified |

The second `globaldce`, `constmerge` and `rpo-function-attrs` the pin runs after the SCC block are rows above (globaldce's is in the EXECUTED row's note). The ranking is provisional: the 12 bench C IRs are a sample (late tailcallelim was the only mover; QB-shaped IRs and more C programs are untested). Follow-up by 10-27: widen the ablation to the QCport modules' stage IRs (65 modules, -Os too) through pin.py and report any pass that moves bytes.

## QCport-stage ablation (the follow-up of the MISSING ranking)

`tools/pairs/pin_ablate.py` over the 65 QCport modules' stage IRs (`llrm-cc -m16 -march=i386 -O2 --dump`, `01-globalopt.ll`; the pin parses
all 65 once `threeway` and `flagsresult`, llrm's own attributes, are dropped), the pin's -O2 list with one instance of a pass removed at
a time; at -O2 and at -Os (the -O2 list on `optsize` IR: `LLRM_ABLATE_OPTSIZE=1`); baseline 63213 instructions (-O2), 51432 (-Os).
The metric is the pin's output instruction count, not object bytes. The instances that still have no slot of ours and move something:

| pass (instance) | -O2: IRs changed, delta instructions | -Os | what it is |
|---|---|---|---|
| constraint-elimination | 5 of 65, +11 | 4 of 65, +13 | Simplify list (translated, slot 699: w-5) |
| mldst-motion | 4, +11 | 4, +9 | Simplify list (MISSING, w-4) |
| late instcombine (the last of the Late list) | 3, +17 | none | Late list (MISSING, w-4) |
| late loop-unroll<O2> | 3, -95 (the unroll adds 95) | none | Late list (MISSING, w-3) |
| late loop-rotate | 3, -3 | 4, -2 | Late loop nest (MISSING, w-3) |
| late sroa<preserve-cfg;aggregate-to-vector> | 1, -1 | 1, -1 | Late list (MISSING, w-4) |
| infer-alignment | 1, +2 | none | Late list (MISSING, w-1) |
| loop-instsimplify | 1, -1 | 1, -1 | loop-mssa nest (translated, slot 715: w-5) |

Nothing reaches 0.05% of the baseline except the late `loop-unroll` at -O2 (95 instructions, 0.15%: it makes code bigger, for speed):
the next most is `constraint-elimination` at -Os, 13 of 51432 (0.025%). The ranking of the first table stands, with these eight as
the ones that move anything on QCport; every other MISSING pass changes 0 of 65 IRs at both levels. Instances our slots already
match (`tailcallelim`, the early `loop-rotate`, `sroa`, `instcombine`) move code, as expected; they are in the raw output
(`qcablate_O2.txt`, `qcablate_Os.txt`).

## Order structure

The pin's pipeline is nested, not a flat list of slots (`PassBuilderPipelines.cpp` at d1106deb71cc: [buildInlinerPipeline L973](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Passes/PassBuilderPipelines.cpp#L973), the CGSCC members [L1047-L1049](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Passes/PassBuilderPipelines.cpp#L1047-L1049), [L1074](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Passes/PassBuilderPipelines.cpp#L1074)). `default<O2>` as the pin prints it, params shortened, `require`/`invalidate` left out:

```
memprof-remove-attributes
annotation2metadata
forceattrs
inferattrs
coro-early
function<…>(
  ee-instrument<…>
  lower-expect
  simplifycfg<…>
  sroa<…>
  early-cse<…>
)
openmp-opt
ipsccp
called-value-propagation
globalopt
function<…>(
  mem2reg
  instcombine<…>
  simplifycfg<…>
)
always-inline
function(
)
cgscc(
  devirt<…>(
    inline
    function-attrs<…>
    openmp-opt-cgscc
    function<…>(
      sroa<…>
      early-cse<…>
      speculative-execution<…>
      jump-threading
      correlated-propagation
      jump-table-to-switch
      simplifycfg<…>
      instcombine<…>
      aggressive-instcombine
      libcalls-shrinkwrap
      tailcallelim
      simplifycfg<…>
      reassociate
      constraint-elimination
      loop-mssa(
        loop-instsimplify
        loop-simplifycfg
        licm<…>
        loop-rotate<…>
        licm<…>
        simple-loop-unswitch<…>
      )
      simplifycfg<…>
      instcombine<…>
      loop(
        loop-idiom
        indvars
        extra-simple-loop-unswitch-passes
        loop-deletion
        loop-unroll-full
      )
      sroa<…>
      vector-combine
      mldst-motion<…>
      gvn<…>
      sccp
      bdce
      instcombine<…>
      dfa-jump-threading
      jump-threading
      correlated-propagation
      adce
      memcpyopt
      dse
      move-auto-init
      loop-mssa(
        licm<…>
      )
      coro-elide
      simplifycfg<…>
      instcombine<…>
    )
    function-attrs
    function(
    )
    coro-split
    coro-annotation-elide
  )
)
deadargelim
coro-cleanup
globalopt
globaldce
elim-avail-extern
rpo-function-attrs
recompute-globalsaa
function<…>(
  drop-unnecessary-assumes
  float2int
  lower-constant-intrinsics
  loop(
    loop-rotate<…>
    loop-deletion
    loop-interchange
  )
  loop-distribute
  inject-tli-mappings
  loop-vectorize<…>
  drop-unnecessary-assumes<…>
  infer-alignment
  loop-load-elim
  instcombine<…>
  simplifycfg<…>
  slp-vectorizer
  vector-combine
  instcombine<…>
  loop-unroll<…>
  transform-warning
  sroa<…>
  infer-alignment
  instcombine<…>
  loop-mssa(
    licm<…>
  )
  alignment-from-assumptions
  loop-sink
  instsimplify
  div-rem-pairs
  mergeicmps
  expand-memcmp
  tailcallelim
  simplifycfg<…>
)
alloc-token
globaldce
constmerge
cg-profile
rel-lookup-table-converter
lower-comment-string
function(
  annotation-remarks
)
```

Ours as executed (`--dump` of d_faces -O2, stage files in the order the manager ran them; pipeline.rs `recorded` with `llvm_order`):

```
module
  globalopt                      GlobalOpt (a subset)             pin: globalopt (first)
  ports                          target I/O ports                 pin: -
  stamp                          interprocedural::Stamp           pin: inferattrs
  function  Early list           per body, all bodies             pin: function(Early)
  ModuleIpo                      ipsccp, ipa-cp, deadargelim      pin: ipsccp, deadargelim (W1: before the Simplify list; the pin runs it after the CGSCC block)
  function  Cleanup list         GlobalCleanup                    pin: function(mem2reg, instcombine, simplifycfg)
  Walk (program pass)            callees first, per function: inline sites (inlinecost::sites), then the Simplify list over it, once
  function  Late list            DEVIATE: before the interprocedural step; the pin runs it after the module passes below
  Interprocedural                inline again, ipa-cp clones, what each body does (function-attrs); changed bodies rerun the Simplify list
  writeonly, globaldce, available-externally, calleepop           pin: globalopt, globaldce, elim-avail-extern (+ fastcc)
  llrm's own MIR passes          rotate (header copy), lsr, window, ...   pin: -
```

Differences, by order and contents (each a row of the table or of the list below):

1. `devirt<4>`: the pin repeats the whole SCC block up to four times while indirect calls are devirtualized; ours runs the Simplify list once per body in `Walk` and again in `Interprocedural` (walk.rs W2a: per function, callee-first, not per SCC, no devirtualization repeat). Both go callee-first (`Walk`, walk.rs): the order is not the difference; the SCC unit and the repeat are. Row: DEVIATE W2a.
2. `function-attrs` inside the SCC: the pin runs `function-attrs<skip-non-recursive-function-attrs>` right after `inline` and `function-attrs` after the function block, so a callee's attributes (readonly, nounwind, norecurse, argmem) are inferred from its simplified body before the caller is simplified. Ours states facts once up front (`stamp`) and infers them in `Interprocedural`, after the walk. Row: MISSING inside the walk, owner funcattrs-port, 2026-11-13 (evidence: w-1's `_cmd_tokenize@3` attr difference).
3. `deadargelim` position: before the Simplify list in ours, after the CGSCC block in the pin (walk.rs W1). `always-inline` is a separate pass before the SCC block in the pin, inside `Walk`'s site decision in ours. Row: DEVIATE W1.
4. After the SCC block the pin runs `deadargelim, coro-cleanup, globalopt, globaldce, elim-avail-extern, rpo-function-attrs, recompute-globalsaa`, then the Late function list, then `globaldce` again and `constmerge`. Ours runs the Late list before `Interprocedural`, then `writeonly`, `globaldce` (once), `available-externally`, `calleepop`; no `rpo-function-attrs`, no second `globaldce`, no `constmerge` (Module rows MISSING in the table).
5. Contents of the function block `function<eager-inv;no-rerun>(...)` against our Simplify list, pass by pass: the pin's `speculative-execution`, `jump-table-to-switch`, `libcalls-shrinkwrap`, `constraint-elimination`, `loop-instsimplify`, `loop-simplifycfg`, `extra-simple-loop-unswitch-passes`, `vector-combine`, `mldst-motion`, `move-auto-init`, `coro-elide` have no slot of ours (classified in the table: N/A for the target and frontends, MISSING for the rest, x_funcptr: pin -O2 i486 makes 3 switches, none with -enable-jump-table-to-switch=false; the 12-IR ablation (0 differ) has no function-pointer table); ours has `loopsimplify`/`lcssa` pairs at :764 and :772 the pin's loop adaptors make implicit (ours-only rows). The order of the matched ones is the pin's (no inversion at -O2/-O3/-Os; by position).
6. Not here: the `_sift` PRE/smax split is with w-1/w-4 via w-5.

## Table

(generated: `LLRM_PIN_SLOT_TABLE=FILE cargo test -p llrm-transforms --lib pin_slot`; FILE.counts has the counts above)

| level | phase | pin slot | nest | state | ours | note | owner |
|---|---|---|---|---|---|---|---|
| -O1 | Early | ee-instrument<> | function<eager-inv> | N/A |  | -pg/-finstrument-functions: no frontend asks for it Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Early | lower-expect | function<eager-inv> | MATCH | lowerexpect@1177:Translated |  |  |
| -O1 | Early | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;no-switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | MATCH | decide@1178:Partial |  |  |
| -O1 | Early | sroa<modify-cfg> | function<eager-inv> | MATCH | sroa@1179:Translated |  |  |
| -O1 | Early | early-cse<> | function<eager-inv> | MATCH | earlycse@1180:Translated |  |  |
| -O1 | Cleanup | mem2reg | function<eager-inv> | MATCH | promote@1241:Translated |  |  |
| -O1 | Cleanup | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MATCH | combine@1242:Partial + dead@1242:Old |  |  |
| -O1 | Cleanup | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | MATCH | decide@1244:Partial |  |  |
| -O1 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | sroa@646:Translated |  |  |
| -O1 | Simplify | early-cse<memssa> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | earlycse@649:Translated |  |  |
| -O1 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@675:Partial |  |  |
| -O1 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@677:Partial + dead@677:Old |  |  |
| -O1 | Simplify | libcalls-shrinkwrap | cgscc/devirt<4>/function<eager-inv;no-rerun> | MISSING |  | errno-setting libm calls; ablation 0 of 12 IRs | funcattrs-port, 11-09 |
| -O1 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@690:Partial |  |  |
| -O1 | Simplify | reassociate | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | reassociate@696:Translated |  |  |
| -O1 | Simplify | loop-instsimplify | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | loop-instsimplify@715:Translated |  |  |
| -O1 | Simplify | loop-simplifycfg | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | loop-simplifycfg@716:Translated |  |  |
| -O1 | Simplify | licm<no-allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | floatloop@724:Old + licm@724:Translated |  |  |
| -O1 | Simplify | loop-rotate<header-duplication;no-prepare-for-lto;no-check-exit-count> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | rotate@728:Translated |  |  |
| -O1 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | licm@730:Translated |  |  |
| -O1 | Simplify | simple-loop-unswitch<no-nontrivial;trivial> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | trivialunswitch@732:Translated |  |  |
| -O1 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@766:Partial |  |  |
| -O1 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@768:Partial + dead@768:Old |  |  |
| -O1 | Simplify | loop-idiom | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | loopidiom@737:Translated + loopidioms@737:Old |  |  |
| -O1 | Simplify | indvars | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | indvars@738:Partial |  |  |
| -O1 | Simplify | loop-deletion | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | loopdeletion@749:Translated |  |  |
| -O1 | Simplify | loop-unroll-full | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | unroll@753:Translated + indvars@753:Translated |  |  |
| -O1 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | sroa@776:Translated |  |  |
| -O1 | Simplify | memcpyopt | cgscc/devirt<4>/function<eager-inv;no-rerun> | ORDER | memcpyopt@818:Translated + merge@818:Partial | -O1 only: ours places it where the O2 list does; the pin's O1 list orders it differently (the one slot off the pin's order, by position) | unassigned (coordinator, 10-26) |
| -O1 | Simplify | sccp | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | sccp@792:Translated |  |  |
| -O1 | Simplify | bdce | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | bdce@797:Translated |  |  |
| -O1 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@801:Partial + fold@801:Old + dead@801:Old |  |  |
| -O1 | Simplify | coro-elide | cgscc/devirt<4>/function<eager-inv;no-rerun> | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Simplify | adce | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | adce@815:Translated |  |  |
| -O1 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@832:Partial |  |  |
| -O1 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@837:Partial + fold@837:Old + dead@837:Old |  |  |
| -O1 | Simplify |  |  | DEVIATE | 669: cvp@669 | -O1 only: the pin's O1 list is buildO1FunctionSimplificationPipeline, a shorter one; ours runs the O2 order minus ABSENT_AT_O1 (unverified slot by slot) |  |
| -O1 | Simplify |  |  | DEVIATE | 764: loopsimplify@764 + lcssa@764 | llrm's loop-simplify/lcssa stand-ins that LLVM's loop pass adaptor makes implicit (parity.rs RUN_COUNTS_OPEN) |  |
| -O1 | Simplify |  |  | DEVIATE | 772: loopsimplify@772 + lcssa@772 | as 764 |  |
| -O1 | Simplify |  |  | DEVIATE | 787: inferspace@787 | -O1 only: the pin's O1 list is buildO1FunctionSimplificationPipeline, a shorter one; ours runs the O2 order minus ABSENT_AT_O1 (unverified slot by slot) |  |
| -O1 | Simplify |  |  | DEVIATE | 810: cvp@810 | -O1 only: the pin's O1 list is buildO1FunctionSimplificationPipeline, a shorter one; ours runs the O2 order minus ABSENT_AT_O1 (unverified slot by slot) |  |
| -O1 | Simplify |  |  | DEVIATE | 820: dse@820 | -O1 only: the pin's O1 list is buildO1FunctionSimplificationPipeline, a shorter one; ours runs the O2 order minus ABSENT_AT_O1 (unverified slot by slot) |  |
| -O1 | Simplify |  |  | DEVIATE | 823: loopsimplify@823 + lcssa@823 + licm@823 | -O1 only, as 669 (unverified slot by slot) |  |
| -O1 | Late | drop-unnecessary-assumes | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O1 | Late | float2int | function<eager-inv> | MATCH | float2int@1594:Translated |  |  |
| -O1 | Late | lower-constant-intrinsics | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O1 | Late | loop-rotate<header-duplication;no-prepare-for-lto;check-exit-count> | function<eager-inv>/loop | MISSING |  | the late loop nest's rotate; ablation 0 of 12 IRs (the early one: +51 instructions, matched) | w-3 (w-5 if silent at 09:00), 11-09 |
| -O1 | Late | loop-deletion | function<eager-inv>/loop | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O1 | Late | loop-interchange | function<eager-inv>/loop | MATCH | loop-interchange@1631:Translated |  |  |
| -O1 | Late | loop-distribute | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O1 | Late | inject-tli-mappings | function<eager-inv> | N/A |  | maps scalar libcalls to vector ones; vectors deferred Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -O1 | Late | loop-vectorize<no-interleave-forced-only;vectorize-forced-only;> | function<eager-inv> | N/A |  | vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -O1 | Late | drop-unnecessary-assumes<drop-deref> | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O1 | Late | infer-alignment | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O1 | Late | loop-load-elim | function<eager-inv> | MATCH | loop-load-elim@1400:Partial |  |  |
| -O1 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MISSING |  | the three late runs; ours has no combine slot in the late list | w-4, 11-09 |
| -O1 | Late | simplifycfg<bonus-inst-threshold=1;forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;switch-to-lookup;no-keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | MATCH | simplifycfg@1438:Partial |  |  |
| -O1 | Late | vector-combine | function<eager-inv> | N/A |  | vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -O1 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MISSING |  | the three late runs; ours has no combine slot in the late list | w-4, 11-09 |
| -O1 | Late | loop-unroll<O1> | function<eager-inv> | MISSING |  | the late loop-unroll<O2>; ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O1 | Late | transform-warning | function<eager-inv> | N/A |  | diagnostic for failed pragmas; no output Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Late | sroa<preserve-cfg;aggregate-to-vector> | function<eager-inv> | MISSING |  | the late sroa<preserve-cfg;aggregate-to-vector>; ablation 0 of 12 IRs | w-4, 11-09 |
| -O1 | Late | infer-alignment | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O1 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MISSING |  | the three late runs; ours has no combine slot in the late list | w-4, 11-09 |
| -O1 | Late | licm<allowspeculation> | function<eager-inv>/loop-mssa | MATCH | licm@1508:Translated |  |  |
| -O1 | Late | alignment-from-assumptions | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O1 | Late | loop-sink | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O1 | Late | instsimplify | function<eager-inv> | MATCH | instsimplify@1662:Translated |  |  |
| -O1 | Late | div-rem-pairs | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -O1 | Late | mergeicmps | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -O1 | Late | expand-memcmp | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -O1 | Late | tailcallelim | function<eager-inv> | MATCH | tailrec@1677:Translated |  |  |
| -O1 | Late | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;speculate-unpredictables> | function<eager-inv> | MATCH | simplifycfg@1682:Partial |  |  |
| -O1 | Late | annotation-remarks | function | N/A |  | remarks off Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Module | memprof-remove-attributes |  | N/A |  | memprof: no profile Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Module | annotation2metadata |  | N/A |  | no source annotations Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Module | forceattrs |  | N/A |  | command-line forced attributes: none given Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Module | inferattrs |  | EXECUTED |  | stamp (interprocedural::Stamp): states what a body does, llrm's own form |  |
| -O1 | Module | coro-early |  | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Module | openmp-opt |  | N/A |  | OpenMP: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Module | ipsccp |  | DEVIATE |  | DEVIATE (ipsccp.rs): ipo-early (walk::ModuleIpo: run_ipsccp); no FunctionSpecializer, ipa-cp clones stand for it (ipsccp.rs DEVIATE 1) |  |
| -O1 | Module | called-value-propagation |  | MISSING |  | indirect calls only; not measured | w-6, 11-09 |
| -O1 | Module | globalopt |  | EXECUTED |  | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -O1 | Module | always-inline |  | EXECUTED |  | walk (the CGSCC walk's inliner) |  |
| -O1 | Module | inline | cgscc/devirt<4> | EXECUTED |  | walk, and interprocedural (inlines again after the clones) |  |
| -O1 | Module | function-attrs<skip-non-recursive-function-attrs> | cgscc/devirt<4> | EXECUTED |  | interprocedural (states what each body does); not compared pass by pass |  |
| -O1 | Module | function-attrs | cgscc/devirt<4> | EXECUTED |  | interprocedural (states what each body does); not compared pass by pass |  |
| -O1 | Module | coro-split | cgscc/devirt<4> | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Module | coro-annotation-elide | cgscc/devirt<4> | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Module | deadargelim |  | DEVIATE |  | DEVIATE W1 (walk.rs): ipo-early (walk::ModuleIpo: ipa-deadargs) runs before the Simplify list, the pin's after the CGSCC block |  |
| -O1 | Module | coro-cleanup |  | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Module | globalopt |  | EXECUTED |  | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -O1 | Module | globaldce |  | EXECUTED |  | globaldce (once). The pin's second globaldce, after the Late list, has no stage of ours: 0 B on all 65 QCport modules at -Os m16 (pin ablation, 2026-10-23), so N/A until a row moves (owner w-6) |  |
| -O1 | Module | elim-avail-extern |  | EXECUTED |  | available-externally |  |
| -O1 | Module | rpo-function-attrs |  | MISSING |  | no stage of ours is known to do it; unverified | w-6, 11-09 |
| -O1 | Module | recompute-globalsaa |  | N/A |  | an analysis recompute; ours holds and freezes GlobalsAA (pipeline.rs) Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Module | alloc-token |  | N/A |  | clang option, off Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Module | globaldce |  | EXECUTED |  | globaldce (once). The pin's second globaldce, after the Late list, has no stage of ours: 0 B on all 65 QCport modules at -Os m16 (pin ablation, 2026-10-23), so N/A until a row moves (owner w-6) |  |
| -O1 | Module | constmerge |  | MISSING |  | merging identical constants: -Os m16 on 65 QCport modules through the pin (llc obj sizes, constmerge removed): +58 B of 399336 (0.015%), 2 modules (game/mdl_ai +44, ui/sbar +14); a row of low weight | w-6, 11-09 |
| -O1 | Module | cg-profile |  | N/A |  | profile data Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Module | rel-lookup-table-converter |  | N/A |  | relative lookup tables are a PIC feature; no PIC targets Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O1 | Module | lower-comment-string |  | N/A |  | #pragma comment strings Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Early | ee-instrument<> | function<eager-inv> | N/A |  | -pg/-finstrument-functions: no frontend asks for it Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Early | lower-expect | function<eager-inv> | MATCH | lowerexpect@1177:Translated |  |  |
| -O2 | Early | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;no-switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | MATCH | decide@1178:Partial |  |  |
| -O2 | Early | sroa<modify-cfg> | function<eager-inv> | MATCH | sroa@1179:Translated |  |  |
| -O2 | Early | early-cse<> | function<eager-inv> | MATCH | earlycse@1180:Translated |  |  |
| -O2 | Cleanup | mem2reg | function<eager-inv> | MATCH | promote@1241:Translated |  |  |
| -O2 | Cleanup | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MATCH | combine@1242:Partial + dead@1242:Old |  |  |
| -O2 | Cleanup | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | MATCH | decide@1244:Partial |  |  |
| -O2 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | sroa@646:Translated |  |  |
| -O2 | Simplify | early-cse<memssa> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | earlycse@649:Translated |  |  |
| -O2 | Simplify | speculative-execution<only-if-divergent-target> | cgscc/devirt<4>/function<eager-inv;no-rerun> | N/A |  | runs only on a target with divergent branches; x86 has none Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | jumpthreading@668:Translated |  |  |
| -O2 | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | cvp@669:Translated |  |  |
| -O2 | Simplify | jump-table-to-switch | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | jumptableswitch@672:Translated |  |  |
| -O2 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@675:Partial |  |  |
| -O2 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@677:Partial + dead@677:Old |  |  |
| -O2 | Simplify | aggressive-instcombine | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | aggressive-instcombine@678:Partial |  |  |
| -O2 | Simplify | libcalls-shrinkwrap | cgscc/devirt<4>/function<eager-inv;no-rerun> | MISSING |  | errno-setting libm calls; ablation 0 of 12 IRs | funcattrs-port, 11-09 |
| -O2 | Simplify | tailcallelim | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | tailrec@688:Translated |  |  |
| -O2 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@690:Partial |  |  |
| -O2 | Simplify | reassociate | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | reassociate@696:Translated |  |  |
| -O2 | Simplify | constraint-elimination | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | constraint-elimination@699:Translated |  |  |
| -O2 | Simplify | loop-instsimplify | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | loop-instsimplify@715:Translated |  |  |
| -O2 | Simplify | loop-simplifycfg | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | loop-simplifycfg@716:Translated |  |  |
| -O2 | Simplify | licm<no-allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | floatloop@724:Old + licm@724:Translated |  |  |
| -O2 | Simplify | loop-rotate<header-duplication;no-prepare-for-lto;no-check-exit-count> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | rotate@728:Translated |  |  |
| -O2 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | licm@730:Translated |  |  |
| -O2 | Simplify | simple-loop-unswitch<no-nontrivial;trivial> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | trivialunswitch@732:Translated |  |  |
| -O2 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@766:Partial |  |  |
| -O2 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@768:Partial + dead@768:Old |  |  |
| -O2 | Simplify | loop-idiom | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | loopidiom@737:Translated + loopidioms@737:Old |  |  |
| -O2 | Simplify | indvars | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | indvars@738:Partial |  |  |
| -O2 | Simplify | extra-simple-loop-unswitch-passes | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O2 | Simplify | loop-deletion | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | loopdeletion@749:Translated |  |  |
| -O2 | Simplify | loop-unroll-full | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | unroll@753:Translated + indvars@753:Translated |  |  |
| -O2 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | sroa@776:Translated |  |  |
| -O2 | Simplify | vector-combine | cgscc/devirt<4>/function<eager-inv;no-rerun> | N/A |  | vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -O2 | Simplify | mldst-motion<no-split-footer-bb> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -O2 | Simplify | gvn<> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | inferspace@787:Old + gvn@787:Translated |  |  |
| -O2 | Simplify | sccp | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | sccp@792:Translated |  |  |
| -O2 | Simplify | bdce | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | bdce@797:Translated |  |  |
| -O2 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@801:Partial + fold@801:Old + dead@801:Old |  |  |
| -O2 | Simplify | dfa-jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | dfajumpthreading@806:Translated |  |  |
| -O2 | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | jumpthreading@809:Translated |  |  |
| -O2 | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | cvp@810:Translated |  |  |
| -O2 | Simplify | adce | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | adce@815:Translated |  |  |
| -O2 | Simplify | memcpyopt | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | memcpyopt@818:Translated + merge@818:Partial |  |  |
| -O2 | Simplify | dse | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | dse@820:Partial |  |  |
| -O2 | Simplify | move-auto-init | cgscc/devirt<4>/function<eager-inv;no-rerun> | N/A |  | needs -ftrivial-auto-var-init metadata; no frontend emits it Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | loopsimplify@823:Old + lcssa@823:Old + licm@823:Translated |  |  |
| -O2 | Simplify | coro-elide | cgscc/devirt<4>/function<eager-inv;no-rerun> | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@832:Partial |  |  |
| -O2 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@837:Partial + fold@837:Old + dead@837:Old |  |  |
| -O2 | Simplify |  |  | DEVIATE | 764: loopsimplify@764 + lcssa@764 | llrm's loop-simplify/lcssa stand-ins that LLVM's loop pass adaptor makes implicit (parity.rs RUN_COUNTS_OPEN) |  |
| -O2 | Simplify |  |  | DEVIATE | 772: loopsimplify@772 + lcssa@772 | as 764 |  |
| -O2 | Late | drop-unnecessary-assumes | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O2 | Late | float2int | function<eager-inv> | MATCH | float2int@1594:Translated |  |  |
| -O2 | Late | lower-constant-intrinsics | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O2 | Late | loop-rotate<header-duplication;no-prepare-for-lto;check-exit-count> | function<eager-inv>/loop | MISSING |  | the late loop nest's rotate; ablation 0 of 12 IRs (the early one: +51 instructions, matched) | w-3 (w-5 if silent at 09:00), 11-09 |
| -O2 | Late | loop-deletion | function<eager-inv>/loop | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O2 | Late | loop-interchange | function<eager-inv>/loop | MATCH | loop-interchange@1631:Translated |  |  |
| -O2 | Late | loop-distribute | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O2 | Late | inject-tli-mappings | function<eager-inv> | N/A |  | maps scalar libcalls to vector ones; vectors deferred Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -O2 | Late | loop-vectorize<no-interleave-forced-only;no-vectorize-forced-only;> | function<eager-inv> | N/A |  | vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -O2 | Late | drop-unnecessary-assumes<drop-deref> | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O2 | Late | infer-alignment | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O2 | Late | loop-load-elim | function<eager-inv> | MATCH | loop-load-elim@1400:Partial |  |  |
| -O2 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MISSING |  | the three late runs; ours has no combine slot in the late list | w-4, 11-09 |
| -O2 | Late | simplifycfg<bonus-inst-threshold=1;forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;switch-to-lookup;no-keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | MATCH | simplifycfg@1438:Partial |  |  |
| -O2 | Late | slp-vectorizer | function<eager-inv> | MATCH | slp@1455:Partial |  | arm64 only: slpcompares.rs, the wide compare the pin leaves of an and-chain of byte equalities; no vectors (target has vector registers: arm64; i486 N/A, user ruling 10-20) |
| -O2 | Late | vector-combine | function<eager-inv> | N/A |  | vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -O2 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MISSING |  | the three late runs; ours has no combine slot in the late list | w-4, 11-09 |
| -O2 | Late | loop-unroll<O2> | function<eager-inv> | MISSING |  | the late loop-unroll<O2>; ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O2 | Late | transform-warning | function<eager-inv> | N/A |  | diagnostic for failed pragmas; no output Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Late | sroa<preserve-cfg;aggregate-to-vector> | function<eager-inv> | MISSING |  | the late sroa<preserve-cfg;aggregate-to-vector>; ablation 0 of 12 IRs | w-4, 11-09 |
| -O2 | Late | infer-alignment | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O2 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MISSING |  | the three late runs; ours has no combine slot in the late list | w-4, 11-09 |
| -O2 | Late | licm<allowspeculation> | function<eager-inv>/loop-mssa | MATCH | licm@1508:Translated |  |  |
| -O2 | Late | alignment-from-assumptions | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O2 | Late | loop-sink | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O2 | Late | instsimplify | function<eager-inv> | MATCH | instsimplify@1662:Translated |  |  |
| -O2 | Late | div-rem-pairs | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -O2 | Late | mergeicmps | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -O2 | Late | expand-memcmp | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -O2 | Late | tailcallelim | function<eager-inv> | MATCH | tailrec@1677:Translated |  |  |
| -O2 | Late | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;speculate-unpredictables> | function<eager-inv> | MATCH | simplifycfg@1682:Partial |  |  |
| -O2 | Late | annotation-remarks | function | N/A |  | remarks off Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | memprof-remove-attributes |  | N/A |  | memprof: no profile Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | annotation2metadata |  | N/A |  | no source annotations Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | forceattrs |  | N/A |  | command-line forced attributes: none given Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | inferattrs |  | EXECUTED |  | stamp (interprocedural::Stamp): states what a body does, llrm's own form |  |
| -O2 | Module | coro-early |  | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | openmp-opt |  | N/A |  | OpenMP: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | ipsccp |  | DEVIATE |  | DEVIATE (ipsccp.rs): ipo-early (walk::ModuleIpo: run_ipsccp); no FunctionSpecializer, ipa-cp clones stand for it (ipsccp.rs DEVIATE 1) |  |
| -O2 | Module | called-value-propagation |  | MISSING |  | indirect calls only; not measured | w-6, 11-09 |
| -O2 | Module | globalopt |  | EXECUTED |  | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -O2 | Module | always-inline |  | EXECUTED |  | walk (the CGSCC walk's inliner) |  |
| -O2 | Module | inline | cgscc/devirt<4> | EXECUTED |  | walk, and interprocedural (inlines again after the clones) |  |
| -O2 | Module | function-attrs<skip-non-recursive-function-attrs> | cgscc/devirt<4> | EXECUTED |  | interprocedural (states what each body does); not compared pass by pass |  |
| -O2 | Module | openmp-opt-cgscc | cgscc/devirt<4> | N/A |  | OpenMP: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | function-attrs | cgscc/devirt<4> | EXECUTED |  | interprocedural (states what each body does); not compared pass by pass |  |
| -O2 | Module | coro-split | cgscc/devirt<4> | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | coro-annotation-elide | cgscc/devirt<4> | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | deadargelim |  | DEVIATE |  | DEVIATE W1 (walk.rs): ipo-early (walk::ModuleIpo: ipa-deadargs) runs before the Simplify list, the pin's after the CGSCC block |  |
| -O2 | Module | coro-cleanup |  | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | globalopt |  | EXECUTED |  | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -O2 | Module | globaldce |  | EXECUTED |  | globaldce (once). The pin's second globaldce, after the Late list, has no stage of ours: 0 B on all 65 QCport modules at -Os m16 (pin ablation, 2026-10-23), so N/A until a row moves (owner w-6) |  |
| -O2 | Module | elim-avail-extern |  | EXECUTED |  | available-externally |  |
| -O2 | Module | rpo-function-attrs |  | MISSING |  | no stage of ours is known to do it; unverified | w-6, 11-09 |
| -O2 | Module | recompute-globalsaa |  | N/A |  | an analysis recompute; ours holds and freezes GlobalsAA (pipeline.rs) Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | alloc-token |  | N/A |  | clang option, off Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | globaldce |  | EXECUTED |  | globaldce (once). The pin's second globaldce, after the Late list, has no stage of ours: 0 B on all 65 QCport modules at -Os m16 (pin ablation, 2026-10-23), so N/A until a row moves (owner w-6) |  |
| -O2 | Module | constmerge |  | MISSING |  | merging identical constants: -Os m16 on 65 QCport modules through the pin (llc obj sizes, constmerge removed): +58 B of 399336 (0.015%), 2 modules (game/mdl_ai +44, ui/sbar +14); a row of low weight | w-6, 11-09 |
| -O2 | Module | cg-profile |  | N/A |  | profile data Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | rel-lookup-table-converter |  | N/A |  | relative lookup tables are a PIC feature; no PIC targets Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O2 | Module | lower-comment-string |  | N/A |  | #pragma comment strings Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Early | ee-instrument<> | function<eager-inv> | N/A |  | -pg/-finstrument-functions: no frontend asks for it Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Early | lower-expect | function<eager-inv> | MATCH | lowerexpect@1177:Translated |  |  |
| -O3 | Early | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;no-switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | MATCH | decide@1178:Partial |  |  |
| -O3 | Early | sroa<modify-cfg> | function<eager-inv> | MATCH | sroa@1179:Translated |  |  |
| -O3 | Early | early-cse<> | function<eager-inv> | MATCH | earlycse@1180:Translated |  |  |
| -O3 | Early | callsite-splitting | function<eager-inv> | MISSING |  | -O3 only; not measured | funcattrs-port, 11-09 |
| -O3 | Cleanup | mem2reg | function<eager-inv> | MATCH | promote@1241:Translated |  |  |
| -O3 | Cleanup | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MATCH | combine@1242:Partial + dead@1242:Old |  |  |
| -O3 | Cleanup | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | MATCH | decide@1244:Partial |  |  |
| -O3 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | sroa@646:Translated |  |  |
| -O3 | Simplify | early-cse<memssa> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | earlycse@649:Translated |  |  |
| -O3 | Simplify | speculative-execution<only-if-divergent-target> | cgscc/devirt<4>/function<eager-inv;no-rerun> | N/A |  | runs only on a target with divergent branches; x86 has none Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | jumpthreading@668:Translated |  |  |
| -O3 | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | cvp@669:Translated |  |  |
| -O3 | Simplify | jump-table-to-switch | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | jumptableswitch@672:Translated |  |  |
| -O3 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@675:Partial |  |  |
| -O3 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@677:Partial + dead@677:Old |  |  |
| -O3 | Simplify | aggressive-instcombine | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | aggressive-instcombine@678:Partial |  |  |
| -O3 | Simplify | libcalls-shrinkwrap | cgscc/devirt<4>/function<eager-inv;no-rerun> | MISSING |  | errno-setting libm calls; ablation 0 of 12 IRs | funcattrs-port, 11-09 |
| -O3 | Simplify | tailcallelim | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | tailrec@688:Translated |  |  |
| -O3 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@690:Partial |  |  |
| -O3 | Simplify | reassociate | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | reassociate@696:Translated |  |  |
| -O3 | Simplify | constraint-elimination | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | constraint-elimination@699:Translated |  |  |
| -O3 | Simplify | loop-instsimplify | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | loop-instsimplify@715:Translated |  |  |
| -O3 | Simplify | loop-simplifycfg | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | loop-simplifycfg@716:Translated |  |  |
| -O3 | Simplify | licm<no-allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | floatloop@724:Old + licm@724:Translated |  |  |
| -O3 | Simplify | loop-rotate<header-duplication;no-prepare-for-lto;no-check-exit-count> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | rotate@728:Translated |  |  |
| -O3 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | licm@730:Translated |  |  |
| -O3 | Simplify | simple-loop-unswitch<nontrivial;trivial> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | trivialunswitch@732:Translated |  |  |
| -O3 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@766:Partial |  |  |
| -O3 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@768:Partial + dead@768:Old |  |  |
| -O3 | Simplify | loop-idiom | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | loopidiom@737:Translated + loopidioms@737:Old |  |  |
| -O3 | Simplify | indvars | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | indvars@738:Partial |  |  |
| -O3 | Simplify | extra-simple-loop-unswitch-passes | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O3 | Simplify | loop-deletion | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | loopdeletion@749:Translated |  |  |
| -O3 | Simplify | loop-unroll-full | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | unroll@753:Translated + indvars@753:Translated |  |  |
| -O3 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | sroa@776:Translated |  |  |
| -O3 | Simplify | vector-combine | cgscc/devirt<4>/function<eager-inv;no-rerun> | N/A |  | vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -O3 | Simplify | mldst-motion<no-split-footer-bb> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -O3 | Simplify | gvn<> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | inferspace@787:Old + gvn@787:Translated |  |  |
| -O3 | Simplify | sccp | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | sccp@792:Translated |  |  |
| -O3 | Simplify | bdce | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | bdce@797:Translated |  |  |
| -O3 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@801:Partial + fold@801:Old + dead@801:Old |  |  |
| -O3 | Simplify | dfa-jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | dfajumpthreading@806:Translated |  |  |
| -O3 | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | jumpthreading@809:Translated |  |  |
| -O3 | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | cvp@810:Translated |  |  |
| -O3 | Simplify | adce | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | adce@815:Translated |  |  |
| -O3 | Simplify | memcpyopt | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | memcpyopt@818:Translated + merge@818:Partial |  |  |
| -O3 | Simplify | dse | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | dse@820:Partial |  |  |
| -O3 | Simplify | move-auto-init | cgscc/devirt<4>/function<eager-inv;no-rerun> | N/A |  | needs -ftrivial-auto-var-init metadata; no frontend emits it Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | loopsimplify@823:Old + lcssa@823:Old + licm@823:Translated |  |  |
| -O3 | Simplify | coro-elide | cgscc/devirt<4>/function<eager-inv;no-rerun> | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@832:Partial |  |  |
| -O3 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@837:Partial + fold@837:Old + dead@837:Old |  |  |
| -O3 | Simplify |  |  | DEVIATE | 764: loopsimplify@764 + lcssa@764 | llrm's loop-simplify/lcssa stand-ins that LLVM's loop pass adaptor makes implicit (parity.rs RUN_COUNTS_OPEN) |  |
| -O3 | Simplify |  |  | DEVIATE | 772: loopsimplify@772 + lcssa@772 | as 764 |  |
| -O3 | Late | drop-unnecessary-assumes | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O3 | Late | float2int | function<eager-inv> | MATCH | float2int@1594:Translated |  |  |
| -O3 | Late | lower-constant-intrinsics | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O3 | Late | chr | function<eager-inv> | N/A |  | control-height-reduction needs profile data Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Late | loop-rotate<header-duplication;no-prepare-for-lto;check-exit-count> | function<eager-inv>/loop | MISSING |  | the late loop nest's rotate; ablation 0 of 12 IRs (the early one: +51 instructions, matched) | w-3 (w-5 if silent at 09:00), 11-09 |
| -O3 | Late | loop-deletion | function<eager-inv>/loop | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O3 | Late | loop-interchange | function<eager-inv>/loop | MATCH | loop-interchange@1631:Translated |  |  |
| -O3 | Late | loop-distribute | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O3 | Late | inject-tli-mappings | function<eager-inv> | N/A |  | maps scalar libcalls to vector ones; vectors deferred Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -O3 | Late | loop-vectorize<no-interleave-forced-only;no-vectorize-forced-only;> | function<eager-inv> | N/A |  | vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -O3 | Late | drop-unnecessary-assumes<drop-deref> | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O3 | Late | infer-alignment | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O3 | Late | loop-load-elim | function<eager-inv> | MATCH | loop-load-elim@1400:Partial |  |  |
| -O3 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MISSING |  | the three late runs; ours has no combine slot in the late list | w-4, 11-09 |
| -O3 | Late | simplifycfg<bonus-inst-threshold=1;forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;switch-to-lookup;no-keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | MATCH | simplifycfg@1438:Partial |  |  |
| -O3 | Late | slp-vectorizer | function<eager-inv> | MATCH | slp@1455:Partial |  | arm64 only: slpcompares.rs, the wide compare the pin leaves of an and-chain of byte equalities; no vectors (target has vector registers: arm64; i486 N/A, user ruling 10-20) |
| -O3 | Late | vector-combine | function<eager-inv> | N/A |  | vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -O3 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MISSING |  | the three late runs; ours has no combine slot in the late list | w-4, 11-09 |
| -O3 | Late | loop-unroll<O3> | function<eager-inv> | MISSING |  | the late loop-unroll<O2>; ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O3 | Late | transform-warning | function<eager-inv> | N/A |  | diagnostic for failed pragmas; no output Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Late | sroa<preserve-cfg;aggregate-to-vector> | function<eager-inv> | MISSING |  | the late sroa<preserve-cfg;aggregate-to-vector>; ablation 0 of 12 IRs | w-4, 11-09 |
| -O3 | Late | infer-alignment | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O3 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MISSING |  | the three late runs; ours has no combine slot in the late list | w-4, 11-09 |
| -O3 | Late | licm<allowspeculation> | function<eager-inv>/loop-mssa | MATCH | licm@1508:Translated |  |  |
| -O3 | Late | alignment-from-assumptions | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -O3 | Late | loop-sink | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -O3 | Late | instsimplify | function<eager-inv> | MATCH | instsimplify@1662:Translated |  |  |
| -O3 | Late | div-rem-pairs | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -O3 | Late | mergeicmps | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -O3 | Late | expand-memcmp | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -O3 | Late | tailcallelim | function<eager-inv> | MATCH | tailrec@1677:Translated |  |  |
| -O3 | Late | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;speculate-unpredictables> | function<eager-inv> | MATCH | simplifycfg@1682:Partial |  |  |
| -O3 | Late | annotation-remarks | function | N/A |  | remarks off Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Late |  |  | DEVIATE | 1474: loop-unroll-and-jam@1474 | -O3: the pin's printed list has no unroll-and-jam in this fixture; unverified |  |
| -O3 | Module | memprof-remove-attributes |  | N/A |  | memprof: no profile Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Module | annotation2metadata |  | N/A |  | no source annotations Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Module | forceattrs |  | N/A |  | command-line forced attributes: none given Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Module | inferattrs |  | EXECUTED |  | stamp (interprocedural::Stamp): states what a body does, llrm's own form |  |
| -O3 | Module | coro-early |  | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Module | openmp-opt |  | N/A |  | OpenMP: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Module | ipsccp |  | DEVIATE |  | DEVIATE (ipsccp.rs): ipo-early (walk::ModuleIpo: run_ipsccp); no FunctionSpecializer, ipa-cp clones stand for it (ipsccp.rs DEVIATE 1) |  |
| -O3 | Module | called-value-propagation |  | MISSING |  | indirect calls only; not measured | w-6, 11-09 |
| -O3 | Module | globalopt |  | EXECUTED |  | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -O3 | Module | always-inline |  | EXECUTED |  | walk (the CGSCC walk's inliner) |  |
| -O3 | Module | inline | cgscc/devirt<4> | EXECUTED |  | walk, and interprocedural (inlines again after the clones) |  |
| -O3 | Module | function-attrs<skip-non-recursive-function-attrs> | cgscc/devirt<4> | EXECUTED |  | interprocedural (states what each body does); not compared pass by pass |  |
| -O3 | Module | argpromotion | cgscc/devirt<4> | MISSING |  | -O3 only; argpromotion.rs exists, where it runs is not in the registry: unverified | w-6, 11-09 |
| -O3 | Module | openmp-opt-cgscc | cgscc/devirt<4> | N/A |  | OpenMP: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Module | function-attrs | cgscc/devirt<4> | EXECUTED |  | interprocedural (states what each body does); not compared pass by pass |  |
| -O3 | Module | coro-split | cgscc/devirt<4> | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Module | coro-annotation-elide | cgscc/devirt<4> | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Module | deadargelim |  | DEVIATE |  | DEVIATE W1 (walk.rs): ipo-early (walk::ModuleIpo: ipa-deadargs) runs before the Simplify list, the pin's after the CGSCC block |  |
| -O3 | Module | coro-cleanup |  | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Module | globalopt |  | EXECUTED |  | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -O3 | Module | globaldce |  | EXECUTED |  | globaldce (once). The pin's second globaldce, after the Late list, has no stage of ours: 0 B on all 65 QCport modules at -Os m16 (pin ablation, 2026-10-23), so N/A until a row moves (owner w-6) |  |
| -O3 | Module | elim-avail-extern |  | EXECUTED |  | available-externally |  |
| -O3 | Module | rpo-function-attrs |  | MISSING |  | no stage of ours is known to do it; unverified | w-6, 11-09 |
| -O3 | Module | recompute-globalsaa |  | N/A |  | an analysis recompute; ours holds and freezes GlobalsAA (pipeline.rs) Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Module | alloc-token |  | N/A |  | clang option, off Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Module | globaldce |  | EXECUTED |  | globaldce (once). The pin's second globaldce, after the Late list, has no stage of ours: 0 B on all 65 QCport modules at -Os m16 (pin ablation, 2026-10-23), so N/A until a row moves (owner w-6) |  |
| -O3 | Module | constmerge |  | MISSING |  | merging identical constants: -Os m16 on 65 QCport modules through the pin (llc obj sizes, constmerge removed): +58 B of 399336 (0.015%), 2 modules (game/mdl_ai +44, ui/sbar +14); a row of low weight | w-6, 11-09 |
| -O3 | Module | cg-profile |  | N/A |  | profile data Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Module | rel-lookup-table-converter |  | N/A |  | relative lookup tables are a PIC feature; no PIC targets Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -O3 | Module | lower-comment-string |  | N/A |  | #pragma comment strings Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Early | ee-instrument<> | function<eager-inv> | N/A |  | -pg/-finstrument-functions: no frontend asks for it Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Early | lower-expect | function<eager-inv> | MATCH | lowerexpect@1177:Translated |  |  |
| -Os | Early | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;no-switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | MATCH | decide@1178:Partial |  |  |
| -Os | Early | sroa<modify-cfg> | function<eager-inv> | MATCH | sroa@1179:Translated |  |  |
| -Os | Early | early-cse<> | function<eager-inv> | MATCH | earlycse@1180:Translated |  |  |
| -Os | Cleanup | mem2reg | function<eager-inv> | MATCH | promote@1241:Translated |  |  |
| -Os | Cleanup | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MATCH | combine@1242:Partial + dead@1242:Old |  |  |
| -Os | Cleanup | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | MATCH | decide@1244:Partial |  |  |
| -Os | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | sroa@646:Translated |  |  |
| -Os | Simplify | early-cse<memssa> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | earlycse@649:Translated |  |  |
| -Os | Simplify | speculative-execution<only-if-divergent-target> | cgscc/devirt<4>/function<eager-inv;no-rerun> | N/A |  | runs only on a target with divergent branches; x86 has none Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | jumpthreading@668:Translated |  |  |
| -Os | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | cvp@669:Translated |  |  |
| -Os | Simplify | jump-table-to-switch | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | jumptableswitch@672:Translated |  |  |
| -Os | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@675:Partial |  |  |
| -Os | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@677:Partial + dead@677:Old |  |  |
| -Os | Simplify | aggressive-instcombine | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | aggressive-instcombine@678:Partial |  |  |
| -Os | Simplify | libcalls-shrinkwrap | cgscc/devirt<4>/function<eager-inv;no-rerun> | MISSING |  | errno-setting libm calls; ablation 0 of 12 IRs | funcattrs-port, 11-09 |
| -Os | Simplify | tailcallelim | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | tailrec@688:Translated |  |  |
| -Os | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@690:Partial |  |  |
| -Os | Simplify | reassociate | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | reassociate@696:Translated |  |  |
| -Os | Simplify | constraint-elimination | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | constraint-elimination@699:Translated |  |  |
| -Os | Simplify | loop-instsimplify | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | loop-instsimplify@715:Translated |  |  |
| -Os | Simplify | loop-simplifycfg | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | loop-simplifycfg@716:Translated |  |  |
| -Os | Simplify | licm<no-allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | floatloop@724:Old + licm@724:Translated |  |  |
| -Os | Simplify | loop-rotate<header-duplication;no-prepare-for-lto;no-check-exit-count> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | rotate@728:Translated |  |  |
| -Os | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | licm@730:Translated |  |  |
| -Os | Simplify | simple-loop-unswitch<no-nontrivial;trivial> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | trivialunswitch@732:Translated |  |  |
| -Os | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@766:Partial |  |  |
| -Os | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@768:Partial + dead@768:Old |  |  |
| -Os | Simplify | loop-idiom | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | loopidiom@737:Translated + loopidioms@737:Old |  |  |
| -Os | Simplify | indvars | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | indvars@738:Partial |  |  |
| -Os | Simplify | extra-simple-loop-unswitch-passes | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -Os | Simplify | loop-deletion | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | loopdeletion@749:Translated |  |  |
| -Os | Simplify | loop-unroll-full | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | MATCH | unroll@753:Translated + indvars@753:Translated |  |  |
| -Os | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | sroa@776:Translated |  |  |
| -Os | Simplify | vector-combine | cgscc/devirt<4>/function<eager-inv;no-rerun> | N/A |  | vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -Os | Simplify | mldst-motion<no-split-footer-bb> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -Os | Simplify | gvn<> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | inferspace@787:Old + gvn@787:Translated |  |  |
| -Os | Simplify | sccp | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | sccp@792:Translated |  |  |
| -Os | Simplify | bdce | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | bdce@797:Translated |  |  |
| -Os | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@801:Partial + fold@801:Old + dead@801:Old |  |  |
| -Os | Simplify | dfa-jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | dfajumpthreading@806:Translated |  |  |
| -Os | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | jumpthreading@809:Translated |  |  |
| -Os | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | cvp@810:Translated |  |  |
| -Os | Simplify | adce | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | adce@815:Translated |  |  |
| -Os | Simplify | memcpyopt | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | memcpyopt@818:Translated + merge@818:Partial |  |  |
| -Os | Simplify | dse | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | dse@820:Partial |  |  |
| -Os | Simplify | move-auto-init | cgscc/devirt<4>/function<eager-inv;no-rerun> | N/A |  | needs -ftrivial-auto-var-init metadata; no frontend emits it Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | MATCH | loopsimplify@823:Old + lcssa@823:Old + licm@823:Translated |  |  |
| -Os | Simplify | coro-elide | cgscc/devirt<4>/function<eager-inv;no-rerun> | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | decide@832:Partial |  |  |
| -Os | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | MATCH | combine@837:Partial + fold@837:Old + dead@837:Old |  |  |
| -Os | Simplify |  |  | DEVIATE | 764: loopsimplify@764 + lcssa@764 | llrm's loop-simplify/lcssa stand-ins that LLVM's loop pass adaptor makes implicit (parity.rs RUN_COUNTS_OPEN) |  |
| -Os | Simplify |  |  | DEVIATE | 772: loopsimplify@772 + lcssa@772 | as 764 |  |
| -Os | Late | drop-unnecessary-assumes | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -Os | Late | float2int | function<eager-inv> | MATCH | float2int@1594:Translated |  |  |
| -Os | Late | lower-constant-intrinsics | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -Os | Late | loop-rotate<header-duplication;no-prepare-for-lto;check-exit-count> | function<eager-inv>/loop | MISSING |  | the late loop nest's rotate; ablation 0 of 12 IRs (the early one: +51 instructions, matched) | w-3 (w-5 if silent at 09:00), 11-09 |
| -Os | Late | loop-deletion | function<eager-inv>/loop | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -Os | Late | loop-interchange | function<eager-inv>/loop | MATCH | loop-interchange@1631:Translated |  |  |
| -Os | Late | loop-distribute | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -Os | Late | inject-tli-mappings | function<eager-inv> | N/A |  | maps scalar libcalls to vector ones; vectors deferred Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -Os | Late | loop-vectorize<no-interleave-forced-only;no-vectorize-forced-only;> | function<eager-inv> | N/A |  | vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -Os | Late | drop-unnecessary-assumes<drop-deref> | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -Os | Late | infer-alignment | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -Os | Late | loop-load-elim | function<eager-inv> | MATCH | loop-load-elim@1400:Partial |  |  |
| -Os | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MISSING |  | the three late runs; ours has no combine slot in the late list | w-4, 11-09 |
| -Os | Late | simplifycfg<bonus-inst-threshold=1;forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;switch-to-lookup;no-keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | MATCH | simplifycfg@1438:Partial |  |  |
| -Os | Late | slp-vectorizer | function<eager-inv> | MATCH | slp@1455:Partial |  | arm64 only: slpcompares.rs, the wide compare the pin leaves of an and-chain of byte equalities; no vectors (target has vector registers: arm64; i486 N/A, user ruling 10-20) |
| -Os | Late | vector-combine | function<eager-inv> | N/A |  | vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out. |  |
| -Os | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MISSING |  | the three late runs; ours has no combine slot in the late list | w-4, 11-09 |
| -Os | Late | loop-unroll<O2> | function<eager-inv> | MISSING |  | the late loop-unroll<O2>; ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -Os | Late | transform-warning | function<eager-inv> | N/A |  | diagnostic for failed pragmas; no output Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Late | sroa<preserve-cfg;aggregate-to-vector> | function<eager-inv> | MISSING |  | the late sroa<preserve-cfg;aggregate-to-vector>; ablation 0 of 12 IRs | w-4, 11-09 |
| -Os | Late | infer-alignment | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -Os | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | MISSING |  | the three late runs; ours has no combine slot in the late list | w-4, 11-09 |
| -Os | Late | licm<allowspeculation> | function<eager-inv>/loop-mssa | MATCH | licm@1508:Translated |  |  |
| -Os | Late | alignment-from-assumptions | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-1, 11-09 |
| -Os | Late | loop-sink | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-3 (w-5 if silent at 09:00), 11-09 |
| -Os | Late | instsimplify | function<eager-inv> | MATCH | instsimplify@1662:Translated |  |  |
| -Os | Late | div-rem-pairs | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -Os | Late | mergeicmps | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -Os | Late | expand-memcmp | function<eager-inv> | MISSING |  | ablation 0 of 12 IRs | w-4, 11-09 |
| -Os | Late | tailcallelim | function<eager-inv> | MATCH | tailrec@1677:Translated |  |  |
| -Os | Late | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;speculate-unpredictables> | function<eager-inv> | MATCH | simplifycfg@1682:Partial |  |  |
| -Os | Late | annotation-remarks | function | N/A |  | remarks off Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | memprof-remove-attributes |  | N/A |  | memprof: no profile Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | annotation2metadata |  | N/A |  | no source annotations Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | forceattrs |  | N/A |  | command-line forced attributes: none given Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | inferattrs |  | EXECUTED |  | stamp (interprocedural::Stamp): states what a body does, llrm's own form |  |
| -Os | Module | coro-early |  | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | openmp-opt |  | N/A |  | OpenMP: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | ipsccp |  | DEVIATE |  | DEVIATE (ipsccp.rs): ipo-early (walk::ModuleIpo: run_ipsccp); no FunctionSpecializer, ipa-cp clones stand for it (ipsccp.rs DEVIATE 1) |  |
| -Os | Module | called-value-propagation |  | MISSING |  | indirect calls only; not measured | w-6, 11-09 |
| -Os | Module | globalopt |  | EXECUTED |  | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -Os | Module | always-inline |  | EXECUTED |  | walk (the CGSCC walk's inliner) |  |
| -Os | Module | inline | cgscc/devirt<4> | EXECUTED |  | walk, and interprocedural (inlines again after the clones) |  |
| -Os | Module | function-attrs<skip-non-recursive-function-attrs> | cgscc/devirt<4> | EXECUTED |  | interprocedural (states what each body does); not compared pass by pass |  |
| -Os | Module | openmp-opt-cgscc | cgscc/devirt<4> | N/A |  | OpenMP: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | function-attrs | cgscc/devirt<4> | EXECUTED |  | interprocedural (states what each body does); not compared pass by pass |  |
| -Os | Module | coro-split | cgscc/devirt<4> | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | coro-annotation-elide | cgscc/devirt<4> | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | deadargelim |  | DEVIATE |  | DEVIATE W1 (walk.rs): ipo-early (walk::ModuleIpo: ipa-deadargs) runs before the Simplify list, the pin's after the CGSCC block |  |
| -Os | Module | coro-cleanup |  | N/A |  | coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | globalopt |  | EXECUTED |  | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -Os | Module | globaldce |  | EXECUTED |  | globaldce (once). The pin's second globaldce, after the Late list, has no stage of ours: 0 B on all 65 QCport modules at -Os m16 (pin ablation, 2026-10-23), so N/A until a row moves (owner w-6) |  |
| -Os | Module | elim-avail-extern |  | EXECUTED |  | available-externally |  |
| -Os | Module | rpo-function-attrs |  | MISSING |  | no stage of ours is known to do it; unverified | w-6, 11-09 |
| -Os | Module | recompute-globalsaa |  | N/A |  | an analysis recompute; ours holds and freezes GlobalsAA (pipeline.rs) Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | alloc-token |  | N/A |  | clang option, off Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | globaldce |  | EXECUTED |  | globaldce (once). The pin's second globaldce, after the Late list, has no stage of ours: 0 B on all 65 QCport modules at -Os m16 (pin ablation, 2026-10-23), so N/A until a row moves (owner w-6) |  |
| -Os | Module | constmerge |  | MISSING |  | merging identical constants: -Os m16 on 65 QCport modules through the pin (llc obj sizes, constmerge removed): +58 B of 399336 (0.015%), 2 modules (game/mdl_ai +44, ui/sbar +14); a row of low weight | w-6, 11-09 |
| -Os | Module | cg-profile |  | N/A |  | profile data Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | rel-lookup-table-converter |  | N/A |  | relative lookup tables are a PIC feature; no PIC targets Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
| -Os | Module | lower-comment-string |  | N/A |  | #pragma comment strings Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang). |  |
