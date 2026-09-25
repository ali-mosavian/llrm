# Allocated index doubling

ADDRM's QB loop, before:

```asm
mov si,bx
shl si,1
```

After:

```asm
lea si,[ebx+ebx]
```

Both occupy four bytes in 16-bit mode. Low-word addition is independent of
the source's unknown upper bits; LEA accesses no memory. The peephole requires
a same-width word/dword register copy and shift by one, excludes ESP as the
index, and proves the flags are overwritten within this block before any
possible observer. Unknown operations, carry consumers and block boundaries
stop that proof. The existing zeroing peephole shares the same flag rules.
Original byte coverage is retained, including the inserted-copy shape in
ADDRM where the shift owns the bytes and the copy owns none.

The default 386 scoreboard ranks copy plus shift at five, LEA at two.
Across twenty iterations ADDRM saves 60 estimated core-cost units:

| Compiler | Before | After | After / target 754 | Object bytes |
|---|---:|---:|---:|---:|
| PDS /G2 | 1126 | 1066 | 1.414 | 886, unchanged |
| QB /O | 1132 | 1072 | 1.422 | 869, unchanged |
| VBDOS /G3 | 1126 | 1066 | 1.414 | 1123, unchanged |

This is a static ranking, not measured execution time. In particular, later
CPUs may penalize the partial-register dependency introduced by reading EBX
after writing BX; these figures do not establish a P6 speedup.

An audit of 96 primary fixtures changes only ADDRM, STRIDE and DIVMOD in
each compiler variant, with no new emission refusal. STRIDE size is unchanged;
DIVMOD saves one byte per variant. All nine combinations pass actual-program
checks (69 cases total). The new ADDRM emitted-code regression was observed
to fail before the change. The focused peephole file passes 61 tests.
All stages were dumped before and after; no broad test-suite run was needed.
