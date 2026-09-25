# Whole scalar arguments

The raise now rejoins adjacent high/low argument words when both are proven
extracts of the same four-byte value. The memory stack receives exactly the
same four bytes in the same order. Reversed halves, duplicate halves, separated
pushes, memory operands and observable intermediate effects are not joined.
Lowering selects the machine instruction; optimization sees one whole argument.

ARITH previously passed an AND result like this:

```asm
and eax,ebx
push eax
pop bx
pop cx
push eax
pop ax
pop ax
push ax
push bx
```

It now emits:

```asm
and eax,ebx
push eax
```

PDS/G2 comparisons, before versus after this change:

| Program | Static cost before | After | Object bytes before | After |
|---|---:|---:|---:|---:|
| ARITH | 1130 | 794 | 1551 | 1487 |
| DIVMOD | 1934 | 1462 | 2249 | 2147 |
| NEGNOT | 354 | 288 | 907 | 899 |

These are static ranking units, not measured execution times. ARITH and DIVMOD
still lack independently derived references. NEGNOT's existing target is 290.

All 102 output cases pass across PDS/G2, QB/O and VBDOS/G3. The ARITH emitted-code
regression fails before the change on all three compilers; ordered-half guards
and the focused constant/scoreboard checks pass (58 checks total).
