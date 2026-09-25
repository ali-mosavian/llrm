# Constant propagation into arguments

MIR folding now substitutes a proven constant for a scalar argument read.
It requires every byte, preserves the argument's stack write, and removes
only the replaced memory read. Unknown or partial facts leave the argument
unchanged. The resulting numeric operand owns no address relocation.

QB NOTS's first printed result, before:

```asm
push word [r+2]
push word [r]
call far B$PEI4
```

After:

```asm
push word 0EDCBh
push word 0A987h
call far B$PEI4
```

QB static cost falls 462 to 438: 1.43x its unchanged 306-unit reference,
below the 459-unit goal limit. PDS falls 450 to 426 (1.39x); VBDOS falls
400 to 388 (1.27x). These are not runtime timing measurements.

The new operands exposed a backend encoding limitation: unsigned dword bit
patterns above 7fffffffh need conversion to the signed integer accepted by
iced's constructor, without changing their bits or pushed width. The selector
now uses its existing immediate conversion for pushes, with fail-first cases
80000000h, EDCBA987h and FFFFFFFFh.

The twelve changed PDS fixtures and their QB/VBDOS variants pass all 102 output
checks. Sixty focused argument, selector, relocation and scoreboard checks pass.
The NOTS emitted-code regression failed first on all three compilers.
