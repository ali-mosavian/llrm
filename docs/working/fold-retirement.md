# Retiring `fold` slot by slot

Owner: w-6. Each slot PR shows d_faces -O2 m32 Ir (dists, `-m32 -mabi=sysv -march=i486`), objsweep and QCport (`tools/rows/qcport.py`, m16).

| Slot | d_faces M | objsweep | QCport B | PR |
|---|--:|---|--:|---|
| fold@677 | 1305.5 -> 1259.9 (-45.6) | 0 of 568 changed | +119 of 1,233,771 | #3818 |
| fold@768 | 1303.3 -> 1278.8 (-24.5) | 3 of 568 (nbody m32 O3 -40 B) | -105 of 1,233,701 | #3832 |
| fold@801 | 1202.5 -> 1089.9 (-112.6) | 0 of 568 changed | +846 of 1,233,858 (34 objects, 3 smaller) | this PR |

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

## FR-2: objects larger without fold@801 (owner w-6, review 2026-11-04)

QCport m16, fold@801 removed: 31 objects larger (largest d_alias Os +156, mdl_ai O2 +119, O3 +111, O1 +31, Os +67,
ls O2/O3 +61, savegame O1/O2/O3 +18/+24/+23, weapons +26/+26/+17, ent O2/O3 +12, pak O2/O3 +18), 3 smaller (client O1 -14,
sc O1 -20, savegame Os -8). Asm instruction multisets of five of them, `fold801` dists (7bbe8e4c7b vs 081c72b216):

| Object | B | What differs |
|---|--:|---|
| d_alias Os | +156 | a branch chain on a compare of a stored word is a `test ax, ax; je` pair and two x87 loads (51 more instructions) in place of stack-slot compares and constant moves: the compare/select folds fold did |
| mdl_ai O2 | +119 | `sete al; and cl, al` (4 sites) and `je` for `jne` chains: an and of compares kept as a value, not a branch |
| ls O2/O3 | +61 | `idiv bx`/`fmul` by a constant in a table (`$K28`) for `lea [ecx+ecx*4]` and `idiv cx`: a multiply/divide by a constant folded differently |
| weapons O2 | +26 | `sete dl` (3 sites) for `je` branches, and a different x87 stack shuffle (`fstp st(3)`/`fxch`) |
| savegame O2 | +24 | `fcompp; fnstsw ax` for an integer compare, one more spill (`sub sp, 360` for 358) |

Cause not yet named per rewrite (fold's value evaluator `_numbers` at this slot, per w-8's table): the next step is the
stage diff of each body before and after the slot, then the rewrite translated into `combine` or the number accepted.
Not checked against the pin (m16 has no pin asm; the check is the pin's InstCombine on the stage IR).
