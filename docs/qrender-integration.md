# Qrender correctness gate

New optimization passes are paused until the optimized renderer builds and
runs correctly. Target: `qb-qrender/.claude/worktrees/qgl-poly-draw`, source
commit `2965fa9d91c8e5f14fd3cddd804219956c75e42c`.

## E1M1: native build fails during screenshot allocation

The all-21-module native build below passes dm3, **not E1M1**.
Isolated run: `/tmp/qbopt-qrender-e1m1.qxAC64`, normal core, Pentium III,
75,000 cycles, 64 MB; both executables use:

```text
e1m1.bsp -lm -nostats -yaw 183 -bench 40 -ticks 60
```

Untouched baseline: **18.34392 FPS**, 19 frames, completed BMP and return to
DOS. Native: **no valid FPS**, only the 54-byte BMP header before failure.
Evidence: `e1m1-qbase.exe.TXT`, `e1m1-qbase.exe.BMP`, inspected screenshots
`e1m1-qbase.exe-013.png` and `e1m1-qrender.exe-016.png` in that directory.

Assets were regenerated using **the pinned** `tools/mkassets.py` and
`tools/mkportals.py`, not the modified working-tree packer. Reusing dm3's
archive first produced the expected map-mismatch rejection; that was setup,
not a compiler failure. Both measured E1M1 runs used the same assets:

```text
e1m1.bsp  7b7061ec63c3e8ecb9c0e0a8075f18823efea6578666d57d601c191bcaf16c26
assets.zip 7407e19011f0dc64b7ba9953e5e38bdc758350554910c8ef84966e4a7efb4037
```

The first-error breakpoint at loaded `B$RUNERR` (`33ea:52a3`) records
**BX=14, out of string space**. SCREEN's `45c6: call B$LDFS` (original
site `3bb7`) requests one palette-component byte in `SCR_SCREENSHOT`.
`B$AlcTmpSH` calls `FResizePpv` with three bytes including its header and
gets failure. The temporary-descriptor cursor is `47a6`, below limit
`4812`; descriptor exhaustion is not the explanation. The later visible
MAIN `0824:0131` out-of-memory error is secondary, in error reporting.
All SCREEN stage dumps remain in `/tmp/qbopt-screen-native`.

Actual code-segment lengths, rather than total OBJ file sizes:

| Code | BC bytes | Native bytes | Growth |
| --- | ---: | ---: | ---: |
| SCREEN | 16,036 | 19,124 | 3,088 |
| D_SURF | 12,547 | 15,220 | 2,673 |
| All 21 BASIC modules | 113,386 | 131,095 | 17,709 |

Every module grew. Depth-stage far free memory falls from 44,640 to 26,928
bytes (17,712 bytes, consistent with aligned code growth). Earlier OBJ-size
reductions are **not code-size reductions**. This is a measured regression,
not yet proof of the allocator failure's cause. Next: distinguish heap
growth failure from corrupted allocation state before changing contracts
or allocation. No compiler fix or passing regression is claimed here.

### Matched heap comparison: the larger image exhausts available space

The untouched executable was stopped at the same palette entry, **39**,
before original SCREEN `3bb7: call B$LDFS`. Its heap chain has **17,472
free bytes**; native has zero. Every corresponding block has the same size
except the string heap: baseline 688 bytes, native 448. Both chains have
the same upper sentinel (`7b57`); DGROUP moves from `434e` to `47a1`.
Thus **17,712 bytes of loaded-image growth minus 240 fewer allocated string
bytes = all 17,472 bytes of baseline headroom**. This is capacity exhaustion,
not merely a correlation with the earlier depth-stage memory reading.

Evidence: `baseline-heap-at-palette-39.json` and
`baseline-palette-39-dgroup.bin` in `/tmp/qbopt-e1m1-baseline-heap.7PhNBf`;
native `native-first-error-dgroup.bin` in the E1M1 run directory. The chain
was read using the actual linked allocator's descriptor layout and checked
block by block. This breakpoint run is diagnostic, not an FPS measurement.
Reduce backend code growth without weakening call contracts or changing the
renderer to reserve less memory; rerun E1M1 after the first measured reduction.

### First backend reduction: share literal argument materialization

SCREEN's native emission now saves **22 code bytes** (19,124 -> 19,102).
The same constant was materialized for the stack and again for a required
call register. The post-allocation peephole now emits:

```asm
; before, 45ba                  ; after, 45a6
push 1                         mov ax,1
mov ax,1                       push ax
mov dx,bx                      mov dx,bx
mov bx,di                      mov bx,di
mov di,[bp-4Ch]                 mov di,[bp-4Ch]
call far B$LDFS                 call far B$LDFS
```

Register contents, pushed bytes and flags agree. Only adjacent synthetic
materializations qualify; stack/frame registers, relocations, constrained
operations and block boundaries are excluded. ABI contracts are unchanged.
All dumps through `s89-lir-prologue.txt` are unchanged; the first difference
is the peephole in `/tmp/qbopt-screen-push-constants`. Fourteen focused
checks pass; disabling the transform makes both emitted-code regressions
fail again. No new runtime/FPS acceptance yet, and 22 bytes does not close
the memory deficit. The larger allocation/copy overhead remains the priority.

## Reproducible native CLI build

All 21 BASIC modules now build through the ordinary CLI with checked-in
profiles in `docs/contracts/`; no temporary monkeypatch scripts are needed.
The recipe is in `docs/contracts/README.md`. It preserves the audited input
bounds and per-caller cleanup facts, checks defining symbols and dependency
hashes, and leaves unknown effects conservative.

Fresh build `/tmp/qbopt-qrender-native-profiles.cyyLSk` (port 2228):
**9.73036 FPS**, baseline **9.43334**, previous native build **9.47705**.
One 13-frame scene, not a statistical speedup claim. BENCH.BMP remains
byte-identical, SHA1 `d6e4096b3610249ff4d53b6829f1ab18ec108c7a`;
`profiles-bench-016.png` was inspected and the run returned to DOS.
The only additional log difference is `dt`: 0.1 -> 0.09917081 seconds.
Pinned `h_bench.bas:225` prints `g.scr.frame_time`; `main.bas:1027` gets it
from `sys_frame_time`, whose `sys.bas:440-459` timer delta is capped at 0.1.
This is a timing field, not an independently changed physics answer. All
other non-timing/non-memory fields match the baseline.

Fresh surface, array and raster checks PASS with byte-identical baseline
logs (79, 617 and 2,701 bytes). Screenshots are
`qrender.exe-qgl{check,arr,diff}-*.png` in the fresh build; these off-screen
checks return to DOS and report no FPS.
The fresh face check still FAILs with a log byte-identical to the untouched
baseline (1,263 bytes, centre-oracle XOR 84). Its screenshot is black during
the intentional post-log timer hold. This failure remains open; it is not
counted as a pass or as proof that all rendering is correct.

Nine rebuilt objects match the previous accepted objects exactly except
for the completion marker: MAIN, H_BENCH, H_FRAME, D_MDL, MOD_TEX, MODEL,
R_BSP, SCREEN and PL_MOVE. The other twelve incorporate intervening backend
fixes. All 21 have completion markers; an unchanged fallback was not counted.
Profile validation: 14 focused tests passed. No compiler code changed here.

COMMON's old/new dumps agree through coalescing; the first difference is
`s103-lir-regalloc.txt`. Repeated CLI emission is byte-identical. At the
original B$ERS1 site 0826, the existing ABI splitter now supplies both
required input registers rather than assigning one shared value to both.
Its two-byte spill also increases the frame adjustment. Actual emitted ASM:

```asm
; previous accepted              ; current CLI build
0944 mov di,bx                   0944 mov di,si
0946 mov bx,cx                   0946 mov [bp-0DCh],di
0948 call far B$ERS1             094a mov di,bx
094d add sp,64h                  094c mov bx,cx
                                094e mov cx,si
                                0950 mov si,[bp-0DCh]
                                0954 call far B$ERS1
                                0959 add sp,66h
```

Full dumps: `/tmp/qbopt-qrender-native-es-20260911/common-stages` before,
`/tmp/qbopt-common-profile-native` after. This closes build reproducibility,
not the broader correctness gate or the 1.5x optimization targets.

## Previous twenty-one-module build — MAIN benchmark accepted

`/tmp/qbopt-qrender-native-main.VVuBHt` links and returns to the DOS prompt
(port 2226). **9.47705 FPS**, baseline **9.43334**; 13 frames. All
non-timing/non-memory benchmark fields match; BENCH.BMP is byte-identical,
SHA1 `d6e4096b3610249ff4d53b6829f1ab18ec108c7a`. Fresh screenshot
`bench-native-023.png` was inspected. MAIN is 31,138 -> 29,694 bytes.
All 21 BASIC modules are now optimized with native x87 enabled; C/ASM
objects remain original. Accepted for this scene only, without a speedup
claim. Broader renderer coverage and the baseline face-oracle failure remain open.

### Dedicated checks on all 21 modules

Fresh runs on the same normal/Pentium III/75,000-cycle machine compare the
accepted executable to an untouched baseline copy, `qbase.exe`. Each uses
`dm3ish.bsp -lm -nostats -yaw 183 -bench 1` plus the named flag.

| Check | Native / baseline | Log comparison |
| --- | --- | --- |
| `-qglcheck` | PASS / PASS | Identical, 79 bytes; conventional and EMS surfaces |
| `-qglarr` | PASS / PASS | Identical, 617 bytes; 16 probes across the 2,730-record page boundary |
| `-qgldiff` | PASS / PASS | Identical, 2,701 bytes; flat, affine and perspective raster cases |
| `-qglface` | FAIL / FAIL | Identical, 1,263 bytes; coverage 1,756, centre-oracle XOR 84 |

The first three returned to the DOS prompt on port 2227. Logs are
`QGL{CHK,ARR,DIFF}.native21.LOG` and baseline `QGL{CHK,ARR,DIFF}.LOG`
in the accepted build directory. Fresh screenshots use
`qrender.exe-qgl{check,arr,diff}-*.png` and `qbase.exe-qgl{check,arr,diff}-*.png`.
These are off-screen checks: prompt screenshots prove completion, not a rendered scene.

The face logs are in the accepted build and
`/tmp/qbopt-qrender-native-screen.cMXOfq` respectively; the latter ran the
untouched `qbase.exe`, not its optimized qrender executable. Fresh face
screenshots are black, as expected for this diagnostic. Its pinned source
holds graphics mode in a TIMER loop for 120 seconds after closing the log;
log completion is not claimed as process completion. No oracle was weakened.
These modes report no FPS; the preceding 9.47705 FPS scene remains the timing evidence.
No assembly changed during these checks.

MAIN now uses the checked-in `docs/contracts/qrender-main.json` profile via
the ordinary rewrite CLI. Its 65 symbols are checked against their defining
objects and all artifact hashes; unknown effects remain unknown. With
`--native-fpu --contracts docs/contracts/qrender-main.json --contract-root
/tmp/qbopt-qrender-baseline-20260910`, `/tmp/qbopt-main-cli-native.obj` matches
the accepted MAIN object exactly apart from the completion marker. Profile
SHA-256: `4b4573460f973a4634d4756e85106158ba152608bc1005688b845f4abc95293f`.
The same profile and native option work in `tools/stages.py`. No machine
instructions changed, so no new FPS measurement is attributed to this step.
The later full-profile migration and fresh runtime evidence are above.

MAIN initially crashed while reserving two spill bytes: it ends through
HOST_SHUTDOWN, not a direct runtime exit. A machine-independent, fixed-point
call-graph analysis now proves that HOST_SHUTDOWN reaches B$CEND and cannot
return; lowering carries that fact to frame insertion. Any returning or
unexplained exit blocks the proof. This follows LLVM FunctionAttrs' no-return
inference; it does not infer purity or preservation. Unsupported frame
layouts now return the original object and reason instead of throwing.

The real MAIN regression fails with the frame fix removed; its negative
case removes the terminal-call proof and still refuses. Ten focused tests
pass. Dumps: `/tmp/qbopt-main-native`, especially `s104-lir-parcopy.txt`,
`s105-lir-prologue.txt`, and `s107-asm-emitted.txt`.

```asm
; original MAIN                 ; emitted MAIN
push cs                         sub sp,2       ; reserve the spill slot
push errorHandler               push cs
call far B$OEGA                 push errorHandler
                                call far B$OEGA
; ...                           ; ... [bp-2] spill/reloads ...
call far HOST_SHUTDOWN          call far HOST_SHUTDOWN
; no normal return              ; no normal return: no epilogue needed
```

### MAIN input audit

MAIN's input-only project audit is `/tmp/qbopt-main-audit-20260911.py`;
its sorted object-name/SHA256 manifest hashes to
`e25c794952bd94f09693ab058aad41f945f7c15a7464f8c4df03a34db12990a1`.
All project calls resolve. BASIC entries enter B$ENRA; C layout checks start
with CMP; assembly drawing/matrix routines start with ADD SP; shutdown helpers
test their state before branching. VGASCREEN calls local 0023 then SFINIT,
which XORs SI before dispatch. ZSCALE and TMRTICKS read no arithmetic flags.
Only incoming arithmetic flags are excluded; GP inputs and unknown side
effects remain. Calls are retained, e.g. `call far QGLVGASCREEN` before/after.
Native emission dumps: `/tmp/qbopt-main-native`.

### Previous accepted build — twenty modules

`/tmp/qbopt-qrender-native-screen.cMXOfq` links and returns to the DOS prompt
(port 2225). **9.46025 FPS**, baseline **9.43334**; 13 frames. All
non-timing/non-memory benchmark fields match. BENCH.BMP is byte-identical,
SHA1 `d6e4096b3610249ff4d53b6829f1ab18ec108c7a`; fresh screenshot
`bench-native-008.png` was inspected. SCREEN is 50,055 -> 46,822 bytes,
native x87 enabled. Accepted for this scene only, without a speedup claim.
MAIN alone remains original; broader coverage and the baseline face-oracle
failure remain open.

SCR_LOAD_PART's first floating load, original 003e versus emitted 0047:

```asm
; before: emulator protocol       ; after: native x87
db 0CDh,35h,46h,08h              fld dword [bp+8] ; D9 46 08
; equivalent to FLD [BP+8]
```

### SCREEN input audit

SCREEN's hash-checked input-only audit is
`/tmp/qbopt-screen-audit-20260911.py`, with native dumps in
`/tmp/qbopt-screen-native`. Runtime evidence is recorded above.
DR drawing entries allocate a frame with ADD SP before branching; RECT
first calls HLINE, which does so. TXTCHAR does the same; TXTROW uses SUB/CMP;
TXTLOADBAS first calls audited FILEOPENBAS. VGAINIT starts with CMP and
VGAPALETTE uses XOR before output. TMRHZ/TMRTICKS have no arithmetic-flag
reads. SF row wrappers enter QGLDC access routines with CMP at 0013/0059
before dispatch; other SF inputs and BASIC B$ENRA entries were audited above.
Only incoming arithmetic flags are excluded; every GP input and all unknown
clobber, memory, cleanup and control effects remain. Calls stay calls.

### Previous accepted build — nineteen modules

`/tmp/qbopt-qrender-native-model.FJ17pP` links and returns to the DOS prompt
(port 2224). **9.37895 FPS**, baseline **9.43334**; 13 frames. All
non-timing/non-memory benchmark fields match. BENCH.BMP is byte-identical,
SHA1 `d6e4096b3610249ff4d53b6829f1ab18ec108c7a`; fresh screenshot
`bench-native-012.png` was inspected. MODEL is 26,132 -> 24,220 bytes,
native x87 enabled. Accepted for this scene only, with no speedup claim.
MAIN and SCREEN remain original; broader renderer coverage and the existing
baseline face-oracle failure remain open.

Existing division absorption, with the divisor 20 prepared before the original
excerpt (008b..008e), versus native output (009c..00aa):

```asm
; before                       ; after
push dword [si]                mov esi,[si]
call far B$DVI4                mov ecx,14h
                               mov eax,esi
                               cdq
                               idiv ecx
```

No compiler change was needed this round. Input-only project contracts are
hash-checked in `/tmp/qbopt-model-audit-20260911.py`; all stage dumps are in
`/tmp/qbopt-model-native`. BASIC callbacks enter B$ENRA, including SB_SEG
at D_SURF 2904, not SB. FILE close/read/size enter local 004c, whose
DEC AX / CMP AX,4 precedes every branch. OPENBAS/ARNEW/MEMALLOC allocate
their frames with ADD SP; ARWIN enters the audited ARMAP. GEM alloc/free/map
and MEM avail/free execute CMP or TEST before branching or dispatch.
All six GP inputs remain; cleanup, memory, preservation and control are unknown.

### Previous accepted build — eighteen modules

`/tmp/qbopt-qrender-native-modtex.yc5nqA` links and returns to the DOS prompt
(port 2223). **9.48704 FPS**, baseline **9.43334**; 13 frames. Every
non-timing/non-memory benchmark field matches. BENCH.BMP is byte-identical,
SHA1 `d6e4096b3610249ff4d53b6829f1ab18ec108c7a`; fresh screenshot
`bench-native-024.png` was inspected. MOD_TEX is 17,249 -> 16,846 bytes,
native x87 enabled. This accepts module eighteen for this scene only.
Main, model and screen remain original; the baseline face-oracle failure
also remains unresolved. The small FPS difference is not a proven speedup.

The existing divide absorption is now exercised in MOD_LOAD_TEXTURES:

```asm
; original 049a: operands on stack     ; emitted 0589..0598
call far B$DVI4                       mov ebx,[bp-6Ah]
                                      mov eax,40h
                                      cdq
                                      mov ecx,edx
                                      idiv ebx
```

`edc07ca` establishes input-only SSEK/STRI contracts, each with a fail-first
real-object regression. Project bounds are hash-checked in
`/tmp/qbopt-mod-tex-audit-20260911.py`; dumps are `/tmp/qbopt-mod-tex-native`.
SF: FROMFILEBAS uses ADD SP at 0297, SIZE uses OR AX,BX at 04a4, PGET
uses CMP at 0461, VIEWNEW uses ADD SP at 03d2. VIEWAIM calls QGLSETVIEW,
whose TEST AX,AX at 01d5 precedes branching. SCR_LOAD_PART/STAGE/STEP,
MOD_LOAD_FLAT and SYS_ERROR enter the previously audited B$ENRA.
All six GP inputs remain; cleanup, memory, clobbers and control stay unknown.

### Previous accepted build — seventeen modules

`/tmp/qbopt-qrender-native-plmove.T2KVRG` links and returns to the DOS prompt
(port 2222). **9.38883 FPS**, baseline **9.43334**; 13 frames. Every
non-timing/non-memory benchmark field matches. BENCH.BMP is byte-identical,
SHA1 `d6e4096b3610249ff4d53b6829f1ab18ec108c7a`; fresh screenshot
`bench-native-011.png` was inspected. PL_MOVE is 46,147 -> 41,444 bytes,
native x87 enabled. This accepts module seventeen for this scene only,
not a demonstrated speedup or complete renderer coverage.
Four BASIC modules remain original: main, mod_tex, model and screen.
The separate baseline face-oracle failure remains unresolved.

```asm
; original 2f4b                 ; emitted 2e69
xchg ax,[bp-1Ch]                xchg ax,[bp-1Ch]
```

### PL_MOVE diagnosis history

PL_MOVE initially refused emission. Its original 46,147-byte
object is now a regression fixture. Native emission exposed three unknown runtime
interfaces: RND0 at 154f, ATN4 at 18a7 and UBND at 28bc. Each focused test failed
on that refusal before its input contract was added (`dde616a`, `5e7cba0`,
`a941933`). All six GP inputs remain; cleanup, clobbers, memory and control
effects remain conservative. Calls are not replaced:

```asm
; before                         ; after contract recognition
call far B$RND0                  call far B$RND0
call far B$ATN4                  call far B$ATN4
call far B$UBND                  call far B$UBND
```

Raw VBDCL10E.LIB audit: RND0's straight-line helper sets arithmetic flags before
ADC; ATN4 starts with SUB SP,0Ah at 0003; UBND executes OR DH,DH at 013b before
its first branch. These establish incoming arithmetic-flag independence only,
not purity or preservation. Library SHA256:
`59ad49b055c4829528301e512abf9b8b0955181024c18282a49839e6c0680301`.
The RND0/ATN4 refusal dumps are `/tmp/qbopt-pl-move-native-rnd0` and
`/tmp/qbopt-pl-move-native-atn4`; both preserve the original object atomically.
No new FPS or screenshot is claimed for these emission-only attempts.

After those interfaces were established, emission refused PUSH at 2337.
The initial MIR incorrectly named its explicit `[BX]` read as the implicit
destination `[SP-8]`. `c8b0c78` gives only PUSH stores and POP loads the stack
slot; explicit memory operands retain their own address and alias facts.
The real PL_MOVE regression failed first on `[SP-8]` and passes with the two
related frame checks (3 tests). In assembly terms:

```asm
; faulty operand — refused       ; intended source restored in MIR
push dword [sp-8]                push dword [bx]
```

The refused re-emission dumps are `/tmp/qbopt-pl-move-native-push-source`.
The preceding refused dumps remain in
`/tmp/qbopt-pl-move-native-ubnd`.

The next refusal, XCHG AX,[BP-1Ch] at 2f4b, retained the correct operands
through allocation but lacked an emitter case. `f3ded97` adds the memory
encoding in the backend only. Six fail-first cases cover both operand orders
at byte, word and dword widths (0.38 seconds). The emitted word form is
`87 46 e4`: `xchg ax,[bp-1Ch]`, unchanged from the original operation; no
load/store expansion weakens its exchange semantics. Fresh native dumps:
`/tmp/qbopt-pl-move-native-xchg`.

### Previous accepted build — sixteen modules

After `f5c4186`, `/tmp/qbopt-qrender-native-rbsp-live.bETvby` links and
returns to the DOS prompt (port 2221). **9.37895 FPS**, baseline **9.43334**;
13 frames / 9 timed samples. Every non-timing/non-memory benchmark field
matches; BENCH.BMP is byte-identical, SHA1
`d6e4096b3610249ff4d53b6829f1ab18ec108c7a`. Fresh screenshot
`bench-native-023.png` was inspected. R_BSP emits 22,021 bytes, versus
23,280 original. This accepts module sixteen for this scene only.
Five BASIC modules remain original: main, mod_tex, model, screen, pl_move.
The separate baseline face-oracle failure remains unresolved.

The missing byte read is restored at original 136d (emitted 159b):

```asm
; faulty candidate               ; fixed
mov ax,[bp-68h]                  mov dl,[es:si]
mov cx,ax                        and dx,0FFh
and cx,0FFh
```

### R_BSP diagnosis history (rejected candidate)

The adjacent r01-algebraic -> r01-dead dumps locate a dangling definition:
PEEK's byte loads at 136d and 13d9 are deleted while the following AND 255
still reads their results. Liveness treated an explicit mask input as
preservation-only because it also appeared in `merges`. It now follows
the explicit read as well as any preserved upper portion. The focused
regression fails first on the deleted load; all 11 dead-ownership and byte
recognition tests pass (1.36 seconds). Native recheck above passes; full
dumps are in `/tmp/qbopt-r-bsp-native-live-mask`.

`/tmp/qbopt-qrender-native-rbsp.geOVi5` links and returns to the DOS
prompt (port 2220), but its **13.39483 FPS is not a speedup**: polygons
266 -> 103, triangles 820 -> 323, PVS count 55 -> 167 and portal culls
11 -> 147. It runs 15 frames / 11 timed samples instead of 13 / 9.
BENCH.BMP differs (SHA1 `f9b26ea9f9636b4e18acffcccc893228f2d1248d`);
fresh screenshot `bench-native-019.png` was inspected. Keep the fifteen
module build below. Next isolate visibility/culling in adjacent MIR dumps
`/tmp/qbopt-r-bsp-native` (s00..s91). R_BSP is 23,280 -> 22,042 bytes.

Input-only project bounds are in `/tmp/qbopt-r-bsp-audit-20260911.py`,
hash-checked against the original objects. R_RECURSIVE_WORLD_NODE begins
with SUB SP,48h at 0607; R_PORTAL_MARK with SUB SP,62h at 0348;
R_PORTAL_DRAW with SUB SP,84h at 0926. QGLARMAP and QGLARLOADBAS
begin with ADD SP at 010f and 03ea. These kill incoming arithmetic flags.
MOD_LOAD_FLAT, MOD_PVS_PAGE, SYS_NOW and SYS_ERROR enter the previously
audited B$ENRA. All six GP inputs remain, with unknown cleanup, memory,
clobbers and control effects. No compiler source fix or new preservation
claim was made for this candidate.

Plane-distance address computation, decoded source versus native output
(source emulator instructions displayed as their x87 equivalents):

```asm
; before                         ; candidate, not accepted
mov ax,[bp+8]                    mov di,si ; retained plane pointer
add ax,8                         add di,8
mov si,ax
fmul dword [si]                  fmul dword [di]
```

### Accepted fifteen-module evidence

After `d546d5f`, `/tmp/qbopt-qrender-native-dsurf-extract.JlcBA1`
links and returns to the DOS prompt (port 2219). **9.49704 FPS** versus
baseline **9.43334**, over 13 frames / 9 timed samples; no speedup claim.
Every non-timing/non-memory field matches baseline: `sc_test=1`,
`lm_fallback=0`, `sc_made=21`. BENCH.BMP is byte-identical, SHA1
`d6e4096b3610249ff4d53b6829f1ab18ec108c7a`. Fresh screenshot
`bench-native-022.png` was inspected. D_SURF: 39,734 -> 36,196 bytes.
This accepts the fifteenth module for this scene, not all renderer behavior.
Six BASIC modules remain original: main, mod_tex, model, r_bsp, screen,
pl_move. The separate baseline QGLFACE oracle failure remains unresolved.

Actual allocated descriptor write, before and after the extraction fix:

```asm
; before                         ; after
                                 mov si,[bp-6Ch]
                                 mov es,[si+2]
mov [es:bx],eax                  mov [es:bx],eax
```

### D_SURF diagnosis history (failed candidates below)

The remaining descriptor write at 1833 loses its selector **during raise**:
182a loads ES, 182d loads a long, then the synthesized EXTRACT at 1830 has
no machine node. Address recognition treated that missing node as an
unknown segment clobber. The store consequently reached lowering without
any selector requirement. Synthesized value extraction now preserves the
selector; unknown calls still end it. The focused regression failed on the
missing ES input requirement before the fix; both address-dependency checks
pass (0.33 seconds), as do all 17 constraint tests (0.45 seconds).
Native rebuild dumps: `/tmp/qbopt-d-surf-native-extract-selector`.
The native runtime recheck above establishes the repaired descriptor store.

Latest native-x87 recheck after selector liveness fix (`2a209e5`):
`/tmp/qbopt-qrender-native-dsurf-selector.rYLLPB`, port 2218.
LINK succeeds and the guest returns to the DOS prompt. **SC_SELFTEST now
passes: `sc_test=1`, matching baseline.** D_SURF emits 36,193 bytes.
However, this candidate is still not accepted: `lm_fallback=11` rather than
0; `sc_built=447` rather than 469; `sc_dlit=204` rather than 214;
`sc_made=163` rather than 21; `sc_worst=233` rather than 245.
At 13 frames / 9 timed samples it reports **8.61702 FPS**, baseline
**9.43334**. BENCH.BMP remains different, SHA1
`fc890f87e7045ebbabd754e3dae030e591edc3a3`.
Fresh screenshot `bench-native-033.png` was inspected. The fourteen-module
build remains the last accepted benchmark. Next isolate the lightmap/cache
build fallback using adjacent pass dumps; the LRU self-test is no longer
the failing symptom.

Actual allocation at original 241d, from
`/tmp/qbopt-d-surf-native-selector-live/s103-lir-regalloc.txt`:

```asm
; faulty candidate               ; selector liveness fixed
mov ax,[es:bx]                    mov es,[bp-0F4h]
                                 mov ax,[es:bx]
```

The next adjacent-dump check identifies a separate lost dependency:
forward replaces the selector load at 241a with the value from 23dc,
correctly retaining that value in the far reference. Lowering discarded the
reference's selector dependency, so after 23ec/23fc changed ES, the read at
241d used the wrong array's segment. Lowering now requires each far-memory
operation's selector in ES; spilling or forwarding must restore that value
at its consumer. The new emitted-instruction regression fails with the fix
removed; all 16 constraint tests pass (0.95 seconds). Native recheck pending.

The first selector-input patch missed recognized instruction sites: their
early return supplied only the original address-register requirements.
Two emitted candidates stayed byte-identical to the ES-write candidate;
neither was benchmarked again. Inspecting the real raised references proved
the selector metadata was present. A second fail-first case exercises the
recognized-site route. Selector inputs now compose with every existing
input requirement rather than sitting after its early returns; all 17
constraint tests pass (0.56 seconds). Rebuild dumps:
`/tmp/qbopt-d-surf-native-selector-sites`.

The bounded lowerer probe at 241d then proved the requirement was present
(`v1113 -> ES`) but absent from LIR's `uses`. Operand-derived liveness had
dropped the implicit selector even though MIR still carried it. Lowering
now includes all required inputs in the instruction's use list. Both
ordinary and recognized-site regressions fail first on the missing live
selector; all 17 constraint tests pass (0.39 seconds). Recheck dumps:
`/tmp/qbopt-d-surf-native-selector-live`. No runtime acceptance is inferred
from these host checks.

```asm
; before (ES holds another array) ; required selector restoration
mov ax,[es:bx]                    mov es,cx ; saved selector
                                 mov ax,[es:bx]
```

Recheck after ES-write fix (`21e67d7`):
`/tmp/qbopt-qrender-native-dsurf-es.ircB1e`, port 2217, reports **8.69943 FPS**.
It links successfully and returns to the DOS prompt, but `sc_test=-4000`,
all six previously differing non-timing fields, and the differing BMP hash
are unchanged. Fresh `bench-native-034.png` is the screenshot evidence.
The emitted OBJ is 36,054 bytes. Dumps in `/tmp/qbopt-d-surf-native-es`
confirm 0fcc now writes ES before spilling it. This fixes an independently
reproduced backend defect, **not the remaining cache failure**.

`/tmp/qbopt-qrender-native-dsurf.vw3Xem` links without errors and exits to
the DOS prompt, but the fifteen-module native-x87 candidate is **incorrect**.
FPS is **8.61702** versus baseline **9.43334**; `sc_test=-4000` versus `1`,
`lm_fallback=11` versus `0`, and cache counters differ. BENCH.BMP differs
(SHA1 `fc890f87e7045ebbabd754e3dae030e591edc3a3`). Fresh
`bench-native-022.png` captures the rendered scene. The fourteen-module build
below remains the last accepted benchmark.

The pinned source decodes -4000 precisely: after allocating three cache
blocks and touching face 0, SC_SELFTEST expects face 1 at the LRU head but
observes face 0. Next isolate SC_FIND's list update versus the caller's
nested-array read, using `/tmp/qbopt-d-surf-native-shifts` (s00..s107).
Do not explain this away as timing variation or accept the fallback.

The dumps expose a backend defect in SC_LRU_UNLINK: the segment load at
original 0fcc becomes an ordinary spilled GP value, while its following far
load still uses physical ES. A body-wide pin did not survive the spiller's
fresh value. Lowering now attaches an instruction-level ES output requirement.
The focused forced-spill regression fails first and decodes an ES destination
after the fix; all 15 constraint tests pass (0.31 seconds). The runtime
recheck above shows that the remaining cache failure has another cause.

```asm
; faulty candidate               ; required after spilling
mov dx,[si+2]                    mov es,[si+2]
mov [bp-26h],dx                  mov [bp-26h],es
mov dx,[es:di]                   mov dx,[es:di]
```

The candidate shrinks D_SURF from 39,734 to 36,045 bytes. At original 183a,
the formerly unencodable hoisted shift now emits at 1d5c..1d5f:

```asm
; before: whole-object refusal   ; candidate (not correctness-approved)
; cidx << 2 had no mnemonic      mov ax,[bp-1Ah]
                                shl ax,2
```

### Interface and lowering fixes

Four real D_SURF calls failed lowering before their VBDOS input contracts
were established: DSG0 at 211c, PUT3 at 2ceb, SMID at 2d5d and SPAC at 2d11.
DSG0 only stores DS and returns. PUT3 enters GET3's shared file path;
LocateFDB kills incoming arithmetic flags. SMID first enters strutil 0013,
whose OR AX,AX kills them; SPAC enters strfcn 0191, whose OR CX,CX does so.
All GP inputs remain conservative, as do cleanup, memory, clobbers and
control/error effects. No runtime operation is replaced.

All four real-object regressions failed first. The eight selected D_MDL and
D_SURF call checks pass (131.91 seconds); no broad suite was run. Native-x87
emission using `/tmp/qbopt-d-surf-audit.py` refused atomically; the 39,734-byte
object was unchanged. Complete dumps s00..s107 in `/tmp/qbopt-d-surf-native`
pinpoint the next defect: a hoisted `cidx << 2` at 183a is valid MIR but
lowering gives it an empty mnemonic. The adjacent lowered view shows the
valid increment at the same address, followed by this unnamed shift.
Lowering now maps SHL/SHR/SAR explicitly; six fail-first regressions decode
the emitted 16/32-bit instructions and check their operands. All 27 focused
lowering checks pass in 0.38 seconds. Project bindings remain hash-scoped.
This is **not yet an accepted fifteenth module**; the latest
verified FPS and screenshot remain the fourteen-module evidence below.

Before/after this interface-only change (call instruction retained; final
relocated address is not yet established):

```asm
; before                         ; after
call far B$PUT3                   call far B$PUT3
call far B$SMID                   call far B$SMID
call far B$SPAC                   call far B$SPAC
```

The shift fix's focused emitted-code check (EAX is the test allocation, not
a claim about the renderer's final assignment):

```asm
; before: emission refused       ; after: C1 E0 02, prefixed 66 for EAX
; unnamed cidx << 2              shl eax,2
```

## Latest benchmark — fourteen modules, native x87, 2026-09-11

D_MDL now passes the pinned benchmark on top of the thirteen-module build.
`/tmp/qbopt-qrender-native-dmdl.yOiEO3` (port 2215) reports **9.49864 FPS**
versus baseline **9.43334**. All checked non-timing/non-memory fields match,
BENCH.BMP is byte-identical, and fresh `bench-native-025.png` shows the
scene. The debugger observes the DOS prompt after completion. No speedup
claimed within the observed variation.

FLOF, GET3, GET4 and SACT were the remaining unaudited runtime interfaces.
Actual VBDCL10E entries establish where incoming arithmetic flags are first
overwritten: FLOF and GET3 enter LocateFDB (XOR SI,SI); GET4 starts record
validation with OR CX,CX; SACT starts with a descriptor-length CMP. All GP
inputs remain live; cleanup, memory, preservation and transitive control/error
effects remain unknown. No file or string operation is replaced. Four real
D_MDL call regressions fail before the fix; all 34 selected file-contract
checks pass in 3.44 seconds, including existing conservative-effect checks.

The project bindings in `/tmp/qbopt-d-mdl-audit.py` are hash-scoped, not
global declarations. D_MDL shrinks from 15,539 to 15,134 bytes. Full dumps:
`/tmp/qbopt-d-mdl-native`. Existing long-comparison absorption can now emit
the file-length comparison (allocator saves omitted; DX:AX is copied into
BX:SI before the shown native packing):

```asm
; original                      ; emitted comparison
push dx                         push bx
push ax                         push si
push dword 30h                  pop ebx
call far B$CPI4                 cmp ebx,30h
jge enough                      jge enough
```

Seven BASIC modules remain original: main, d_surf, mod_tex, model, r_bsp,
screen, pl_move. The single-scene limitation and face-oracle failure remain.

## Previous benchmark — thirteen modules, native x87

H_FRAME passes the same pinned benchmark on top of the twelve-module build.
`/tmp/qbopt-qrender-native-hframe.NlH7qn` (port 2214) reports **9.50707 FPS**
versus baseline **9.43334**. All checked non-timing/non-memory fields match;
BENCH.BMP is byte-identical. Fresh `bench-native-030.png` shows the scene,
and the debugger subsequently observes the DOS prompt. No speedup claimed.

All 25 previously unresolved project calls were located in pinned objects
and scoped by hashes in `/tmp/qbopt-h-frame-audit.py`. BASIC entry prologues
reach the audited ENRA before consuming incoming flags; C/assembly entries
overwrite arithmetic flags in their stack adjustment before dispatch.
The supplied contracts retain all GP inputs and leave cleanup, memory,
preservation and transitive control effects unknown. They are not global
contracts for arbitrary same-named functions. No compiler defect was fixed
in this round and no new host-test loop was run.

H_FRAME shrinks from 21,516 to 20,411 bytes. Complete stage dumps:
`/tmp/qbopt-h-frame-native`. HOST_ADVANCE's actual instruction bytes:

```asm
; before: /FPi protocol                ; after: native x87
db 0CDh,35h,04h    ; fld dword [si]     fld dword [si]       ; D9 04
db 0CDh,34h,46h,20h ; fadd [bp+20h]    fadd dword [bp+20h]  ; D8 46 20
db 0CDh,35h,1Ch    ; fstp dword [si]    fstp dword [si]      ; D9 1C
db 0CDh,3Dh        ; wait              wait                 ; 9B
```

Eight BASIC modules remain original: main, d_mdl, d_surf, mod_tex, model,
r_bsp, screen, pl_move. This single scene does not establish every input
path, and the separate face-oracle failure remains open.

## Previous benchmark — twelve modules, native x87

H_BENCH now passes the pinned benchmark on top of the eleven-module build.
`/tmp/qbopt-qrender-native-benchfix.DyrmCf` (port 2213) completes 13 frames,
9 timed samples, and reports **9.40866 FPS**, versus baseline **9.43334**.
All non-timing/non-memory fields match, including 105.777777777778
triangles/frame; BENCH.BMP is byte-identical. Fresh screenshot
`bench-native-027.png` shows the rendered scene; the debugger observes the
DOS prompt afterwards. No speedup claimed within this variation.

The constraint splitter discarded both input and output requirements when
it inserted an ABI copy. A later spill then lost the register a call's
result actually arrives in. Requirements now remain attached to their
rewritten input/output values, allowing allocation to recover them each
round. The focused regression reproduces a call-input split followed by a
result spill and fails on the emitted store's source before the fix.
All 14 constraint tests and two pinned-return coalescing tests pass.

```asm
; failing candidate             ; corrected candidate
call far B$CHOU                 call far B$CHOU
mov [bp-116h],bx                mov [bp-116h],si
mov [bp-118h],ax                mov [bp-118h],di
```

Stage dumps: `/tmp/qbopt-h-bench-fixed-native`; H_BENCH OBJ is 28,281 bytes
versus 29,337 original. Nine BASIC modules remain original: main, h_frame,
d_mdl, d_surf, mod_tex, model, r_bsp, screen, pl_move. The separate face
oracle failure below remains unresolved; the full project goal is not met.

## Previous candidate — eleven modules, native x87

QGLFACE now lowers with native x87 in addition to the ten modules below.
VBDOS POW8 and POW4 are aliases of the same runtime entry; POW8 inherits
the audited interface, retaining unknown effects and the original call.
Before: lowering refused POW8's unestablished interface. After: the call
remains, followed by native `fistp dword [bp-1Eh]`, `wait`, and a GP load.
The real-object regression failed before the change; four focused tests pass.

Candidate `/tmp/qbopt-qrender-native-face-20260911` completes at **9.50707
FPS** versus baseline **9.43334**, with identical correctness fields and
BENCH.BMP. No speedup claimed. Fresh screenshot `diff-live-009.png` shows
the rendered scene; copied screenshots are not evidence of this run.
Full stage dumps: `/tmp/qbopt-qglface-native`; rewritten OBJ: 12,928 bytes.

The dedicated `-bench 1 -qglface` check is byte-identical to the ten-module
reference's 1,263-byte log, but **both report FAIL: coverage differs from
the oracle**. This establishes no observed optimizer regression, not a
passing face check. The full correctness gate remains open. Dedicated-check
screenshots are black and are not used as rendered-scene evidence.

## Next module — H_BENCH interface audit

The combined project-input audit now emits H_BENCH (29,337 -> 28,281
bytes), but its twelve-module runtime candidate **fails report correctness**.
Build `/tmp/qbopt-qrender-native-bench.m7sMnr`, port 2212, linked without
errors and returned to the DOS prompt. BENCH.BMP matches baseline exactly;
fresh `bench-native-030.png` shows the scene. Nevertheless BENCH.TXT says
`fps_mean 137438953472`, `ft_mean 1.#INF`, `ft_n 0` and
`mtri_per_frame -6.25274070120563D-07` instead of 105.777777777778.
The reported FPS is invalid, not a performance result. Eleven-module
evidence remains the latest accepted benchmark; do not promote this build.

All project summaries are scoped to hash-checked pinned objects in
`/tmp/qbopt-h-bench-audit.py`. BASIC entries reach the audited ENRA before
reading incoming flags; profiling stubs XOR AX/DX and RETF. Only GP input
bounds were added, retaining unknown cleanup and side effects (except the
previously audited SCR_SCREENSHOT and QGLMEMAVAIL cleanup).

Complete dumps: `/tmp/qbopt-h-bench-project-native`. The report's frame-count
pointer is saved at original 01f1 to BP-56h, then used by FIDIV at 0224.
MIR retains that dependency; allocated code instead saves BX after CHOU
where the original consumes SI. This is the next discrepancy to trace,
not yet a proven root cause or fixed regression:

```asm
; original pointer save         ; failing candidate (extra spills omitted)
mov bx,si                       mov [bp-116h],bx ; after CHOU
; ...                           ; ...
mov dx,bx                       mov ax,[bp-116h]
mov [bp-56h],dx                 mov [bp-56h],ax
```

VBDOS STR4/STR8 now have a conservative GP input bound. Actual library
wrappers pass AL=4/8 and BX=BP+6 to STR_COMMON; FOUTBX overwrites incoming
arithmetic flags before dispatch. The dependency audit reaches floating
formatting and temporary-string allocation with unproved paths, so cleanup,
memory, preservation and control effects remain unknown. Audit artifact:
`/tmp/qbopt-str4-contract.json`. No string conversion is replaced.

Both real-object regressions failed first at 01b1/0239; these and two
conservative-contract checks pass (four focused tests, 31.67 seconds).
With the previously audited SCR_SCREENSHOT interface, native emission next
refuses HOST_BENCH_REPORT at 077a, SYS_RDTSC_HZ. Stage files are in
`/tmp/qbopt-h-bench-native`. No new executable or FPS measurement: H_BENCH
remains original, and the whole-module before/after assembly is unchanged.
The dumper displays emulator operations as x87 equivalents:

```asm
; before                      ; after (atomic refusal)
sub sp,4                      sub sp,4
mov bx,sp                     mov bx,sp
fstp dword [bx]               fstp dword [bx]
wait                          wait
call far B$STR4               call far B$STR4
```

## Previous gate — ten modules, native x87

QGLDIFF is now accepted on top of the nine-module build. Candidate
`/tmp/qbopt-qrender-native-forward-20260911` runs on port 2210; its
dedicated QGLDIFF.LOG is byte-identical to the 2,701-byte reference and
ends RESULT PASS. The ordinary benchmark completes at **9.50707 FPS**
versus baseline 9.43334; all checked correctness fields and BENCH.BMP
match. No speedup claimed within this variation. Render evidence:
`diff-live-019.png`; dedicated-check completion: `qgldiff-check.png`.

The second defect was load-provider classification: after constant
folding, ADD of a constant and memory had no non-address SSA input,
so forwarding treated its result as the memory contents. At original
048c it replaced the array base with base+20; the plane computation
subtracted a coordinate from itself. Only a MIR LOAD may now provide
loaded bytes. The real-fixture regression fails before the fix and passes
after; four focused forwarding checks pass. A mistakenly broad test
selection was interrupted after 515 passes, not treated as a completed gate.
Full stages: `/tmp/qbopt-qgldiff-forward-native`; OBJ 9,699 bytes.

```asm
; faulty candidate             ; corrected candidate
fld dword [es:si]              fld dword [es:si]
fsub dword [es:si]             xor si,si
                              add si,[bx+0Ah]
                              fsub dword [es:si]
```

Eleven original modules remain. Prior failed candidates below are diagnostic
history, not accepted results. A stale-config launch and a debugger-port
conflict were excluded; unwatched artifacts were preserved separately.

SYS now emits and links, adding it to the eight accepted native modules.
Build: `/tmp/qbopt-qrender-native-sys-20260911`; SYS OBJ 20,947 -> 20,449
bytes. The same pinned normal-core benchmark completes 13 frames at
**9.50285 FPS**, versus baseline **9.43334 FPS**. This difference is within
observed variation, not evidence of a speedup. All non-timing/non-memory
report fields match; BENCH.BMP is byte-identical (SHA1
`d6e4096b3610249ff4d53b6829f1ab18ec108c7a`). Screenshot `sys-live-005.png`
shows the rendered scene; the debugger subsequently observes the DOS prompt.
The first watcher launch lost its process before attachment; a terminal-owned
launch provided the successful measured run. No FPS was inferred from that failure.

The FISTP defect was in floatalloc: its early return for bodies without
named 80-bit values bypassed conversion of runtime-produced physical ST0.
Integer stores now obtain owned frame storage before named-stack allocation.
Two fail-first physical-ST0 regressions and all 59 floatalloc tests pass.
An additional FP selection check has 11 passes and one pre-existing q-O
dead-conversion assertion failure, reproduced with the committed old allocator.

```asm
; before, SYS_INIT_TABLES
call far B$POW4
call far B$FIST

; after, native SYS (the power helper remains)
call far B$POW4
mov [bp-24h],ax
fistp dword [bp-1Eh]
wait
mov ebx,[bp-1Eh]
```

Full before/after and pass dumps: `/tmp/qbopt-sys-fistp-native`.
The remaining original modules still need incremental native integration;
this is not the full-project correctness gate.

### Next: QGLDIFF and shared rounding interfaces

The duplicate-input fix now gives additional ABI slots independent live
ranges before constraints are keyed by value. Two fail-first BX/CX and
AX/BX/CX regressions pass; all 13 constrain tests and two nearby call-mask/
coalescing checks pass. QGL_DIFF_TRI now emits:

```asm
mov ax,0
mov cx,0
mov bx,0                 ; restored; was missing
call far B$ENRA
```

Candidate `/tmp/qbopt-qrender-native-inputs-20260911` (port 2208) completes
the dedicated check instead of hanging, but reports **RESULT FAIL**:
off-exact is 32 for all six checked cases, versus reference 1/1/1/1/8/4.
Drawn counts and every logged pixel sample match. Next investigate the
QGL_DIFF_WANT/QGL_DIFF_DEV numerical path, comparing each stage; do not
change the oracle or count this module as accepted. Native stage dumps:
`/tmp/qbopt-qgldiff-inputs-native`; OBJ 9,666 bytes. Ordinary benchmark
is 9.40866 FPS, all correctness fields and BMP match baseline. Evidence:
`diff-live-005.png` (rendered scene), `qgldiff-check.png` (returned prompt),
and QGLDIFF.LOG (2,707 bytes versus reference 2,701).

QGLDIFF native emission succeeds (9,663-byte OBJ), but its dedicated gate
**fails to complete**. Candidate `/tmp/qbopt-qrender-native-diff-20260911`
passes the ordinary benchmark at 9.50285 FPS with identical correctness
fields and BMP; screenshot `diff-live-010.png`. That benchmark does not
exercise QGLDIFF and is insufficient to accept it. The nine-module reference
finishes `-qgldiff` with RESULT PASS (2,701-byte QGLDIFF.LOG); the candidate
repeatedly stays in B$FCompactMove/B$MoveFreeBlockBX with an empty log.
Guest port 2207 is paused for inspection; no optimized QGLDIFF acceptance.

The live caller is QGL_DIFF_TRI's B$ENRA. Stage dumps in
`/tmp/qbopt-qgldiff-native` show two zero inputs CSE'd to one value; lowering
correctly requires it in BX and CX, but constrained/allocation output supplies
only CX. `constrain._wanted` keys requirements by value, silently overwriting
one register requirement. The next fix must split distinct input occurrences,
not disable CSE or weaken the runtime contract. Before/after evidence:

```asm
; original entry at 00e2: both runtime inputs are zero
mov cx,0
mov bx,0
call far B$ENRA

; candidate entry at 00d5: BX setup lost (invalid)
mov ax,0
mov cx,0
call far B$ENRA
```

VBDOS INT4/INT8 share `87bint.asm:0016`. The dispatcher at emulator
segment 2 offset 002a kills incoming arithmetic flags with CMP; BX=6
selects the relocated table entry 001c -> 0637. Hardware uses FRNDINT
with saved/restored control; software exception dispatch remains unknown.
The wrapper restores SP/BP and RETF with zero argument cleanup. Only this
interface is established; effects remain conservative and no rounding
operation is substituted. Three fail-first checks pass, including lowering
the real QGLDIFF INT4 call at 0700. Before/after at this call is intentionally
`call far B$INT4` in both cases; the difference is that lowering now accepts it.

Remaining original modules: main, h_bench, h_frame, qgldiff, qglface,
d_mdl, d_surf, mod_tex, model, r_bsp, screen, pl_move. INT4/INT8 appear
across eight of these. QGLDIFF is the next bounded integration candidate:
its surface interfaces were audited for QGLCHK; additional project calls
are QGLCLRECT, QGLDRFILL and QGLRSPOLY. Raw objects show normal RETF
cleanup 8/14/16 respectively; drawing dependencies still require checking
before those summaries are used. No new module run or FPS is claimed here.

### SYS interface audit history

The six remaining VBDOS input interfaces (CSCN, WIDT, SLEP, TIMR,
FRI2, STI4) are now bounded from runtime disassembly and their dependencies.
CSCN has count-dependent cleanup; the other normal-return cleanups are
4, 4, 0, 2, 4 bytes respectively. All memory, clobber and error effects
remain conservative. Real SYS blocks pass lowering; six independent
contract checks fail with these variants removed and pass with them restored.

The next native-emission blocker is **08fa: FISTP to a GP register**:
the final LIR says `ebx := fistp st(0)`, an impossible x87 encoding.
Full stage dumps: `/tmp/qbopt-sys-runtime-native`. Compare the raised
floating store with each subsequent stage to locate where the required
memory destination was lost. SYS is refused atomically: before/after OBJ
assembly is unchanged, and no new runtime run or FPS measurement is claimed.

Earlier interface audit:

POW4 at 08f5 is now bounded: VBDCL10E `87btran.asm` establishes a BP
frame, examines the x87 operands, and overwrites arithmetic flags at 00f3
before dispatch. Normal exits at 0089/0098/00b3/00d8 restore BP and RETF
without consuming caller stack arguments. Error paths tail RUNERR. The
runtime call and all conservative effects remain; this does not replace
power with an algebraic approximation. The actual SYS_INIT_TABLES block
failed strict lowering at 08f5 before the contract and passes now.

Native emission now reaches **SYS_ERROR: CSCN at 0980**. Full dumps are in
`/tmp/qbopt-sys-pow-native`. A complete missing-input inventory of SYS now
contains six names: CSCN, FRI2, SLEP, STI4, TIMR, WIDT. Initial dependency
inspection locates CSCN -> ScSetup/EnsureFI/SCRSTT/ScCleanUpParms; FRI2 ->
heap compaction (decoder reports overlapping instructions, requiring raw
inspection); SLEP -> keyboard/clock interrupt setup; STI4 -> STR_COMMON;
TIMR -> DOS time; WIDT -> EnsureFI/SWIDTH. These are audit leads, not
established contracts. No new executable was accepted or benchmarked.

```asm
; before                         ; after (atomic whole-module refusal)
08f5 call far B$POW4              ; call far B$POW4
08fa call far B$FIST              ; call far B$FIST
```

The allocator refusal is fixed: interval construction merged touching
segments on opposite sides of a definition. At HOST_SHUTDOWN (00c2), a
coalesced SI id was both consumed and newly defined; merging its two
lifetimes made the call's register mask look like a forbidden clobber of
a surviving value. Retaining the touching definition boundary lets it
allocate. A genuinely surviving value is still rejected from a clobbered
register. The focused fail-first regression and two neighboring mask/
coalescing checks pass. Captured allocator input and per-round evidence:
`/tmp/qbopt-sys-allocation`; new full dumps:
`/tmp/qbopt-sys-intervals-native`.

SYS_PARSE_ARGS now allocates (1072 LIR instructions); the call at 00c2
receives AX/BX/CX/DX/SI/DI without the failing reload/store around SI.
Native whole-module emission advances to **SYS_INIT_TABLES, POW4 at 08f5**.
No new scene run: the object still refuses atomically, so its actual ASM
before/after remains identical:

```asm
; before                         ; after (atomic whole-module refusal)
00bd call far B$PESD              ; call far B$PESD
00c2 call far HOST_SHUTDOWN        ; call far HOST_SHUTDOWN
```

FDR1 at 0823 is now bounded too. VBDCL10E `dkdir.asm` sets the DOS DTA,
tests search mode, then calls RefStringArgLast, GET_PATHNAME, DelTempSH,
DOS find-first/find-next, GetZStrLen and StrAlcTmpCopy. The shared exit
restores DI/SI/BP and selects RETF versus RETF 2 using saved mode. Cleanup
remains unknown; neither helper side effects nor fixed mode across every
dependency are assumed. Register inputs are conservatively all six GP
registers. The real parser-block regression failed at 0823 before this
interface and passes now; both LCAS and FDR1 checks pass.

Full native emission in `/tmp/qbopt-sys-fdr1-native` now reaches allocation:
`Unplaced: value#1591 cannot be spilled and no register is free for it`.
That is the next blocker. Atomic refusal still preserves SYS exactly:

```asm
; before                         ; after (atomic refusal)
0822 push ax                     ; push ax
0823 call far B$FDR1              ; call far B$FDR1
```

No new scene was run; the accepted eight-module FPS/screenshot below remain
the latest runtime evidence, not evidence for optimized SYS.

`sys.obj` initially refused HOST_SHUTDOWN at 00c2. Explicit project-interface
audit (`/tmp/qbopt-sys-audit.py`) established normal-return cleanup from the
original callees: HOST_SHUTDOWN 0 (MAIN 1801; normally terminates via CEND),
COM_TOKENIZE 8 (COMMON 021f), COM_PARSE_CONFIG 4 (COMMON 0830), KBD/MOUSE/
TMR/VGA/MEM shutdown 0 (00b8/016f/011d/00ef/0270), TMRINIT 2 (00cf),
TMRTICKS/TMRCYCLES 0 (00dd/00f1), MEMAVAIL 2 (01ff/0208). Interrupt
continuations were read directly; MEMAVAIL's indirect dependency remains
unknown. All six GP inputs and unknown register/memory/control/error effects
are retained. These are hash-checked project inputs, not global ABI rules.

Next was LCAS at 012f. The VBDCL10E implementation passes its descriptor to
RefStringArgLast; RefString overwrites arithmetic flags before branching.
The indirect character-conversion target is **B$ToLower**, proved by the
01e9 relocation, and PUSH CS / near CALL pairs with its RETF. Both empty
and nonempty paths restore their local stack then RETF 2. Only this bounded
interface is added; no string allocation, alias or preservation claims.

`fixtures/regressions/qrender-sys-v-g3.obj` is the original BC output.
Its actual argument-parser block failed strict lowering at 012f without
the contract and passes with it (one focused regression). Full native SYS
emission now reaches **FDR1 at 0823**, still refused atomically. Dumps:
`/tmp/qbopt-sys-contracts-native` before, `/tmp/qbopt-sys-lcas-native` after.
No new executable/scene run, FPS or screenshot is claimed for SYS.

```asm
; before                         ; after (whole-module refusal, unchanged)
012b push bx                     ; push bx
012c mov [bp-36h],ax              ; mov [bp-36h],ax
012f call far B$LCAS              ; call far B$LCAS
0134 push ax                     ; push ax
```

### Accepted eight-module scene

`/tmp/qbopt-qrender-native-es-20260911` passes the 60-tick scene: identical
BENCH.BMP (SHA-1 `d6e4096b3610249ff4d53b6829f1ab18ec108c7a`) and all
non-timing/non-memory report fields match baseline, including `plat_zofs -288`.
Mean FPS **9.40866**, frame **106.28508 ms**, best/worst **9.80198/7.5**;
baseline 9.43334, previous eight-module emulator-protocol run 9.34942.
This is within observed short-run variation, not evidence of a speedup.
Live rendered screenshot `live-028.png` and DOS `completion.png` are saved;
the rendered screenshot was visually inspected. LINK reported no errors and
the program exited with code 0. Thirteen BASIC modules remain unrewritten.

The native platform mismatch below was the lost ES prefix, confirmed from
the live emulator-patched bytes, not inferred from FPS or the matching frame.
VBDOS/FIDRQQ's INT 3Ch becomes `90 26 <ESC> <operand>` in VBDCL10E.
The module-aware frontend now restores ES before building MIR; generic
byte-only decoding does not guess other runtime dialects. Emulator rewrapping
also preserves the recovered ES form and adjusts relocation-field offsets.

```asm
; before: VBDOS object, original ent offsets
0729: cd 3c d9 07       ; emulator-prefixed fld es:[bx]
0744: cd 3c d9 1f       ; emulator-prefixed fstp es:[bx]
; after: native output (allocator chose SI for the load)
07a7: fld  dword [es:si]
07aa: fchs
; ... destination address and ES loaded ...
07c1: fstp dword [es:bx]
07c4: wait
```

The regression first failed because the decoded load had no segment prefix;
it now checks both affected real-object accesses and their native encodings,
plus lossless emulator rewrapping. Seven focused checks pass. Every MIR stage
and emitted assembly is retained under each module's `*-stages` directory.
The first changed stage now has a concrete far-memory load/store at
0x729/0x744 instead of `[?]`; no optimizer pass gained machine-specific logic.

### Entity conversion fix and native-FPU transition — 2026-09-11

Commit `75db48d` fixes `ent` refusing `ENT_CHECK_TELEPORT` at 0xae9.
Adjacent dumps in `/tmp/qbopt-ent-audited` showed recognition of B$FIS2
creating `fstore signed16` followed by EXTRACT(word, 0). Lowering supports
extracting halves of a long, not extracting a word from the same word.
The raise now uses COPY for the whole signed16 value; long results still
use two extractions. Actual `ent.obj` is retained as
`fixtures/regressions/qrender-ent-v-g3.obj`. Its regression failed at the
observed unsupported extraction before the fix and now lowers to word
FISTP without a FIS2 call. Three existing long-conversion emission tests
also pass. Full corrected `ent` emission succeeds (17843 OBJ bytes), with
dumps in `/tmp/qbopt-ent-word-fixed`.

```asm
; before, original 0xae9
call far B$FIS2
push ax
; after, conversion site in the emitted listing
fistp word [bp-32h]
; allocation supplies the converted value to its later use
```

The subsequent user instruction switches optimized builds to native x87:
`native_fpu=True`, without `basic_semantics`. Re-emit selected modules from
the original objects, not from previously optimized output. The remaining
unrewritten modules keep their original FPi code; unresolved segmented
emulator protocols must not be described as proven native conversions.
`/tmp/qbopt-native-eight.py` batches the eight selected modules with stage
dumps and hash-checked, explicit project contracts. Native runtime results
are a separate comparison from the earlier emulator-protocol runs.

All eight native emissions succeeded in `/tmp/qbopt-qrender-native-20260911`:
d_turb, view, common, in_main, vid, qglchk, qglarr, ent. Mapped output code
has no remaining INT 34h..3Dh sites. Linking and the 60-tick scene complete,
but **the native build is not accepted**: `plat_zofs` is 0 versus baseline
-288. All other non-timing/non-memory fields and the BMP agree. The earlier
eight-module emulator-protocol run retained -288, so investigate native
conversion rather than accepting the matching screenshot as correctness.
Native mean FPS 9.52402 (frame 104.99767 ms, best/worst 9.90121/7.63376);
eight-module emulator-protocol mean 9.34942; baseline 9.43334. These are not
valid optimization gains while state differs. Live native captures caught
startup black (`live-render.png`) and completion (`live-scene.png`), not a
rendered frame; do not label those as visual rendering proof. The fresh
BENCH.BMP is the frame evidence. Investigate segmented emulator conversion:
the decoder removes INT 3Ch without recovering its segment override, while
the native emission path can select the resulting unprefixed operation.
This was the initial hypothesis; the corrected native gate above proves it.

Added `qglarr`; fourteen BASIC modules remain. Build directory:
`/tmp/qbopt-qrender-array-20260910`. The same 60-tick scene linked and
completed with identical BMP and all non-timing/non-memory fields.
Mean FPS **9.41860**, frame time **106.17285 ms**, best/worst
**9.80198/7.55725 FPS**; last six-module repeat 9.56811, baseline 9.43334.
The short-run difference is within the variation already observed on the
unchanged six-module build; no speed claim. Live render and DOS completion
captures were saved and inspected as `live-render.png` / `live-complete.png`.

The scene does not exercise the array checker. Both optimized and original
executables therefore also ran `dm3ish.bsp -qglarr` on port 2202.
`QGLARR-OPT.LOG` and `QGLARR.LOG` are byte-identical: seven probes below the
window, nine at/above, zero mismatches for hand-filled and qglArLoad paths;
last record 3820 checked, final `RESULT PASS`. Live completion captures:
`array-check.png` / `array-baseline-check.png`. A prompt screenshot proves
return to DOS, not the verdict; the logs carry the verdict.

Explicit external contracts retain all six GP inputs and all unknown
effects. Normal cleanup (QGL prefix omitted): ARFREE/ARHANDLE/ARPAGES/
ARPERPG 4, ARLOADBAS 12, ARMAP/ARNEW 10, FILECLOSE/FILEOPENBAS/FILESIZE 2,
FILEREAD 10, GEMMAP 6. Frame allocation or the initial slot-validation
dependency overwrites arithmetic flags before other work. AR's validator
at 0000 and FILE's at 004c use DEC/CMP before branching. GEMMAP starts with
CMP; its EMS-interrupt continuation and FILECLOSE's DOS-interrupt
continuation were also checked in `src/qgl/{gem,file}.asm`, since the
decoder stops at those interrupts. No dependency preservation is inferred.

Hash-checked harness `/tmp/qbopt-qglarr-audit.py`; dumps
`/tmp/qbopt-qglarr-audited`; original `/tmp/qbopt-qglarr-before.asm`.
Object SHA-256 identities (GEM is recorded below):

```
ar    09fd7e9c41a37d9375ddbbdbe1449ab162c26455351c70dafbc58c3c8e68eb7e
file  111b78e083dc2a2ec8dcf7f64daa05019000bbe77c87d6ffaca3eb2ffbcd9412
```

```asm
; before: signed file size > 0
push dword [bp-1Ah]
push dword 0
call far B$CPI4
jg ...
; after
mov eax,[bp-1Ah]
test eax,eax
jg ...
```

Local call elimination does not offset all allocation/layout overhead:
code grows 4312→5122 bytes; OBJ shrinks 14166→13181, not an instruction-cost
measure. No new compiler fix or optimization pass was introduced this round.

## Six-module gate — 2026-09-10

Screenshot-evidence repeat: preserved the previous BENCH files as
`BENCH.BMP.pre-screenshot` / `BENCH.TXT.pre-screenshot`, then reran the
same six-module executable and scene. Live captures were saved and visually
inspected: `/tmp/qbopt-qrender-check-20260910/live-render.png` shows the
textured scene, and `live-complete.png` shows return to DOS. Fresh BMP and
non-timing/non-memory fields still match baseline. Mean FPS **9.56811**,
frame time **104.51389 ms**, best/worst **9.95757/7.67721**. The same build
previously measured 9.41860 FPS: do not attribute that difference to a code
change. Capture live rendering and completion evidence on subsequent runs.

Added `qglchk` to the five-module build below, using explicit audited
project contracts. `/tmp/qbopt-qrender-check-20260910` linked successfully
and completed the same scene: BMP and all non-timing/non-memory fields
match baseline. Mean FPS **9.41860**, mean frame **106.17285 ms**, best/worst
**9.80198/7.55725 FPS**; previous run 9.56811, baseline 9.43334.
This is -1.56% against the previous run and -0.16% against baseline; nine
timed samples do not establish a performance regression or gain.

The scene does not call `qglCheckAll`. Therefore both this executable and
the untouched baseline were also run with `dm3ish.bsp -qglcheck` on port
2201, observing return to the DOS prompt. Their fresh logs are identical:

```
   ok   cmem surface round trip
   ok   ems  surface round trip
RESULT PASS
```

Use the map argument: the parser treats the first token as the map name,
so `qrender.exe -qglcheck` alone does not select the check. Artifacts:
`QGLCHK-OPT.LOG` (optimized) and `QGLCHK.LOG` (baseline) in the check build.
Six modules now have runtime evidence; fifteen remain.

External interfaces retain all six GP inputs and every unknown effect.
Normal-return cleanup: GEMFRAME 0, MEMAVAIL 2, SFINIT 0, SFFREE 4,
SFNEW 6, SFPGET 8, SFPSET 10, SFRDROW 6 (all names prefixed `QGL`).
GEMFRAME is MOV/RETF with no incoming-flag use; the others overwrite
arithmetic flags before dependent work. SFRDROW delegates to ACCESSRD in
qgldc, whose CMP at 0013 precedes indirect driver dispatch. Indirect calls
do not establish preservation. Entrypoint audits: gem 0050; mem 01ea;
sf code segment 1: 0054/0128/0157/0454/04c4, segment 3: 0008.
Hash-checked harness `/tmp/qbopt-qglchk-audit.py`, dumps
`/tmp/qbopt-qglchk-audited`, original ASM `/tmp/qbopt-qglchk-before.asm`.
Additional object SHA-256 identities (SF is listed below):

```
gem    e1bc70f97466c0ea2bbc5552042824dcc375f9fc4b03b60415cb696516d8b97c
mem    c18eeeb883dcb9ef480b166d5048f27fe4244189e19f717d65187e4205ccd7f8
qgldc  c970873e01f4186daf70eb172ffded78a5ae1f712767d5fb98d503e6190ff07b
```

```asm
; before: arguments to QGLSFNEW
push 40h
push 8
push word [bp+0Ah]
call far QGLSFNEW
; after: identical argument bytes, one combined push
push dword 00400008h
push word [bp+0Ah]
call far QGLSFNEW
```

This example reduces instruction count, not encoded size (combined push
is six bytes versus four). Allocation/spill overhead remains: code grows
1000→1268 bytes; whole OBJ shrinks 5086→4711. No compiler fix or new pass
was needed in this round; do not count the OBJ shrink as an optimization.

## Five-module gate — 2026-09-10

`common`, `view`, `d_turb`, `in_main` and now `vid` emit through LIR, link
and complete the baseline scene together. Sixteen BASIC modules remain.
Build: `/tmp/qbopt-qrender-video-20260910`; original inputs remain in the
baseline directory. LINK.OUT has no linker errors. The live guest on port
2200 was observed in `R_PORTAL_DRAW`, then back at the DOS prompt. Copied
BENCH files were renamed before launch; the new files are from this run.

| Metric | Baseline | Four modules | Five modules |
|---|---:|---:|---:|
| Mean FPS | 9.43334 | 9.56811 | 9.56811 |
| Mean frame time (ms) | 106.00695 | 104.51389 | 104.51389 |
| Best / worst FPS | 9.81732 / 7.56908 | 9.95757 / 7.67721 | 9.95757 / 7.67721 |

Same normal core, 75000 cycles and scene arguments as baseline: 60 ticks,
13 frames, nine timed samples. Five-module FPS is +1.43% against baseline
and unchanged against four modules; no measurable speed gain from `vid`.
BMP SHA-1 remains `d6e4096b3610249ff4d53b6829f1ab18ec108c7a`.
All fields excluding `ft_*`, `fps_*`, `pt_*`, `mem`, `rdtsc_hz`, `tick_hz`
match baseline exactly. This short scene does not verify all video modes,
error paths or the full renderer.

### Video project-call audit

These are explicit external contracts, not global ABI entries. All six GP
inputs and unknown clobber, memory, control and error effects are retained.
Cleanup applies only to normal return. Audited entry/exit offsets:

| Object | Symbol | Entry | Normal RETF cleanup |
|---|---|---:|---:|
| dr | QGLDRBLITSCL | 048d | 16 at 06a4 |
| dr | QGLDRFILL | 0097 | 14 at 0108 |
| vga | QGLVGAINIT | 0000 | 0 |
| vga | QGLVGASCREEN | 00f0 | 0 at 00fa |
| sf | QGLSFNEW | 0054 | 6 at 009a/00a9 |
| screen | SCR_PAL_INSTALL | 1901 | 0 at 1a31 |
| screen | SCR_SBAR_LOAD | 1a37 | 0 at 1d11 |
| sys | SYS_ERROR | 0927 | 2 at 09b1 |

DR and SF entries overwrite arithmetic flags with their frame allocation
before calls. VGAINIT starts with CMP; its decoder stops at BIOS interrupts,
so the continuation was checked against `src/qgl/vga.asm`. VGASCREEN calls
VgaShape, whose first dependency SfInit starts with XOR SI,SI before its
indirect driver calls. BASIC entries start with the previously audited
ENRA. None of this establishes driver preservation or ordinary return from
SYS_ERROR. Object SHA-256 identities (baseline build):

```
dr      6ffd10d26f0a48e60cac1bce96240c8c227b44f5a1828780dd9ad468edf75d1d
vga     201a348a8f4cdc9f8ac904582ed22cd52182fc7985fe74d1d68c0e331dd16d3c
sf      38cd797456f8ec113ec3582a2fe2884d7716322128c0096bee0dc0f02e7ba2c4
screen  598494eae3e667d6ca043c089d873c133e23b61445dffc4975ec3a7f7641c779
sys     359c02ee944e792e9181333c5acec5ba104d40a5c38512150cb562536e0aad53
```

Audit/emission harness: `/tmp/qbopt-vid-audit.py` checks these hashes before
passing the contracts to `wholeseg.emitted`. Stage dumps:
`/tmp/qbopt-vid-audited`; emitted object `/tmp/qbopt-vid-audited.obj`.
Original ASM: `/tmp/qbopt-vid-before.asm`. Example before/after:

```asm
; before: test the stored function result
mov [bp-16h],ax
cmp word [bp-16h],0
jne ...
; after: test the value already held
mov [bp-16h],ax
test ax,ax
jne ...

; before: address argument through a temporary
mov ax,[bp+6]
add ax,1126h
mov si,ax
push dword [si]
; after
add bx,1126h             ; BX already holds [bp+6]
push dword [bx]
```

These local improvements are not a net code-size win: the code segment
grows from 763 to 858 bytes, including allocation moves, a spill and layout
changes. The whole OBJ shrinks from 9461 to 9034 bytes; object size is not
an instruction-cost measurement. No new compiler defect was fixed in this
round; this is an integration/audit milestone, not a new optimization pass.

## Baseline — 2026-09-10

Fresh build: `/tmp/qbopt-qrender-baseline-20260910`, with default VBDOS
`/O /FPi /R /G3 /E /Zi`, C and assembly objects, and CodeView linking.
Build and link succeeded. On dm3ish with `-lm -nostats -yaw 183 -bench 40
-ticks 60`, normal core, 75000 cycles: 60 ticks, 13 rendered frames,
266 polygons, 820 triangles; saved frame inspected. This is a correctness
baseline, not a timing comparison.

All 21 BASIC modules were checked through `wholeseg.emitted`. None completed
LIR emission. Nineteen refused; `pl_move` and `screen` initially failed MIR
convergence. Commit `000860a` fixes the repeated LCSSA exit phis; those two
now finish optimization and refuse on call contracts too. Original objects
remain intact. The subsequent VBDOS sine-interface audit allows `d_turb`
to emit through LIR; the other 20 modules remain blocked.

With only `d_turb.obj` replaced, the renderer relinks and exits successfully
after the same 60-tick run. Its saved BMP is byte-identical to the baseline
(SHA-1 `d6e4096b3610249ff4d53b6829f1ab18ec108c7a`); frame, polygon, triangle,
entity and player-state outputs agree. Timing fields differ, and available
memory is 32 bytes lower. This is a partial correctness check, not completion
of the integration gate or evidence of a speedup.

`fixtures/regressions/qrender-dturb-v-g3.obj` is the real `d_turb.obj` from
the source revision and compiler flags above. Its fail-first regression
requires LIR emission. The audited `B$SIN4`/`B$SIN8` VBDOS interface retains
all six GP inputs and unknown clobbers, memory and control effects; only
zero argument cleanup is established. Hardware and emulator return paths
restore the local stack frame. The sine call itself is not optimized away.

## Remaining blockers

- **VBDOS entry:** nonzero BX at `B$ENRA` calls `B$HFirstAllocBlock`.
  The helper's allocating path reaches `B$HandleAlloc`, heap compaction,
  recursive dependencies and unresolved indirect transfers. The existing
  BX=0 contract does not cover this path. Do not infer register preservation,
  memory purity or pointer stability from its fast path.
- **Other runtime contracts:** first refusals include `B$FLEN`, `B$EXTS`,
  `B$FREF`, `B$ASSN`, and `B$RDIM`.
- **Cross-module interfaces:** BASIC, C and assembly callees need verified
  contracts; examples are `R_POINT_LEAF`, `IN_HANDLE_TOGGLES`, `QGLSFNEW`
  and `QGLSFPSET`. A name alone does not establish an ABI.
- **Resolved backend refusal:** `view`'s inserted add at 0x243 was blocked
  by an unused flags phi. Lowering now prunes dead flag merges before
  checking lifetimes, retaining consumed conditions. The module reaches
  its next blocker, `QGLMOUSEPOS` at 0x280; output is still unchanged.

The VBDOS cosine interface is now audited with the same conservative effects
as sine. `COS4` and `COS8` share entry 0007; hardware returns at 0043 or
00c6, and the `B$EMCOS` emulator tail restores SP/BP and returns at 0064.
With the separately audited `QGLMOUSEPOS` interface supplied, `view` advances
past cosine at 0370 to unknown `CP_ADVANCE` at 069a. The entire object remains
byte-identical on refusal; before/after ASM at the cosine site is therefore
the same `call far B$COS4`. No native cosine substitution is claimed.

Fix each issue with a fail-first regression. Keep code generation refusal
atomic; a byte-identical fallback is not successful optimization. Relink
rewritten BASIC objects separately from the baseline, retaining the same C,
assembly, libraries and assets. Compare fixed-tick frames and simulation
state before measuring speed or resuming the optimization checklist.

## Entry-helper evidence

Audited project interfaces can be supplied through
`wholeseg.emitted(data, external_contracts={symbol: contract})`. One per-site
map feeds both raising and lowering; nothing is installed globally. This is
a trusted API: the caller must verify contracts against the exact linked
objects/libraries, retain their hashes in the audit evidence, and retain
unknown effects conservatively. Analysis-tool JSON is not automatically an
ABI contract. Unspecified symbols retain their existing unknown behavior.

For BASIC `/FPi` objects, use `tools/contracts.py --fp-emulation` explicitly.
It follows the three emulator byte shapes using the frontend decoder, but
keeps their effects unknown and suppresses preservation proofs. Without it,
`PL_MOVE` inspection stopped at the first floating-point interrupt; with it,
the audit reaches 306 instructions, 18 dependencies and `12d9: retf 22h`.
This changes the audit's visibility, not emitted code:

```asm
; before and after: original object bytes unchanged
12d9  retf 22h
```

`CP_ADVANCE` in baseline `main.obj` starts at 1937 with a BX=0 runtime entry,
and ends with `B$EXSA` at 1da4 and `retf 0Ch` at 1da9. Supplying an external
contract that retains every GP input and all unknown effects, with cleanup
12, moves the camera's refusal to `PL_MOVE` at 08ad. These are audit facts,
not globally installed project-symbol contracts.

With PL_MOVE's conservative external interface (cleanup 34) supplied too,
the camera reaches its PRINT calls. VBDOS `B$CHOU` now has an audited minimal
interface: all GP inputs retained, every effect unknown, `retf 2` at 0062.
The float PRINT entries are now covered conservatively as well. Their shared
`B$PRINT` body has three different returns (`retf 8`, `retf 2`, `retf 4`),
but `rt/prnvalfp.asm` defines each scalar argument's width; the VBDOS stubs
set AL=4 for R4 or AL=8 for R8. PRINT saves that type at 003d and reloads it
at 0087 before selecting cleanup. Comma/semicolon R4 and comma/semicolon/EOL
R8 now retain all GP inputs and unknown effects, with cleanup 4 or 8.

With the three audited project interfaces supplied, `V_UPDATE_CAMERA`
passes its interface checks. The next refusal is in `V_OPEN_SCRIPT`:
`B$RDIM` at 09d4. The entire `view.obj` still refuses atomically, so actual
before/after ASM remains identical, including the camera's print call:

```asm
; before                         ; after (atomic refusal)
call far B$PCR4                   call far B$PCR4
```

VBDOS REDIM now receives a per-call cleanup only when a same-block immediate
rank push immediately precedes the descriptor push and call. The shared
`B$ExitDim` path loads `[bp+8]`, clears CH, and removes `6 + 4*rank` bytes.
The rank word's upper byte is not a dimension count. Relocated or unproven
ranks retain unknown contracts; allocation, alias and error effects remain
unknown. `qrender-view-v-g3.obj` is real baseline compiler output from the
revision and flags above, retained for fail-first rank/interface regressions.
With the same external interfaces, the module now passes its first four
REDIM calls and reaches `B$FREF` at 0a23. The object still refuses atomically;
before/after assembly at 09d4 is the unchanged `call far B$RDIM`.

VBDOS `B$FREF` now has cleanup 0 (the complete NextFDB/PpvWalkHeap graph
also agrees), and `B$LDFS` cleanup 6 from its shared normal-return epilogue.
Both retain every GP input and unknown effects. In particular LDFS's
allocator graph is incomplete; no memory or preservation claim follows.
Further audits found variable-count cleanup in `B$CLOS` and stack relocation
in `B$PEOS`; neither may be assigned a fixed cleanup from its bare RETF.
The same camera-module probe now reaches `B$OPEN` at 0a4b. Before/after
assembly remains byte-identical on atomic refusal, including
`call far B$FREF` at 0a23 and `call far B$LDFS` at 0a3f.

`B$OPEN` and `B$DSKI` now retain unknown effects with audited normal-return
cleanup 8 and 2 respectively. Their alternate root branches enter named
error handlers, not different normal-return epilogues. The camera-module
probe advances to `B$PEOS` at 0aa2. This is a different kind of blocker:
PEOS relocates the stack for terminal INPUT (`b$FInput=0`), but skips that
path for disk input. Do not interpret its final bare RETF as universally
zero cleanup.
The object remains unchanged: before/after calls at 0a4b and 0a5a are still
`call far B$OPEN` and `call far B$DSKI` because refusal is atomic.

The PEOS refusal itself only requires a register-input bound, not a cleanup
claim. Its VBDOS entry kills incoming arithmetic flags before any branch or
dependency, so all six GP inputs can be retained conservatively while
cleanup stays unknown. This does not infer an input mode. Frame-depth
analysis already rejects unknown cleanup (`raising_frame.py`); lowering
can retain the original call with constrained GP inputs. All other effects
remain unknown. The regression explicitly forbids a fixed cleanup claim.
The renderer probe now passes both PEOS calls and stops at `B$FEOF` (0b0b).
Actual before/after ASM at 0aa2 remains `call far B$PEOS`, byte-identical
because the whole object still refuses atomically.

File-loop interfaces now include FEOF (normal cleanup 2), CLOSE (cleanup
unknown because its argument count varies), and ERASE (normal cleanup 2),
all with six retained GP inputs and otherwise unknown effects. The loader
passes these calls and reaches V_BEZIER's local-descriptor REDIM at 0bc4.
That call computes the descriptor between pushes; rather than add another
rank-recognition pattern, REDIM now also has a conservative base register
interface with unknown cleanup. The existing per-site rank proof can still
refine cleanup where justified. No register or alias preservation is inferred.
With the same three external interfaces, the full view-module probe now
reaches code emission and refuses `0x0335: fild is not one select.py can emit`.
This replaces the call-interface blocker with an instruction-selection
blocker. The output remains byte-identical; no optimized assembly or runtime
result is claimed for view yet.

The FILD refusal is fixed: integer promotion left a GP value feeding a
physical `st(0)` destination, while float allocation only materialized
integer operands for named floating destinations. The backend now performs
that bridge before either x87 representation is allocated. Two fail-first
regressions cover word and dword inputs; 57 float-allocation tests pass.
Stage dumps show the concrete correction (not final object output):

```asm
; before: impossible operand       ; after: owned frame slot
fild bx                            mov [bp-70h],bx
                                   fild word [bp-70h]
```

The next emission refusal is `fidiv` at 035c, whose indirect address still
contains an unplaced value after register allocation. The view object remains
unchanged atomically; runtime validation awaits a fully emitted module.

The FIDIV base failure came from constraint splitting: it inserted a copy
into SI and renamed the use list, but left the memory operand referring to
the old value. Splitting now renames nested memory bases too. The fail-first
regression runs allocation and checks exact emitted bytes `DE 34`:

```asm
; before: stale abstract base      ; after: assigned base
mov si,ax                          mov si,ax
fidiv word [v398] ; unencodable     fidiv word [si]
```

This fixes operand binding in the backend; no machine detail is introduced
into MIR optimization.
The full-module dump confirms FIDIV is resolved. Emission next refuses at
0941: a `mov` is carrying one fixup but has no encoded relocation field.
The object is still unchanged; inspect fixup ownership before runtime testing.

The 0941 relocation failure is fixed: fallback far-call recognition looked
at an operation's new placement instead of its original node span. Hoisting
the frame load from 0946 to 0941 therefore attached PER4's call relocation
to a `mov`. The emitter now consults the original span. A fail-first real
fixture regression checks both the load's lack of a relocation and the
call's retained target; 14 relocation/emission-order tests pass.

```asm
; before: bad relocation ownership
call far B$PER4
mov cx,[bp+18h]  ; incorrectly claimed PER4 target relocation
; after: only the call owns that relocation
call far B$PER4
mov cx,[bp+18h]
```

The full view module now emits through LIR: 15,470 -> 14,717 object bytes,
using the three separately audited project interfaces. Dumps are in
`/tmp/qbopt-view-reloc-fixed`. This is an emission result only; relinking and
fixed-tick frame/state comparison followed as recorded below.

### Far-string length interface

`common.obj` next refused COM_TOKENIZE at 007e, `B$FLEN`. This is
far-string length, not file length: VBDCL10E's `farstr/stcore.asm`
02b9..02f9 reads the descriptor and may free a temporary through
FreeDataPpv -> FreeHandle. All paths restore BP and return with two bytes
of caller arguments removed. Incoming arithmetic flags are overwritten
before branches or dependencies. The interface now records only that
cleanup and conservatively retains all GP inputs and unknown effects.
The contract regression failed first; all nine focused file/string
interface tests pass. Stage dumps in `/tmp/qbopt-common-next` and
`/tmp/qbopt-common-flen-fixed` move the refusal to EXTS at 008a.

```asm
; before and after: atomic refusal preserves the original object
007e  call far B$FLEN   ; now has an audited interface, not absorbed
; ...
008a  call far B$EXTS   ; next missing interface
```

### Statement-exit interface

VBDOS `B$EXTS` at `rtenexit.asm` 012e..0149 has no dependencies or stack
adjustments. Its initial CMP replaces incoming arithmetic flags; two
branches and the global/frame-state clearing path join RETF. The audited
interface records zero argument cleanup, keeping conservative GP inputs
and unknown side effects. The new regression failed first; 15 focused
entry-interface tests pass. `/tmp/qbopt-common-exts-fixed` now stops at
FMID (00ca), not EXTS (008a). The whole object is still refused unchanged:

```asm
; before                         ; after (same bytes, no fallback success)
008a  call far B$EXTS             008a  call far B$EXTS
; ...                            ; ...
00ca  call far B$FMID             00ca  call far B$FMID
```

### Substring interface

`B$FMID` normal return consumes six argument bytes (VBDOS
`farstr/strfcn.asm` 00f6..0112). Its first dependency, RefString, replaces
incoming arithmetic flags before branching and returns without stack
adjustment. The substring wrapper consumes its two internal words with
RET 4. Allocation/freeing and error paths remain unknown: this is not a
pure substring operation. The fail-first regression and nine neighboring
interface tests pass. `/tmp/qbopt-common-fmid-fixed` advances to ASSN at
00e7, with the entire object still unchanged:

```asm
; before                         ; after (atomic refusal)
00ca  call far B$FMID             00ca  call far B$FMID
; ...                            ; ...
00e7  call far B$ASSN             00e7  call far B$ASSN
```

A direct call-site inventory also finds unresolved ENRA, ERS1, LNIN,
SCMP, SCPF and SYS_ERROR interfaces in `common`. Audit these before
expecting whole-module emission; the list is not proof that no backend
blockers remain.

### Assignment, comparison and copy/free interfaces

Audited VBDOS ASSN, SCMP and SCPF together. Normal argument cleanup is
12, 4 and 2 bytes respectively. ASSN's copy/padding and helper paths join
one epilogue. SCMP reads both strings, preserves its comparison flags
across temporary deletion and then returns. SCPF calls SCPY and STDL,
each consuming one internal argument. Incoming arithmetic flags are
replaced before conditional work, directly or by RefString. All GP inputs
and unknown effects remain; in particular SCMP is not pure because it
may free temporary strings. Three fail-first regressions pass alongside
ten neighboring interface tests.

`/tmp/qbopt-common-strings-fixed` passes COM_TOKENIZE's missing interfaces
and reaches COM_PARSE_CONFIG's nonzero-selector ENRA at 022b. The object
still refuses atomically; the assembly remains identical, for example:

```asm
; before                         ; after (unchanged object)
00e7  call far B$ASSN             00e7  call far B$ASSN
015b  call far B$SCMP             015b  call far B$SCMP
```

### Nonzero-selector entry interface

VBDOS ENRA's nonzero selector no longer requires an unknown register
interface. Its initial XOR overwrites incoming arithmetic flags before
frame construction or allocation; retaining all six GP inputs is safe
without proving allocator preservation. Cleanup, memory, clobber, control
and error effects remain unknown. The proven BX=0 specialization stays
narrower. HFirstAllocBlock and HandleAlloc were inspected; this change
does not claim their allocation/compaction graph is fully understood.

The three fail-first entry cases (nonzero, relocated selector and an entry
at the call) now check conservative inputs and unknown effects rather than
requiring an unknown interface. All 15 entry tests pass. Stage dumps in
`/tmp/qbopt-common-entry-fixed` reach LNIN at 0271. No rewritten object is
accepted yet:

```asm
; before                         ; after (atomic refusal, identical bytes)
022b  call far B$ENRA             022b  call far B$ENRA
; ...                            ; ...
0271  call far B$LNIN             0271  call far B$LNIN
```

The broader runtime-contract test file reports 280 passes and two failures:
VBDOS EVTRAP emits but `handler_entries` finds no handler, and the table
inventory omits programmatically added HARY/LINA contracts. Both failures
reproduce with the new VBDOS ENRA variant removed in-process, so neither
is introduced by this change. Do not dismiss the event failure as stale:
inspect stage dumps and emitted registration before accepting more modules.

### Event-handler discovery correction

EVTRAP's missing handler was a discovery defect, not a lost registration.
`/tmp/qbopt-evtrap-current` shows the MIR retains the handler address and
lowering substitutes a relocated immediate PUSH while retaining MOV AX.
Both relocations in the emitted sequence name handler 0136, whose entry
still reaches ETT1. Discovery now accepts that sequence only when its two
relocations agree and the target is inside the current code segment.

```asm
; BC registration                ; emitted registration
push cs                          push cs
mov ax,handler                   mov ax,handler
push ax                          push word handler
call far B$ONTA                  call far B$ONTA
```

The discovery fix itself changes no emitted bytes. The new real-fixture
regression fails with the previous discovery implementation and passes
with the fix; mismatched relocations are rejected. Both existing PDS and
VBDOS handler-entry regressions also pass. This is structural evidence,
not a new emulator run or proof of all event behavior.

### Contract inventory correction

HARY/LINA were appended to the global contract map in Python after loading
TOML. Their unchanged fields now live in `runtime.toml`; all three compiler
family entries share those loaded contracts. The inventory regression was
observed failing before the move and now also checks shared family entries.
All 283 runtime tests pass, including event-handler discovery. This is a
data-ownership correction: no contract fields or emitted assembly change.

### Whole-library report and line-input interfaces

Reuse `/tmp/qbopt-vbdos-all-contracts.json` for subsequent audits. It contains
2,717 routines from 2,254 public roots, zero unvisited roots and zero
unvisited resolved dependencies. Unknown indirect effects and decoder
limitations remain explicit; coverage is not a proof of every contract.
Reproduce with:

```sh
uv run python tools/contracts.py --all --fp-emulation \
  --lib /Users/alim/work/other/d32x/toolchains/vbdos/lib/VBDCL10E.LIB \
  --dump /tmp/qbopt-vbdos-all-contracts.json > /tmp/qbopt-vbdos-all-contracts.log
```

LNIN's audited normal return is RETF 10; its final InpReset is not PEOS's
frame-relocating epilogue. Device/input effects remain unknown. ERS1's
empty and heap-freeing paths join RETF 2; FreePpv and DeLink were checked.
Both retain conservative GP inputs and effects. Two fail-first regressions
and thirteen neighboring tests pass. `/tmp/qbopt-common-line-fixed` now
reaches the project-local SYS_ERROR at 0645; no runtime interface remains
unknown in the call-site inventory of `common`.

```asm
; before                         ; after (whole object still refused)
0271  call far B$LNIN             0271  call far B$LNIN
; ...                            ; ...
0645  call far SYS_ERROR          0645  call far SYS_ERROR
```

### Common emission and three-module run attempt

SYS_ERROR was inspected in the linked `sys.obj` (SHA256
`359c02ee944e792e9181333c5acec5ba104d40a5c38512150cb562536e0aad53`).
Entry 0927 sets CX/BX then calls ENRA; the normal epilogue at 09b1 is
RETF 2. Source calls shutdown, SLEEP and END, so ordinary return must not
be assumed. An explicit external contract retains all GP inputs and
unknown control/memory/clobber/error effects, recording only normal cleanup.
No global SYS_ERROR entry was added.

With that interface, `common` emits 15,158 -> 14,468 object bytes.
`/tmp/qbopt-common-project-interface` has all stages. One backend effect:

```asm
; before                         ; after
push word 0                      push dword 31h
push word 31h
push word 1                      push dword 10101h
push word 101h
```

The isolated `/tmp/qbopt-qrender-common-20260910` build links rewritten
common/view/d_turb. Its first 60-tick run exits with code 0 but produces
no fresh BENCH files or load traces. Copied benchmark files were renamed
before launch. This is **not a passing runtime check**. Emulator session
2131, debug port 2197, remains available; inspect execution against the
last working executable before diagnosing a common miscompile. No source
fix has been claimed; any discovered defect must gain a fail-first test.

### Three-module run: live frame-walk hang

Direct register and symbol inspection corrects the preceding exit report:
qrender is still executing B$FindFrame (2fc3:5e64..5e6b), repeatedly following
`BX = [BX-2]`. The socket's `hasLastExit` was stale and its shell-ready/frozen
fields did not establish guest termination. A queued GOOD.EXE comparison
has **not** been consumed; do not launch it a second time or claim it ran.
The live process is 99047, exec session 2131, port 2197.

```asm
; observed runtime loop (no fix yet)
cmp ax,bx
jb  done
mov bx,[bx-2]
jmp loop
```

Observed AX=a910, BP=a902, SP=a8f8; the chain reaches BX=0 and does not
terminate. Inspect frame-link corruption and allocated stack slots using
the saved common stages. No before/after fix or successful run is claimed.

### Frame ownership fixes

The common stage dumps expose two concrete frame-layout defects. The
frame builder matched ENRA only when it had one operand; conservative
two/six-input interfaces bypassed that match. It now reads the size value
from LIR's fixed CX requirement and refuses an unknown size. Separately,
LEA-only frame locals were absent from the floor calculation; Address
operands now count alongside direct memory accesses. Both defects have
fail-first regression coverage; ten focused frame/prologue tests pass.

```asm
; COM_CHECK_ARGS before           ; after the CX-requirement fix
call far B$ENRA                  call far B$ENRA
mov [bp-2],ax                    mov [bp-18h],ax
```

The intermediate dump `/tmp/qbopt-common-frame-fixed` exposed that BP-18h
itself names an address-taken string local. The subsequent Address-floor
fix puts new slots below those locals; its focused test changes a spill
from BP-18h to BP-22h with a BP-20h address-taken local. Regenerate the full
module and rerun before claiming the renderer hang fixed. Runtime-specific
frame metadata sizes also still need checking against the VBDOS prologue;
the inherited FR_SIZE=10 comes from QB's source.

### VBDOS frame-header size

VBDCL10E ENRA pushes ten words below BP at 0024..0036 before SUB SP,CX:
previous frame, SI, DI, CX, runtime word, and five zero words. Its header
is twenty bytes, not QB's ten. Emission now passes compiler family to the
backend frame builder. The regression fails with the inherited size and
passes with twenty; ten focused frame/prologue tests pass.

The final common dumps `/tmp/qbopt-common-frame-final` emit 14,468 bytes:

```asm
; original faulty allocation      ; frame fixes applied
call far B$ENRA                  call far B$ENRA
mov [bp-2],ax                    mov [bp-22h],ax
```

Fresh linked build `/tmp/qbopt-qrender-frame-20260910` is running with
common/view/d_turb rewritten, exec session 81371, debug port 2198. Its
copied benchmark outputs were renamed before launch. Runtime validation
is pending; retain the original hung guest on port 2197 as evidence.

### Three-module runtime check passed

The rebuilt guest returned to the DOS prompt and wrote fresh BENCH files
at 23:33. common/view/d_turb completed 60 ticks: 13 frames, 266 polygons,
820 triangles, with matching entity records. The BMP is byte-identical to
baseline (SHA1 `d6e4096b3610249ff4d53b6829f1ab18ec108c7a`). Timing and
memory telemetry differ; no performance improvement is claimed. This
clears the observed FindFrame hang for this scene, not every renderer
path or the other eighteen BASIC modules.

### Batched string-interface audit

The saved whole-library report was reused for RTRM/FASC/FCHR and their
RefString, substring, temporary-deletion and allocation dependencies.
Each normal public return consumes two bytes. All GP inputs and unknown
effects remain; ASC may delete a temporary, CHR allocates, and RTRIM uses
the substring allocator. Three fail-first regressions and fifteen adjacent
tests pass. The baseline objects contain 10 RTRM, 18 FASC and 26 FCHR call
sites across seven modules. This establishes interfaces, not their complete
contracts or successful module emission. No assembly is rewritten by the
interface declarations themselves:

```asm
; before                         ; after
call far B$RTRM                  call far B$RTRM
call far B$FASC                  call far B$FASC
call far B$FCHR                  call far B$FCHR
```

### Input module: four-module runtime check

Explicit project interfaces for QGLKBDINIT (cleanup 4), QGLMOUSEINIT (8),
SCR_SCREENSHOT (8) and SYS_ERROR (2) let `in_main` emit 9,070 -> 8,692
object bytes. All six GP inputs and unknown effects remain. The initializers
install interrupt handlers; screenshot performs file I/O. Their source and
object prologues/epilogues were inspected, including code after interrupts
where the contract tool stops. These are not global runtime contracts.
Audited object SHA256 values:

| Object | SHA256 |
|---|---|
| kbd | 677360877e2f813fd7f5a37ef40766ef37dc48a735a6d8f9d420a7b5b2fffcdc |
| mouse | 596ad6d94989933eaccd2f9e14799bbc76b2a75d01e9f5f58e9aa28af9a77806 |
| screen | 598494eae3e667d6ca043c089d873c133e23b61445dffc4975ec3a7f7641c779 |

`/tmp/qbopt-input-interfaces` contains all stages. Example address argument:

```asm
; before                         ; after
mov ax,[bp+6]                    mov ax,[bp+6]
add ax,112Ah                     add ax,112Ah
mov bx,ax
push ds                          push ds
pop es                           pop es
push es                          push es
push bx                          push ax
```

Fresh build `/tmp/qbopt-qrender-input-20260910` links common/view/d_turb/
in_main rewritten. Port 2199 (exec session 73683) returned to the DOS
prompt and produced new BENCH files. At 60 ticks: 13 frames, 266 polygons,
820 triangles; every non-timing/non-memory benchmark field matches baseline.
BMP SHA1 is again `d6e4096b3610249ff4d53b6829f1ab18ec108c7a`.
This validates the scene, not interactive keyboard or screenshot coverage.
Seventeen BASIC modules remain; no new source defect was fixed in this step.

### Two-module runtime check

Relinking with rewritten `view` and `d_turb` succeeded. The isolated build in
`/tmp/qbopt-qrender-view-20260910` completed the same `dm3ish.bsp` scene
(`-lm -nostats -yaw 183 -bench 40 -ticks 60`) with exit code 0.
Fresh output matched the baseline: 60 ticks, 13 frames, 266 polygons,
820 triangles, player state and all eight entity records. Both frame BMPs
have SHA1 `d6e4096b3610249ff4d53b6829f1ab18ec108c7a`.
The text differences are timing fields and a constant 208-byte reduction
in reported free memory; this is not a speed or memory improvement claim.

Only two of 21 BASIC modules are rewritten in this run. The three project
interfaces remain explicit, separately audited inputs, not global contracts.
Scripted camera input and mouse movement are not established by this scene.
Keep additional optimization passes postponed while bringing the remaining
modules through emission and validating their actual execution. Every defect
fixed along this path has a fail-first regression in the same commit.

VBDCL10E.LIB, `rtenexit.asm`, B$ENRA:

```asm
004b  or bx,bx
004d  jne 0056
004f  mov [bp-0Eh],ax
0052  jmp far [savedReturn]
0056  push word [runtimeState]
005a  push bx
005b  call far B$HFirstAllocBlock
0060  jmp 004f
```

`tools/contracts.py` follows this dependency graph but reports incomplete
proofs, not an ABI declaration. Assembly before/after remains identical on
refusal; no optimized listing should be presented for these modules yet.
