# Retiring `fold` slot by slot

Owner: w-6. Each slot PR shows d_faces -O2 m32 Ir (dists, `-m32 -mabi=sysv -march=i486`), objsweep and QCport (`tools/rows/qcport.py`, m16).

| Slot | d_faces M | objsweep | QCport B | PR |
|---|--:|---|--:|---|
| fold@677 | 1305.5 -> 1259.9 (-45.6) | 0 of 568 changed | +119 of 1,233,771 | #3818 |
| fold@768 | 1303.3 -> 1278.8 (-24.5) | 3 of 568 (nbody m32 O3 -40 B) | -105 of 1,233,701 | #3832 |
| fold@801 | 1202.5 -> 1089.9 (-112.6) | 0 of 568 changed | +846 of 1,233,858 (34 objects, 3 smaller) | #3870 |
| fold@837 | 1088.0 -> 1071.5 (-16.5) | 11 of 568 changed (+800 B net) | +553 of 1,234,704 (33 objects, 23 larger) | this PR |

## FR-1: the eight objects larger without fold@677 (owner w-6, review 2026-11-04)

Asm of the `fold677` dists (62d45c8880 vs 77048ad015) per object, instruction multiset and diff:

| Object | B | What differs | Cause |
|---|--:|---|---|
| savegame O1/O2/O3 | +2/+21/+21 | the same instructions; `je`/`jne` flipped and blocks reordered | layout: a block order that changes jump encodings, not a rewrite |
| mdl_ai O2/O3 | +2/+9 | O3 identical multiset; O2 `jl` for `jge` | layout, as above |
| mdl_ai O1 | +15 | two extra stores: `mov [bx+48], 0` and `mov dword [bx+54], 1094189056` | a store the old fold dropped (its value evaluator proved the cell already held it); which pin pass drops it, and which of ours misses it: stage diff before/after the slot, w-6, 2026-11-04 |
| savegame Os | +51 | `sete`/`xor al, 1` for `setne` (4 sites) | `xor (icmp eq), true` = `icmp ne`: pin check (`pin.opt('instcombine')`, m32 target): `icmp eq x,0` then `xor true` becomes `icmp ne x,0`; ours: `algebraic` does it, `fold` and `combine` do not (visitXor untranslated); translate into `combine`, w-6, 2026-11-04 |
| mdl O3 | +12 | a spill slot and one extra `mov` | register/slot choice after a one-instruction difference: unexplained; explain by 2026-11-02 (regalloc row for splitkit, or pin-faithful) |

The pin cannot give asm for these m16 objects (its code16 target has 32-bit registers, no 8086 model), so the check is at the IR level with the pin's own passes. Owner w-6: translate the xor-of-compare fold into `combine`
(2026-11-04), find the stores' pass (2026-11-04); the layout rows stay as growth with this number.
Later slot PRs carry a row per growth cause in their body.

## FR-2: objects larger without fold@801, by cause (owner w-6, 2026-11-06)

All 34 changed QCport objects (m16, +846 B net), the earliest stage where the bodies differ, `LLRM_MIR_STAGES` of the
`fold801` dists with value numbers normalised (`run.py`, results `fr2_result.json`):

| Cause | Objects | B | The difference |
|---|---|--:|---|
| A. `foldNot` untranslated | 30: client O1 -14, d_alias O1/O2/O3/Os +10/+16/+16/+156, d_faces O1/O2/Os +1/+5/+19, ent O1/O2/O3 +1/+12/+12, mdl_ai O1/O2/O3/Os +31/+119/+111/+67, pak O2/O3/Os +18/+18/+6, pl_trace O1/O2/O3/Os +2/+1/+1/+4, savegame O1/O2/O3/Os +18/+24/+23/-8, weapons O1/O2/O3/Os -3/+26/+26/+17 | +735 | `br i1 %c, A, B` is `%n = xor i1 %c, true; br i1 %n, B, A`: the not of a condition is kept (an `and`/`or` of compares inverted by `xor true` in place of the inverted compares), and the branch swap does not apply to what `foldNot` would have rewritten first |
| B. float constants not folded | 2: ls O2/O3 +61 | +122 | `fmul float 0x3FF0CCCCC0000000, 1.0e1; fptosi; trunc` stays where fold's value evaluator made the integer constant |
| C. select with an undef arm | 1: sc O1 -20 | -20 | `select %c, X, undef` is X in fold; smaller without |
| D. none in the cleaned stages | 1: combat Os +9 | +9 | the bodies agree through the dumped stages: layout or a later pass |

A is the pin's `foldNot` ([InstCombineAndOrXor.cpp#L5237](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L5237)),
which calls `getFreelyInverted` ([L5428](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L5428)): `combine_xor.rs` lists it
among the folds left out (DEVIATE 2). Plan: translate `foldNot` and `getFreelyInverted` into `combine` (w-6, 2026-11-06); B: the
float constant fold of `fmul`/`fptosi` in `combine` (w-6, 2026-11-06; the pin folds a binop of constants in `simplifyFMul`);
C and D close as numbers (C is a gain).

## FR-3: objects changed by removing fold@837, by cause (owner w-6, 2026-11-06)

QCport m16 (+553 B, 33 objects; earliest differing normalised stage of `LLRM_MIR_STAGES` of the `fold837` dists, `run3.py`):

| Cause | Objects | B |
|---|---|--:|
| A. `foldNot` untranslated (the FR-2 cause: `xor i1 %c, true` kept before a branch) | 23: combat O1/O2/O3/Os -33/-87/-83/-1, d_alias Os +1, d_faces Os -19, ent Os -1, mdl O3 -40, mdl_ai O1/O2/O3/Os +6/+35/+37/+28, pak Os -1, pl_trace Os +29, savegame O1/O2/O3/Os +4/+13/+16/+147, sc Os +1, weapons O1/O2/O3/Os +48/+27/+27/+18 | +172 |
| B. float constants not folded (the FR-2 cause) | ls O2/O3 +165 each | +330 |
| E. not classified: the first hunk is none of the above | keybind Os -5, mod_tex O1/O2/O3/Os +29/+10/+10/+6, sbar O1 +5, snd_mix Os +1, sys Os -5 | +51 |

Rows corpus objsweep, 11 objects. RETRACTED (w-8, 2026-10-24): objsweep wrote `x.old`, which llrm-c writes as assembly text, so every size here was text
bytes. Object bytes, same two binaries: x_fir m16 O2/O3 +21 each and m32 -16, x_cordic m16 O3 +34 and m32 O3 +16, lru m32/m16 Os +10/-1, floats m32 Os -4,
sieve m32 Os -4, nbody m32 Os -4 (+57 B in all). x_fir m16 +21: not GlobalOpt. MIR is -7 instructions in the new build (walk +16, GlobalOpt then
-39) and the machine code -7 until RegAlloc, +8 after it: a register-allocation difference at m16 on the changed IR (tools/sizebisect). The others: causes owed with the E group, w-6, 2026-11-06. The translations of FR-2 (`foldNot`,
the float constant fold) are expected to close A and B. qbq-llrm d_faces: object identical (8170 B).

## Retraction of the cause column of FR-2 and FR-3 (w-6)

The "cause" of FR-2 A (and FR-3 A) is the *first* normalised stage where the two bodies differ (`br %c` against
`xor i1 %c, true; br`), not shown to be what grew the object. The three late InstCombine slots (combine@1403, @1464, @1499, the
pin's L1403, L1464 and L1499) clean exactly that form, and with them QCport's FR-2 objects (savegame, weapons, ls, d_alias, ...) do not
change (`lateic` pair, base 1586dd80c1): the growth comes from a later divergence the first difference only precedes. The per-object
cause stays open (w-6, 2026-11-06); `foldNot` is not shown to matter for it either.

## FR-2 and FR-3 redone by size bisection (w-6)

`tools/sizebisect/firstdiff.py` over the `LLRM_MIR_STAGES` dumps of the two builds of each changed QCport object (new-only steps
counted). The first differing step is `simplifycfg@1438` in 14 of the 34 FR-2 objects and 13 of the 33 FR-3 ones, `walk` (the IPO
snapshot) in 14 and 19, `loop-load-elim@1400` in 5 and 1, `tailrec@1677` in 1. In the `simplifycfg@1438` and `loop-load-elim@1400`
cases the stage before is textually equal in the two builds (value numbers normalised): the same input gives another output. What
differs is not in the text: the value and block numbering, and so the order the late SimplifyCFG walks blocks, predecessors and phis
in. So the growth is not a rewrite `combine` lacks, and not one `fold` did: removing a slot upstream renumbers the body and SimplifyCFG
then decides differently on the same text (the `xor i1 %c, true` before a branch is one of its outputs). The three late InstCombines
(#3929) clean that form and change none of these objects, which fits. Open (w-6, 2026-11-13): which order SimplifyCFG reads (predecessor
order from the use list, a phi order) and what the pin's reads there; until then the rows stay numbered as order sensitivity of SimplifyCFG@1438.

## fold@1242 (FR-4, w-6)

The last fold slot (Cleanup, after combine@1242) is deleted. Base `a1d9045963`, new `ff0f925aaa`, dists, ledger command: d_faces qbq-llrm 1064.5 -> 1038.4 M Ir (llrm-only bucket 72.0 -> 45.3; InstCombine +0.5), object identical; QCport d_faces 889.1 -> 858.3 M.
Pair (71 programs, 852 rows): clocks change on 2 rows, both -Os with the same bytes: nbody_single 8081134 -> 8789134 (+8.76%), fpbench 155773 -> 169933 (+9.09%); d_faces -O3 +0.04%. No row crosses min(gcc, clang). Code bytes: objsweep 5 objects, all +0 (fpbench, nbody_single -Os); m16 sizes -Os total +238 B (fpemu.bas +86, hex_oct_negative +33, textfill +8); QCport 65 modules -0.03%..+0.02% (8-9 modules per level: cmd -50, keybind -40, d_alias +9, host +16). No new tests/run failure.
Cause of the two -Os clock rows (found 2026-10-24): `fold`'s `floatfold::freedoms` was the only place `fdiv x, C` became `fmul x, 1/C`; combine had no float visit. `foldFDivConstantDivisor` in combine (#4241) restores both rows exactly (nbody_single 8789134 -> 8081134, fpbench 169933 -> 155773). Owed (w-6, 2026-11-06): the QCport module deltas.
