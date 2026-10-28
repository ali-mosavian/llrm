# Module / IPO plan

Owner: cycles-queue-quiet-host (w-6). 2026-10-22. Rows by 2026-11-09.

## 1. The walk

Our `Walk` is already the pin's order: callees first, each body inlined into and then run through the Simplify list (pipeline.rs `Walk`, `rerun`). `_sift` is rotated and simplified before it is inlined (x_heapsort -O2 `walk/01`: jumpthreading@668, rotate@728, jumpthreading@809, loopsimplify@823). No walk rewrite.

What `_sift` lacks against the pin's (the pin's `-print-after-all` log of `hs`):
- `icmp sle (or disjoint x, 1), n` to `icmp slt x, n` (InstCombine `foldICmpBinOp`): PR `fix/or-disjoint-compare`, w-6.
- GVN load PRE across the critical edge (`.pre`) and the `smax` after it: w-5.

## 2. Inliner cost model (jpeg)

At feat/parity with #3422 (dist 48f383e9a30d, jpeg_single -O2 m32, `LLRM_INLINE_REMARKS` against the pin's `-pass-remarks=inline` on the same 09-pipeline.ll): `_next` 27 inlined in both (cost 90 ours, 85 pin, threshold 225), `_next16` 4/4, `_put32` 8/8, `_clamp` 3/3, `_receive` 2/2, `_extend` 2/2. They differ on `_message` (ours 9, pin 4) and `_restart` (pin 1, ours 0); `_getbit` into `_decode` is refused by ours (cost 270, threshold 225). The "4 of 31" does not reproduce here; re-measure on #3422's own base. Row: `_message`, `_restart`, `_getbit` against the pin's `InlineCost` (cost by item), w-6, 2026-10-30.

## 3. IPSCCP

The early `Sccp` slot (pipeline.rs, after `ModuleIpo`) is IPSCCP's per-body half; what flows between bodies is `ModuleIpo`'s (`ipa-args`, `ipa-range`). The pin runs IPSCCP once, before the walk; ours runs `ipa-range` again after the list (`Interprocedural`'s second Simplify run). Plan: `ModuleIpo` runs the solver over the program (funcattrs-port's IPO analyses), then the early slot and the second run go. Note with the dist Ir header: 2026-10-27.

## 4. In-SCC structure

The pin: `cgscc(devirt<4>(inline, function-attrs<skip-non-recursive>, function<no-rerun>(...), function-attrs, ...))`.

| row | ours | plan |
|---|---|---|
| devirt<4> repeat | no repeat: `Walk` runs once per body, callees first | compare only: count the bodies where the pin's repeat fires on the rows corpus; N/A if none |
| function-attrs inside the walk | `deriveAttrsInPostOrder` after the walk (interprocedural.rs); the post-order helper is used at walk.rs:389 | which inline decisions read attributes a callee gets only after; 2026-11-02 |
| deadargelim position | before the Simplify list (W1); the pin's after the walk | exit W1 as stated in walk.rs; with section 3 |
| rpo-function-attrs | no stage found | ablate on the rows; N/A with evidence or a row, 2026-11-05 |
| second globaldce (after Late) | one `GlobalDce` after interprocedural | -Os bytes by qcport.py first, 2026-11-02 |
| constmerge | none | -Os data bytes by qcport.py, 2026-11-02 |
| called-value-propagation | none | indirect calls it resolves before the walk: count on the rows, 2026-11-05 |
| argpromotion (-O3) | `argpromotion.rs`, runs as `promote.` in `Interprocedural` (interprocedural.rs:668), after the walk; the pin runs it inside the CGSCC pass | order only; row 2026-11-09 |

## 5. D10 and #3454 under the pin's walk

Neither is an order artifact. LLVM refuses to inline a recursive callee into itself, so the pin has no self-copies at any order; D10 is gcc's `recursive_inlining` as a numbered DEVIATE (ruled 2026-10-22, review 2026-11-30) and its second Simplify run is its cleanup. #3454 (self-copies honour `-fno-inline-functions`, the pin's OnlyAlwaysInlining) is a flag fact independent of the walk. Both survive a walk that matches the pin's caps. D10's chains are re-measured after #3422 and #3497 (hanoi, recsum, rectwo).

## 6. Rows added 2026-10-23

| id | row | evidence | owner | plan note | first code PR |
|---|---|---|---|---|---|
| IPO-FP-1 | A call through a constant table of function pointers (`x_funcptr`: `table[(i*7 + (x&3)) & 3](x)`) is a switch of direct calls the inliner then takes. Cause retracted from CalledValuePropagation and `tryPromoteCall` (that one reads vtable loads from an alloca only): it is `JumpTableToSwitch` ([JumpTableToSwitch.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpTableToSwitch.cpp), added at [PassBuilderPipelines.cpp#L672](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L672) after CorrelatedValuePropagation); ours has no such pass and keeps four indirect calls | pin clang -O2 i486 on x_funcptr.c: 3 `switch`, no indirect call; the same with `-mllvm -enable-jump-table-to-switch=false`: 2 indirect calls, no switch. The slot table's "ablation 0 of 12 IRs" for the pass is the wrong instrument: its corpus has no function-pointer table | w-6 asks w-4 (slot table row owner, 11-09) whether to take it | 2026-11-02 (this row) | 2026-11-06 |
| IPO-INL-1 | Recursive callees we inline (D10) that the pin never does: 20 of the 24 differing program/levels; accounted for by D10 | same comparison | w-6 | D10's numbered DEVIATE (docs/architecture/recursion-rows.md) | none |
| IPO-INL-2 | x_strlen -Os: `bench_x_strlen` into `main` is not inlined in either, but the verdicts rest on different numbers: ours cost 125 / threshold 125, the pin's cost 110 / threshold 50 (`-Rpass-missed=inline`); the other sites (slen, scopy; O1-O3) agree in verdict | `LLRM_INLINE_REMARKS=1` on the dist of #3837's head and the pin's clang `-Rpass=inline -Rpass-missed=inline`, m32 i486, same file | w-6; walk side funcattrs-port | 2026-11-13 (find which bonus lifts our -Os threshold at a call in a function executed once) | |
| IPO-INL-3 | x_funcptr: the four callees of the switch JumpTableToSwitch makes are not inlined into its arms; the pin inlines them (cost -30, threshold 337) | dumps of the dist at the tip: the switch first appears in `10-walk.ll` and `call _f0.._f3` stay; the pin makes the switch in the function pipeline and its CGSCC `devirt<4>` inlines the new direct calls. Hypothesis, not yet confirmed: `walk` decides inlining once per call graph and does not revisit calls the body pipeline creates | w-6; walk side funcattrs-port | 2026-11-13 | |
