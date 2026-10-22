# Retiring `fold` slot by slot

Owner: w-6. Each slot PR shows d_faces -O2 m32 Ir (dists, `-m32 -mabi=sysv -march=i486`), objsweep and QCport (`tools/rows/qcport.py`, m16).

| Slot | d_faces M | objsweep | QCport B | PR |
|---|--:|---|--:|---|
| fold@677 | 1305.5 -> 1259.9 (-45.6) | 0 of 568 changed | +119 of 1,233,771 | #3818 |

## FR-1: objects larger without fold@677 (owner w-6, review 2026-11-04)

QCport m16, fold@677 removed: mdl O3 +12, mdl_ai O1 +15, O2 +2, O3 +9, savegame O1 +2, O2 +21,
O3 +21, Os +51 (client O1 is 14 smaller). w-8 attributes these objects to fold's value evaluator
(`_numbers`, docs/working/fold.md). Exit: the cause named from a stage diff of the body before and after the slot,
then either translated into `combine` or accepted with the number.
