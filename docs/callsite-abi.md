# External call ABI

Unknown non-runtime calls use a C/Pascal ABI assumption at the raise boundary:

- An immediately following positive `add sp, immediate` selects C caller cleanup.
- Otherwise Pascal callee cleanup is assumed, but the argument byte count stays
  unknown. Absence of caller cleanup does not prove the count.
- AX, BX, CX, DX, SI and DI remain possible inputs. Arithmetic flags are not
  language-ABI arguments. Memory, clobber and control effects remain conservative.
- Explicit external contracts override inference. BASIC runtime helpers (`B$…`)
  keep their separate contracts; their conventions are not inferred this way.

This is a calling-convention assumption, not callee analysis. Delayed C cleanup
or unusual assembly interfaces need explicit contracts. MIR passes see no new
machine details.

## Renderer smoke checks

Fresh qb-qrender main `1c9c30a`, DM3ISH, normal/Pentium III/75,000 cycles,
native FPU, identical library/assets. C and assembly objects are unchanged.

| Added optimized module | Object bytes before → after | Mean FPS | Saved frame |
| --- | --- | --- | --- |
| main | 12,924 → 11,289 | 13.79 | byte-identical to baseline |
| h_frame, before order fix | 5,104 → 4,159 | 13.79 | identical pixels, profiling miscompiled |

These first runs completed 60 ticks and rendered 19 frames. Matching pixels
did not establish full correctness: h_frame incorrectly disabled profiling.
The whole renderer is not yet optimized or validated.

### Emission-order fix

LIR put zeroing before the comparison, but emission sorted it back by its
original source address. This made `dparm.prof` always zero:

```asm
; broken emission           ; fixed emission
test ebx,ebx                xor ax,ax
xor ax,ax                   test ebx,ebx
jle done                    jle done
mov ax,0FFFFh               mov ax,0FFFFh
```

Lowered bodies now require their instruction sequence to survive emission.
The emitted-byte regression fails with the old rule restored; 19 focused
checks pass. In the isolated corrected h_frame run, `pt_build_mean` recovered
from zero to 13.18 ms, the image remained byte-identical to baseline, and FPS
was 13.67. Evidence: `/tmp/qbopt-qrender-order.rIh47r/`.

The separate d_surf trial reduced object size from 25,053 to 21,373 bytes and
rendered identical pixels at 13.62 FPS, but used the broken h_frame build.
It must be repeated with corrected objects; none of these short runs proves
a speedup.

### Integrated rebuild with corrected order

`/tmp/qbopt-qrender-current.btIQeb/` contains the fresh 16/17-module rebuild,
its per-module outcomes, LINK map and benchmark evidence. Only snd is refused
(`B$FEVS` input contract unknown); its original object is retained explicitly.

- FPS: 13.88 versus 13.79 baseline, a short smoke only.
- Saved frame: byte-identical; geometry, camera and surface-cache counters match.
- Build profiling: active, 12.87 ms rather than the erroneous zero.
- EXE: 433,722 → 437,898 bytes. Smaller OBJ files did not mean smaller code:
  LINK's code-segment lengths also increased. Do not report object shrinkage
  as an optimization win.
- Layout checks: 2,864 passed, 79 failed. Two representative failures reproduce
  without entering LIR lowering (folded-op instruction counting and a fold test
  that no longer exercises a memory fold). These remain unresolved, not waived.

### Longer run: surface warm-up

Same executables and emulator configuration, sequential runs, `-bench 1200`
without `-ticks`. The built-in timer excludes the first four frames, not a
separate long warm-up window. Each run completed 1,200 frames (1,196 measured).

| Metric | Baseline | Optimized, 16 modules |
| --- | ---: | ---: |
| Mean FPS | 15.5367 | 15.7961 |
| Final FPS reading | 16 | 16 |
| Mean frame time, ms | 64.3636 | 63.3067 |
| Mean surface-build time, ms | 0.1654 | 0.1613 |
| Surfaces built / evictions | 281 / 0 | 281 / 0 |

Images are byte-identical and geometry/camera counters match. Observed FPS
change is +1.67%, not a demonstrated speedup: this is one pair and timer
calibration varies between launches. Six interleaved pairs remain necessary
for a performance claim. No source or pass changed during these runs.

Evidence: `/tmp/qbopt-long-baseline.Hm6ZTS/` and
`/tmp/qbopt-long-opt.bcKVnQ/`, each containing the exact config, executable,
`BENCH.TXT`, `BENCH.BMP` and `benchmark.png`.

### Remaining runtime interface

The VBDOS B$FEVS input contract is now bounded by disassembly. Its first helper,
B$RefStringArgLast, executes `OR AX,AX` before reading arithmetic flags or
making another call. Six GP inputs remain conservative; cleanup, memory,
clobber and control effects remain unknown. The full dependency audit was
incomplete, so its apparent `RETF 2` was not promoted to a cleanup guarantee.

The fail-first contract test also fails when the new variant is removed.
QB/PDS contracts are unchanged. snd now emits (4,559 → 4,013 object bytes),
bringing emission coverage to 17/17 BASIC modules. The external call is still
a call, not an absorbed operation:

```asm
; before / after: same stack argument and descriptor result
push offset envName
call far B$FEVS
push ax
lea ax,[bp-1Ch]
push ax
call far B$SASS
```

Stage dumps: `/tmp/qbopt-snd-stages/`. Audit disassembly:
`/tmp/qbopt-fevs-contract.json` and `/tmp/qbopt-refstring-contract.json`.

One h_frame floating update, before (emulated x87 shown as its equivalent):

```asm
mov si,[bp+8]
fld dword [si]
fadd dword [bp+1Ch]
fstp dword [si]
wait
```

After (native x87; same SINGLE store, no BASIC checkpoint):

```asm
mov si,[bp+8]
fld dword [si]
fadd dword [bp+1Ch]
fstp dword [si]
```

All h_frame stages: `/tmp/qbopt-qrender-frame-abi.1TS8gz/frame-stages/`.
Run evidence: `BENCH.TXT`, `BENCH.BMP` and `live.png` in the same parent folder.
Before h_frame: `/tmp/qbopt-qrender-main-abi.zhAJZK/`.
