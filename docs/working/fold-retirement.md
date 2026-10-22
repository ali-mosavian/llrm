# Retiring `fold` slot by slot

Owner: w-6. Each slot PR shows d_faces -O2 m32 Ir (dists, `-m32 -mabi=sysv -march=i486`), objsweep and QCport (`tools/rows/qcport.py`, m16).

| Slot | d_faces M | objsweep | QCport B | PR |
|---|--:|---|--:|---|
| fold@677 | 1305.5 -> 1259.9 (-45.6) | 0 of 568 changed | +119 of 1,233,771 | #3818 |
| fold@768 | 1303.3 -> 1278.8 (-24.5) | 3 of 568 (nbody m32 O3 -40 B) | -105 of 1,233,701 | this PR |

## FR-1: the eight objects larger without fold@677 (owner w-6, review 2026-11-04)

Asm of the `fold677` dists (62d45c8880 vs 77048ad015) per object, instruction multiset and diff:

| Object | B | What differs | Cause |
|---|--:|---|---|
| savegame O1/O2/O3 | +2/+21/+21 | the same instructions; `je`/`jne` flipped and blocks reordered | layout: a block order that changes jump encodings, not a rewrite |
| mdl_ai O2/O3 | +2/+9 | O3 identical multiset; O2 `jl` for `jge` | layout, as above |
| mdl_ai O1 | +15 | two extra stores: `mov [bx+48], 0` and `mov dword [bx+54], 1094189056` | a store the old fold dropped (its value evaluator proved the cell already held it); the pin drops it in GVN/DSE, not InstCombine: to check |
| savegame Os | +51 | `sete`/`xor al, 1` for `setne` (4 sites) | `xor (icmp eq), true` = `icmp ne`: InstCombine has it (visitXor on a compare); to translate into `combine` |
| mdl O3 | +12 | a spill slot and one extra `mov` | register/slot choice after a one-instruction difference: unexplained |

Not checked against the pin's asm for the same IR. Owner w-6: translate the xor-of-compare fold into `combine`
(2026-11-04), find the stores' pass (2026-11-04); the layout rows stay as growth with this number.
Later slot PRs carry a row per growth cause in their body.
