# FPBENCH's invariant outer index

The September 10 stage dump at `edd5d4a` retained `body << 2` inside
FPBENCH's `other` loop. Its input was already the outer counter's SSA value;
the old loop-carried destination guard, not an input dependence, rejected it.

A complete scalar left shift with a constant count within the value width,
no merged old contents, and no read flags can bypass that destination guard.
The existing input invariance, placement, crossing and allocation checks still
apply. Other operations retain the old guard. This rule names no registers or
CPU costs; lowering and allocation decide where the invariant lives.

Emitted code before, inside every non-self interaction:

```asm
mov si,cx
shl si,2
fld dword [posX+si]
mov bx,ax
shl bx,2
fsub dword [posX+bx]
```

After, once before the inner loop:

```asm
mov bx,ax
shl bx,2
```

And inside each non-self interaction:

```asm
mov si,dx
shl si,2
fld dword [posX+si]
fsub dword [posX+bx]
```

FPBENCH, VBDOS v-g3, 50,000 steps, pinned DOSBox configuration, tuning 386,
native-FPU replacement off: three baseline runs followed by three optimized
runs. Baseline PIT ticks `9269760, 9269758, 9269762`; optimized
`6831062, 6831062, 6831062`. Median baseline 7,768.941 ms, optimized
5,725.080 ms, versus 5,820.381 ms immediately before this change (about 1.6%
less runtime). All twelve rounded printed coordinates and DONE matched;
this is not a bitwise floating-state check or a hardware latency measurement.
Compile and link logs were clean. No floating operation was changed.

The production regression failed before the fix. Six boundary checks cover
live flags, partial output, merged contents, invalid counts and unknown
liveness. The inner counter's shift must stay inside the loop. A differential
emission check of all 487 `tests/fixtures/omf` objects, disabling only this new
exception for the baseline, found no changed bytes or emission outcomes.

Full stage dumps, timed outputs and build logs are retained locally at
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-invariant-shift-4c3nvye3`.
The previous stage dumps are at
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-fpbench-hot-wfzhj5au`.
