# FloatPrep into selection: plan

Owner: w-2 (moved from w12 on 2026-10-25). Pin d1106deb71cc81016406a1917d0508d2df296266.
Ours: `backend/floatassign.rs` (`prepared`, `FloatPrep`), `backend/fpstack.rs` (`converted`).
Numbers: d_faces -O2 m32, callgrind, matched pairs, 2026-10-25.

## Cost today

| Part | Ir | Note |
|---|---|---|
| FloatPrep (`prepared`) | 8.3 M -> 3.45 M | PR 4648: liveness of the floating values only (-4.92 M) |
| `fpstack::converted` | 10.0 M | liveness 1.65, `Ahead::order` 0.77, `_split_for_the_stack` 0.51, `bundles` 0.46, `fpregs::live_out` 0.44, `_split_edges` 0.33 |

Left in `prepared`: `live_among` 2.2 M, the rest scans.

## What FloatPrep computes

| Step | LLVM does it | Emitter can produce it |
|---|---|---|
| `_integer_loads`: `fild` of a register or immediate goes through a cell (constant pool for a constant) | isel (`X86ISelLowering` lowers `sint_to_fp` from a register through a stack slot) | yes: the lowering knows the source |
| `_integer_stores`: float-to-integer store lands in an owned cell, `wait` in BASIC | isel (`FP_TO_INT` through `FIST` to a slot) | yes |
| `_aliased_by`: a later load of an unwritten cell reuses the first load | MachineCSE / GVN before isel; no x87-specific step | no: already CSE's job (row 43); needs `live_out` only to keep a value that outlives its block |
| `_sequenced`: parallel-copy groups of x87 values written one at a time | PHI elimination | no: phielim's (`parcopy`) |
| `_compares_clobber`: float compare defines AX | `FNSTSW16r` defines AX in the pattern | yes: the emitter marks it when it emits the compare |

So three of five steps are selection's, and they need no liveness at all. The liveness in `prepared` serves `_aliased_by` (is the result live out of its block) and the entry check (no float live into the entry).

## Liveness `converted` needs

`live_in`/`live_out` of the floating values, bundles, and `_split_for_the_stack`'s edge splits. `prepared` solves the same rows one pass earlier. Between them only `_integer_loads/stores` and `_aliased` change the body, and they should add only values born and dying in one block (to check in PR 4).

## Order of PRs

1. Done (PR 4648): float-only liveness in `prepared`.
2. `_compares_clobber` in the emitter (`emit.rs` compare arm); delete the pass step. Fail-first: a compare body out of isel has AX in `clobbers`.
3. `_integer_loads` / `_integer_stores` in the emitter (the `fild`/`fist` arms, frame cell and pool at emit time). After 2 `prepared` is `_aliased_by` + `_sequenced`.
4. One float liveness for `prepared` and `converted`: `converted` takes the rows `prepared` made, updated for the values 3 added (single-block). Saves about 1.6 M.
5. `_aliased_by` waits for row 43 (CSE2) and then goes.

Each PR: dist pair on d_faces -O2, objsweep 568 objects 0 changed, fail-first test.

## Parked (2026-10-25)

- Steps 2 and 3: while the old isel path exists both would run, so they save 0 Ir. Revisit when it is deleted.
- Step 4 (one float liveness for `prepared` and `converted`): reuse needs a check that the float defs/uses and CFG are unchanged, which costs the numbering half of the solve; saving <= 0.9 M, and a stale row is a silent miscompile.
- Lazy block-exit liveness in `prepared`: measured -0.06 M (repeated loads are common); dropped.

## File split with w12

w12 edits the call-frame arms of `emit.rs`. This work touches the compare arm and the `fild`/`fist` arms (2, 3), and `floatassign.rs`/`fpstack.rs`. Disjoint hunks; whoever lands second merges `feat/parity`.
