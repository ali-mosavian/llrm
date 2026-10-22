# Fold: what it costs, what it carries, how it goes (llrm-only bucket, d_faces -O2)

Measured with callgrind --separate-callers=30 on d_faces -O2, dist f782e4c567e6 (parity 7c4d1afdb), 2026-10-21 (a shared-dir build: re-take before any number here is quoted). Pin side: clang -m32 -march=i486 (pin side), our `llrm-c -m32 -mabi=sysv -march=i486 -O2`; dist f782e4c567e6 (parity 7c4d1afdb), callgrind --separate-callers=30, 2026-10-21.

## What Fold costs (200.4 M Ir of 1318 M)
| part | Ir (M) | what runs |
|---|---|---|
| `_numbers` | 115.9 | ThroughMemory 46.6 (MemorySSA walk, w-7's Annotated chain), `Held::of` 34.5 (Counted/Bounded ranges chain, mine), floatfacts `Solved` 29.3 |
| `floatfold::_folded` | 52.4 | `FloatFacts` 47.7 |
| `manager::writes` | 30.8 | call-effects table, shared by both |
| `canonical` (compares, identities) | ~1 | cheap |

Invocations: 61 on d_faces -O2 (55 at -Os), 5 change anything (1 the numbers/edge fold), ~3.3 M Ir for a run that changes nothing, ~20 K Ir per instruction. 28 of 61 find the function byte-identical to the last Fold's end, but those are the small ones (218 of 9932 instructions): a skip test saves little.

## What Fold carries (objects, -O2, 416 compared)
Skipping all of Fold changes 50 (27 bigger, 13 smaller); `_numbers` alone 12 (10 bigger: run/loop_fusion -Omax +1674/+1447 B, qcport/lights +641, x_fir +244, client +108, nbody_fixed +64, savegame +81...); floatfold alone 14 (6 bigger, 2 smaller); the rest is canonical forms (cheap). Replaced values on the differing objects: 134 loads, 67 icmp (downstream of loads), 23 casts, 9 binary, 8 phi: it is load forwarding of known memory.

## Which pin pass does the job (x_fir, `opt -passes='default<O2>' -print-after-all`, system clang/opt 20)
coef[16] filled by a constant loop, read by an unrolled inner loop: LoopFullUnroll makes 16 loads of @coef; **GVN** (the pass after unroll, store-to-load forwarding) leaves 3; the module-level **GlobalOpt** after the inliner folds the rest (an internal global only stored constants). Our order already has unroll@753 -> sroa@776 -> gvn@787; so the job is GVN's load forwarding (MemDep/MemorySSA) + a GlobalOpt after the CGSCC inliner, not Fold.

## Plan (order by Ir; each PR has Ir before/after on dist and objects identical or a bytes/clocks row; `Deletes:` line, no switch)
1. (10-30) Find, per differing object, the load our GVN leaves that the pin's removes (stages diff with/without `_numbers`: x_fir first), fix GVN's forwarding (translation fidelity) and run GlobalOpt once more after the inliner: then `_numbers`' load path has no object left to carry. Row: objects of the 12 identical with Fold's numbers off.
2. (11-02) Delete `_numbers` (115.9 M) when 1 holds on the corpus; keep the edge fold (phi of numbers = SCCP/JT) only if an object needs it. First Ir drop.
3. floatfold (52.4 M): the pin folds exact floats in InstSimplify/ConstantFolding inside InstCombine; name the 14 objects' pass the same way, then delete or merge into combine.
4. `writes` (30.8 M) goes with its last caller.
Open: loop_fusion is -Omax only.

## Attribution of lights, client, savegame (2026-10-22; redone on parity 44cf50686 + the FOLD_NO_NUMBERS switch, llrm-c 44cf50686abc+dirty md5 7d53725c; the first table was taken on f8c14ca21, which still had `unroll.rs`). Re-checked 2026-10-22 on 41febda30 (llrm-c 41febda30474+dirty, md5 17fe7f72, same command both arms).
| object | what `_numbers` folds | pin pass | gap |
|---|---|---|---|
| client | `_pl_death_selftest@3`: no difference on 41febda30 (asm identical, 731 lines both arms) | left the table with #3597 | none |
| lights | no difference on 41febda30 (asm identical, 220 lines both arms) | left the table with #3596 | none |
| savegame | `_savegame_selftest@3`: 341 vs 380 lines (341 vs 380 on 44cf50686 and on 41febda30; 3592 vs 3627 asm lines) | none: pin `default<O2>` leaves 380 lines, like ours without `_numbers` | ours-only gain; nothing to translate |

## Mechanism, to be checked on the stage dumps before the PR (2026-10-22)

The x_fir coefficient table is an array global of 16 elements stored by constant stores and read by 16 unrolled loads. LLVM's GlobalOpt splits an array global of up to 16 elements into one global per element (SRAGlobal) and replaces a global stored once by a constant (optimizeOnceStoredGlobal); our GlobalOpt only marks a never-stored global constant. So after the GVN forwarding fix, the second GlobalOpt slot needs SRA of small arrays and the once-stored fold, not only `never_stored`.

## What `_numbers` still changes in QCport (2026-10-22, 41febda30, `FOLD_NO_NUMBERS=1` vs not, same binary; 260 compiles: 65 modules x -O1/-O2/-O3/-Os, 0 failed)
19 objects differ, 11 modules. Bigger without `_numbers`: host/savegame (+44/+81/+83/+18 B at O1/O2/O3/Os), game/mdl_ai O1 +71, game/ent O2/O3 +14, game/weapons O1 +7, sound/snd_mix O1/Os +5/+1, ui/sbar O1/O2 +5/+2, render/dl O1 +2. Smaller without it (`_numbers` makes them bigger): host/cmd O1 -17, host/keybind O1/O2/O3/Os -15/-15/-5/-5, render/d_alias O1 -11. These 11 modules are the deletion list for `_numbers`; savegame is ours-only (the pin does not get it). Script: `numbers-corpus.py` (not in git).

## Deletion list for `_numbers` (2026-10-22, 41febda30, vsgcc corpus + QCport, `FOLD_NO_NUMBERS=1` vs not, same binary)
vsgcc (568 compiles, 0 failed): 13 objects change, all bigger without `_numbers`: x_fir m32/m16 O2,O3 (+558/+1385 B; GVN forwarding of the coefficient loads, #3569, then GlobalOpt SRA and once-stored fold), nbody_fixed O2,O3 m32/m16 (+287..+406 B), x_cordic O3 m32/m16 (+50/+110 B), lru m16 O2,O3,Os (+82 B). QCport: the list above (savegame is ours-only). Unattributed objects: nbody_fixed, x_cordic, lru m16, and the QCport modules other than savegame; each needs its pin pass named (as for x_fir) before it can be a PR.

| step | date |
|---|---|
| name the pin pass for nbody_fixed, x_cordic, lru m16, mdl_ai, ent, weapons, snd_mix, sbar, dl | 10-29 |
| GVN forwarding (#3569) and GlobalOpt SRA of small arrays plus the once-stored fold | 10-30 |
| the objects still changed get a fix or a numbered row each; savegame is a row (ours-only gain) | 11-01 |
| delete `_numbers`, then `writes` with its last caller, then floatfold into combine | 11-02 |

## x_fir after #3569: GlobalOpt's SRA and localize do it (2026-10-22, 25d838613, llrm-c 25d838613b0a+dirty)
With `_numbers` off x_fir still has 3 loads of `coef` (offsets 0, 2, 4) in the preheader of the dot-product loop (+279 B at m32 -O2, +900 B at m16). `-fno-tree-pre` takes them away in our pipeline (asm 189 -> 182 lines, equal to the `_numbers` arm), and the pin's `gvn` alone removes them from our final dump, but that is the wrong probe: the pin's own order, run from our 09-pipeline MIR (sroa, early-cse, instcombine, loop-unroll-full, instcombine, gvn) also leaves 4 `coef` loads, and its next module pass, `globalopt`, removes the global: it splits `coef` into 16 `i16` allocas in `bench_x_fir` (SRA, then LocalizeGlobalVariable: one accessing function that does not recurse and writes the value before reading it), and the following instcombine/gvn fold the loads. So the plan stands: SRA of the global by accessed pieces, then localize it (`isPointerValueDeadOnEntryToFunction`), as a second GlobalOpt slot after the inliner. A first reading of this table (GVN's load PRE, GlobalOpt dropped) was wrong: it probed the pin's `gvn` on our final dump instead of the pin's own sequence.

## Dates

- 10-28: attribution of qcport/lights, client and savegame: done 10-22 (table above).
- 10-30: GVN forwarding fix and GlobalOpt after the inliner, one PR each, objects of the 12 identical with `_numbers` off.
- 11-02: delete `_numbers`.
