# C object integration

Current status: all five qb-qrender C objects are refused unchanged. Running
`llrm-omf` on them is not yet equivalent to optimizing them. Assembly objects remain
out of scope. Subsequent renderer benchmarks use 300 frames.

| Object | Current refusal | Probe after selecting its single `_TEXT` segment |
| --- | --- | --- |
| d_faces | no code segment | unreachable range 0x5d..0x38a |
| pl_trace | no code segment | unreachable range 0x94..0x4e2 |
| r_span | no code segment | unreachable range 0xd2..0x3e2 |
| r_walk | no code segment | unreachable range 0x2f6..0x604 |
| sb_build | no code segment | no entry explains the relocation fields |

These are fresh Borland objects from qb-qrender main `1c9c30a`, compiled with
`-Ox`, in `/tmp/qbopt-qrender-main.psUwqM/`. The probe changed segment selection
in memory only; no output was used in a game build.

Required frontend/backend work, in order:

1. Recognize supported C OMF code segments explicitly, without mistaking
   arbitrary assembly or multiple code segments for a supported module.
2. Seed public procedures and discover direct internal calls; resolve near-call
   relocations before interpreting targets. Preserve private routine boundaries.
3. Partition procedures without requiring BASIC's 0x30-byte header or inventing
   a BASIC main body.
4. Model native BP frames and saved registers. Current spill insertion without
   B$ENRA goes before the first instruction, which is wrong for `push bp;
   mov bp,sp`: it shifts argument offsets. Restore space at the native epilogue.
5. Run ordinary MIR optimization, preserve near/far calls and relocations, then
   compare actual 300-frame output/FPS against the same BASIC-only build.

Example from real R_WALK_LAYOUT_OK, currently unchanged:

```asm
push bp
mov bp,sp
cmp dword [bp+6],13A2h
jne unequal
mov ax,1
jmp done
unequal: xor ax,ax
done: pop bp
retf 4
```

Calling-convention inference does not establish entry points, frame layout or
private-call targets. Keep unsupported objects atomically unchanged while
building this support; do not remove those refusal checks to claim coverage.

## Discovery implemented

Single `_TEXT` segments are now recognized when THEADR names a `.c` file.
Reachable, unrelocated direct near calls seed private routines without adding
callee edges to the caller's CFG. Relocated calls remain unresolved here.
The real Borland `r_walk` fixture now has only two unreached NOP bytes, at
0x80 and 0x20f; previously its private routines at 0..0x604 were not reached
from its public entries.

Before/after assembly is unchanged (discovery does not emit):

```asm
; before: private target not visited   ; after: target discovered
06bf: call 0334h                       ; 06bf: call 0334h
; reached from 0334h:
03c4: call 0000h                       ; 03c4: call 0000h
```

Borland floating-point FIXUPP records name FIDRQQ/FIERQQ/FIWRQQ at
WAIT/opcode bytes, not ordinary operand fields. `src/frontends/bc/fppatches.rs` now
recognizes their verified byte shapes separately from ordinary operands.
The symbol categories agree with JWlink's `c/objcalc.c` FloatNames table.
Nonzero addends, self-relative fixups, wrong widths and wrong bytes are not
accepted. This recognizes patch locations, not the runtime effects of a
particular library's symbols.

Four C code maps now succeed: d_faces (2,050 instructions, 420 patch sites),
pl_trace (804, 200), r_walk (757, 138), and sb_build (502, 86). r_span still
has an unexplained range at 0x34e..0x3e2. Emission explicitly refuses these
patches until backend relocation support exists; native partition/frame
support also remains unfinished.

Correction: the initial counts were too low because table detection still
treated FP patch sites as data. Patch recognition now precedes table detection;
the regression asserts that r_walk contains no inferred tables. Its five
procedures start at 0, 0x2f6, 0x334, 0x604 and 0x6cb, with no main body,
unexplained ownership or overlapping ownership. d_faces, pl_trace and sb_build
also partition completely (four, three and two procedures). Native frame
emission remains explicitly refused. r_span's unexplained function matches
the unused `toggle_active` in its source; no heuristic deletion is performed.

`src/backend/nativeframe.rs` now recognizes the entry footprint. R_WALK's public
renderer procedure reserves 72 bytes and pushes SI/DI, so a new spill must
start below BP-76, not BP-72. Five focused tests verify these offsets against
the real object. Exit matching now locates restores (including x87 cleanup
between saved-register pops and LEAVE), and frame/prologue accept that plan.
The LIR reservation test inserts SUB after saves and ADD before restores,
rejecting lost anchors and mismatched pops. Whole-module emission is still
gated: preservation of frame anchors through raising/allocation, stack-depth
validation and FP linker-patch handling remain to be integrated.

Native stack-depth analysis now propagates SP relative to BP through CFG
edges, requires equal depths at joins, checks restore-site depth and the
final saved-BP pop, and requires an explicit cleanup value for every call.
All five R_WALK procedures balance using private near-call cleanup derived
from their actual RET instructions and the artifact-pinned external contract
for R_EMIT_ENTITIES (28 bytes). `src/abi/nativecalls.rs` supplies those facts;
unknown external calls remain unknown. Tests reject missing caller cleanup and
wrong or missing callee cleanup. This verifier is not yet the production
emission gate; whole-C emission stays refused.

`--native-fpu` now removes verified FP patch FIXUPP subrecords before raising;
instruction bytes and all ordinary fixup targets remain unchanged. The native
mode does not ask the linker to replace WAIT/ESC bytes with emulator forms.
The conversion is idempotent and tested through emitted/reparsed OMF records.
Unknown patch shapes stay unresolved. Native-frame integration remains gated.

Private-call interfaces now combine the requested C/Pascal register-input
assumption with measured near-return cleanup; all memory/clobber/control
effects remain conservative and no floating-stack contract is claimed.
With these per-call contracts supplied to both raise and lower, all five
R_WALK bodies reach LIR (70, 10, 381, 28 and 274 operations in public/private
partition order). Previously 0x604 and 0x334 refused their first private call.
Allocation and x87 call/return integration are not yet verified.

Allocation probe: all five bodies now traverse the machine pipeline with
native frame plans, dumping each stage in `/tmp/qbopt-c-stages.PCYsD4`.
It exposed an ABI defect: unpinned incoming SI/DI saves became PUSH BX/AX,
and their restores became POP AX/AX. Native frame plans now pin those values
in the backend. The regression checks physical saves and restores after the
whole pipeline and fails when the pins are removed. Eighteen focused frame/
prologue tests pass. Spill sizes are 4, 0, 2, 0 and 46 bytes; no executable
has been emitted or benchmarked from this probe.

Per-site clobber wiring is corrected: lower previously reloaded the global
symbol contract instead of using the contract supplied to raise/lower.
A routing regression with an explicit clobber mask fails under the old lookup;
19 frame/prologue tests pass. This does not yet narrow native ABI clobbers:
the existing register-family model maps SI to ESI, so the 16-bit preservation
rule cannot safely be applied to 32-bit values without width-aware facts.

The isolated full pipeline now emits `r_walk-probe.obj` (2,130 bytes) in the
stage directory. The final blocker was missing TEST immediate selection:
the real `test word [bp+6],8000h` now emits `F7 46 06 00 80`, unchanged in
meaning. Eight byte/word/dword register/memory cases pass, fail with the
encoder disabled, and check decoded masks and flag effects. The emitted
object maps successfully, with private near calls and RET/RETF distinctions
retained. It is not yet runtime-validated or enabled in production.

Runtime gate failed: isolated `/tmp/qbopt-c-run.C8JLSn` replaces only r_walk.obj
in the previously rendering all-17-BASIC build. LINK succeeds, but the requested
300-frame run stops with `String space corrupt in line No line number in module
D_SURF at address 06E6:1686`. No BENCH.TXT was produced and no FPS is valid.
Screenshot: `live.png` in that directory. The unchanged comparison build is
`/tmp/qbopt-300.WRBMTm` (300 frames, 15.72 FPS). Keep C emission disabled;
localize the first bad transformation in r_walk and add a symptom regression
before accepting a fix. A mapped object and balanced stack were insufficient.

Isolation run: removing only r_walk's FPU linker patch fixups, with its code
bytes asserted identical to the original, completes 300 frames and renders
DM3ISH. Artifact directory:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.hwd6kf0j`.
`BENCH.TXT` reports 15.60 FPS versus the earlier unchanged-C 15.72 FPS;
these single runs have different timer calibrations and establish no speed
change. `benchmark.png` is the final frame; both runs report 258 polygons,
809 triangles and 281 built surfaces. Thus patch removal alone does not
reproduce the corruption: investigate the raise/backend/emission path next.

One confirmed defect fixed: the frontend address recognizer accepted ES:BX
but returned an unknown address for ES:SI/DI. Re-encoding then lost the
segment override (and, for `[di+0Ch]`, its displacement). r_walk has nine
such accesses. Recognition now retains their far addresses; no MIR pass
was given machine-specific logic. Three emitted-code regressions cover SI,
DI and DI+0Ch; the first two fail again with the old recognition gate.
The related lift/address tests passed (158 tests before adding the third
case, then all three new cases passed).

```asm
; broken rewrite             ; corrected rewrite
fld dword [di]               fld dword es:[di]
fld dword [si]               fld dword es:[si]
fld dword [di]               fld dword es:[di+0Ch]
```

This is not the complete runtime fix. `/tmp/qbopt-c-segment.rI1Pdy` links the
2,139-byte corrected r_walk probe but still reports the D_SURF string-space
corruption during the 300-frame attempt. `live.png` records the error; no
completed benchmark or valid FPS. C production emission remains gated.

Function isolation (same 300-frame configuration):

| Rewritten C body | Other four bodies | Result |
| --- | --- | --- |
| `0604` public wrapper | Carried original instructions | 300 frames, 15.60 FPS, rendered |
| `0334` recursive worker | Carried original instructions | D_SURF string-space corruption; no valid FPS |

Artifacts are respectively
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.rh4au6pu`
(`BENCH.TXT`, `benchmark.png`) and
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.mdey1wxm`
(`live.png`). These are diagnostic variants, not a production fallback policy.
The second reproduces without rewriting either leaf helper or the public
wrapper: focus the next stage comparison on the worker at original `0334`.
No performance gain is established by these single runs.

Second confirmed backend defect: SAHF's implicit AH input was constrained as
a one-byte value even though SSA held the entire status word. At `04D6`,
allocation inserted `mov al,dl`, leaving AH stale. Implicit high-byte inputs
now require the enclosing word register, keeping their bit position intact.

```asm
; broken                      ; corrected, from the allocation dump
mov dx,[bp-2Ch]               mov dx,[bp-2Ch]
mov al,dl                     mov ax,dx
sahf                          sahf
ja target                     ja target
```

`test_native_status_flags.py` fails with the old constraint and passes with
the fix; mutation confirmed. Native-frame/status tests: 13 pass; related
opaque-emission/argument tests: 23 pass. The corrected worker-only probe
still fails with D_SURF corruption: artifacts under
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.gf86h66l`,
including `live.png`. No valid FPS. This fixes a demonstrated instruction
error, not the remaining whole-program failure.

Worker-only probe with the final peephole pass disabled also fails with
D_SURF string-space corruption. Artifact directory:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.upuk_r8u`
(`live.png`, no completed benchmark/FPS). This excludes peephole as the
sole remaining cause. The original `0430..0459` argument pushes and emitted
counterparts agree, including the dword at context+36h and the 28-byte total.
Next isolate raising/lowering/allocation from emission; do not repeat this
peephole-off run on unchanged code.

Resolved the worker-only failure. Pinning values to their original registers
still failed (`qbopt-c-native-only.jdy3whyy` under the same temporary parent).
Internal spill dumps then exposed dead call results surviving in `delivers`
after `allocate.narrowed` removed them from `defines`. Constraint splitting
created copies of undefined values, then spilling read unwritten slots.
Narrowing now drops the corresponding delivery constraints too.

```asm
; before, after the call at original 03C4
call helper
mov cx,[bp-34h]
mov cx,[bp-36h]
add sp,8
; after
call helper
add sp,8
```

The worker's phantom spill reservation falls from 46 bytes to zero.
`test_dead_call_deliveries.py` fails before the fix and on mutation; 25
focused tests pass across this fix, native status flags, lower arguments,
and existing dead-result coverage. Corrected worker-only run (peephole off):
300 frames, 15.5943 FPS, 258 polygons, 809 triangles, visibly rendered.
Artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.3y380bed`
(`BENCH.TXT`, `benchmark.png`). No performance improvement is established.
Next validate all five rewritten bodies together with peephole enabled;
this isolated pass does not yet justify enabling C production emission.

All-five-body integration with peephole enabled completes 300 frames, but
fails visual/geometry comparison. Artifact directory:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.p1ankm5f`
(`BENCH.TXT`, `benchmark.png`). Measured 12.8607 FPS, 351 polygons, 1033
triangles, 479 built surfaces, versus the passing worker-only 15.5943 FPS,
258 polygons, 809 triangles and the original comparison's 281 surfaces.
Camera position matches (232, -16, 184.0313), but the central floor differs.
This is not a successful optimization or a valid like-for-like speed result.
The full object is 2085 bytes; only the culling helper at original `0000`
still spills (2 bytes). Next isolate the rewritten leaf helpers' culling
results; keep production C emission gated until geometry agrees.

The mismatch was a missing native return operand: allocation changed the
culling helper's `xor ax,ax` to `xor cx,cx`, then returned stale AX.
Native raising now retains DX:AX as return values (the object has no return
type); lowering supplies their fixed-register constraints without encoding
them as RET operands. The stack-cleanup immediate is preserved.

```asm
; broken rewrite              ; corrected rewrite
xor cx,cx                     ; xor cx,cx
; restore frame               ; restore frame
                              ; mov ax,cx
ret                           ; ret
```

The fail-first return regression also fails with native return annotation
disabled. All five `r_walk` bodies now rewrite with peephole enabled:
300 frames, 15.7925 FPS, 258 polygons, 809 triangles, correct visible floor
and matching camera. Artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.ehfhotub`
(`BENCH.TXT`, `benchmark.png`). Object size 2144 bytes; culling and worker
each reserve two spill bytes. This is a backend correctness run, not a MIR
optimization run or evidence of a speedup. Production C emission remains
gated pending integration of native frame/call contracts and validation of
the other C modules.

Production integration now validates each native frame and call cleanup
before optimization, uses the same call contracts in raise and lower, and
passes the native layout to the allocator. Return dependencies also retain
SI/DI when a leaf has no original saves. Its fail-first regression fails
again when those operands are removed.

The unoptimized production `r_walk.obj` is byte-identical to the verified
15.7925 FPS artifact (SHA-256
`f63fbbcb9487f2aad609e0efa821ae01e472e34aa2623d217d3d5813ceafb3e1`).
The other C modules still refuse atomically:

| Module | Remaining blocker |
|---|---|
| d_faces | Frame/cleanup unproved at 038a |
| pl_trace | References below the established frame reservation |
| sb_build | Frame/cleanup unproved at 0000 |
| r_span | Unexplained code at 034e–03e2 |

Production probe results and stage dumps:
`/tmp/qbopt-c-production.RzZfcR`. With MIR optimization enabled, `r_walk`
emits 2114 bytes versus 2144 without it. The culling helper reuses its
argument instead of loading it three times:

```asm
; before                      ; after
mov si,[bp+8]                 ; mov ax,[bp+8]
mov dx,[bp+8]                 ; mov dx,ax
add dx,4                      ; add dx,4
mov ax,[bp+8]                 ; mov si,ax
add ax,8                      ; add si,8
```

Optimized production run: 300 frames, 15.9590 FPS, 258 polygons, 809
triangles, matching camera and visible scene. Artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.2q18iyg7`
(`BENCH.TXT`, `benchmark.png`). The first attempt exited before recording
results; the retry used the same executable. Against the unoptimized
15.7925 FPS single run, the small difference does not establish a speedup.
Five focused production/return/refusal checks pass. No full commit gate
was run; the existing lower-call type diagnostics remain outstanding.

### C loop promotion prerequisite

The real `r_walk-borland.obj` culling body at `0000` contains no calls.
Its unpromoted counter is not blocked by unknown callee effects. Raising
currently produces 38 opaque memory barriers:

| Instruction family | Sites |
|---|---:|
| `fldz` | 9 |
| `fcompp` | 8 |
| `fnstsw [bp-4]` | 8 |
| `sahf` | 8 |
| Register `fstp` | 3 |
| Register `fadd` | 1 |
| `les` | 1 |

These counts come from decoded original instructions whose raised operations
satisfy `effects.unmodeled_write`, after removing native-FPU linker patches.
The stage dump `r_walk-opt/0000-mir-r01-promote.txt` under
`/tmp/qbopt-c-production.RzZfcR` retains the memory increment at `02e0`.
Experimentally adding INCREMENT/DECREMENT to promotion's supported reads
does not change it: the opaque operations still invalidate availability.
No production change or performance improvement follows from that experiment.

```asm
; original comparison sequence; still opaque after raising
fldz
fcompp
fnstsw [bp-4]
; status word load, then
sahf

; before                         ; after current optimizer
inc word [bp-2]                 ; inc word [bp-2]
cmp word [bp-2],6               ; mov ax,[bp-2]
                               ; cmp ax,6
```

Next prerequisite: raise the floating compare/status chain and remaining
floating stack forms with explicit value, memory and exception semantics.
Keep unrecognized forms conservative. Then expose increment/decrement as
value updates, promote the counter and measure allocation. Do not simply
declare opaque instructions readonly: the status store really writes memory,
and the stack and comparison dependencies must survive optimization.

Implemented first: `fldz`/`fld1` raise as exact integer-to-real conversion
of 0/1 with a stack push, no memory access, and explicit exception policy.
Lowering selects the original constant-load instruction without allocating
a temporary. The real culling regression failed first and fails with
recognition removed; nine of its 38 opaque barriers are now gone. The 86
related floating semantics/allocation tests pass. Remaining comparison,
status and stack forms are still conservative.

```asm
; before                         ; after (identical bytes)
fldz                            ; fldz
; MIR: opaque unknown write      ; MIR: exact fload 0, no memory access
```

The production `r_walk` rebuild is byte-identical to the correctly rendered
`qbopt-c-native-only.kodssnh4/r_walk.obj` (2114 bytes). Fresh per-pass dumps
are in the probe directory above. This is a recognition prerequisite,
not a speedup; no duplicate renderer benchmark was run.

Register-to-register `fadd`/`fsub`/`fmul`/`fdiv` now also raise as arithmetic
over two extended values, with dynamic precision/rounding and unchanged
stack depth. Both destination forms retain subtraction/division operand
order. Nine new checks failed first, then passed; restoring memory-only
recognition fails the real dot-product regression. The 93 related floating
and stack checks pass.

```asm
; before                         ; after (identical bytes at original 02ba)
fadd st0,st1                    ; fadd st0,st1
; MIR: opaque unknown write      ; MIR: extended addition, no memory access
```

The culling body now has 28 rather than 38 opaque barriers. The rebuilt
2114-byte object still matches the validated renderer object exactly.
Comparison/status transfers, register stores and the far-pointer load
remain prerequisites; counter promotion is not claimed yet.

Memory footprints are now separate from opaque computation. Verified
FCOMPP, FNSTSW, SAHF and register-FSTP forms carry complete memory effects;
their movement/deletion barriers and stack/state dependencies remain.
FNSTSW writes its actual two-byte status cell, not every local. Unknown
instructions and calls still have unknown effects. The stage dumper now
prints these footprints, e.g. `complete reads=- writes=L4` at `0058`.

```asm
; before                         ; after, same status dependency
fcompp                          ; fcompp
fnstsw [bp-4]                   ; fnstsw [bp-4]
nop                             ; nop
mov ax,[bp-4]                   ; mov ax,[bp-4]
sahf                            ; sahf
; memory: all locals clobbered   ; memory: only [bp-4] written
```

The real status-store regression fails without the footprint recognition;
24 focused footprint/availability/call-effect tests pass, including overlap
and unknown-barrier controls. The rebuilt r_walk object grows from 2114
to 2140 bytes. The 300-frame renderer run completes at 16.5042 FPS versus
the preceding 16.5105, with matching 258 polygons / 809 triangles and a
visually verified scene. No speed improvement is established. Artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.0tmks4y1`
(`BENCH.TXT`, `benchmark.png`). Full commit gates remain outstanding.

Promotion now accepts increment/decrement memory updates, separating their
value computation from the retained observable store. The regression uses
the real culling counter's initialization/update/read in isolation, preserves
its flag definition, and fails with unary support removed. The focused run
has 33 passes and three ADDRM failures; all three also fail with this change
disabled. Those assertions remain unchanged pending their own diagnosis.

```asm
; full renderer before           ; after unary promotion support
inc word [bp-2]                 ; inc word [bp-2]
mov ax,[bp-2]                   ; mov ax,[bp-2]
cmp ax,6                       ; cmp ax,6
```

The complete renderer object is still byte-identical to `0tmks4y1/r_walk.obj`.
Its preheader's opaque `les bx,[bp+4]` invalidates the counter initialization
before the loop, so unary support alone cannot promote it. Modeling the far
pointer/address-space identity is required; simply declaring LES readonly
would risk retaining loads through the previous segment identity. No new
FPS run or speedup is claimed for this pass change.

Address-state work in progress: bodies containing a 16-bit LES now carry
address-space definitions through CFG joins using SSA. The bounding-box
loads consume the LES result; plane loads consume the later selector load,
not the bounding-box identity. LES has a complete four-byte read footprint.
The real identity regression and five existing address tests pass, and
disabling address-state construction makes the new regression fail.

This enables the counter's promotion, but allocation spills it:

```asm
; before                         ; candidate after
inc word [bp-2]                 ; mov ax,[bp-2Ah]
mov ax,[bp-2]                   ; inc ax
cmp ax,6                       ; mov [bp-2],ax
jge exit                       ; cmp ax,6
                               ; jge exit
                               ; mov [bp-2Ah],ax
```

**Candidate runtime gate failed:** the 2160-byte optimized r_walk completed
300 frames with zero polygons/triangles. Its 103.5899 FPS is invalid,
not a speedup. Artifacts: `qbopt-c-native-only.vylu5nxr` under the same
temporary parent as the prior runs. Do not accept this address-state change
as renderer-correct. A 2155-byte build retaining the new raise/backend but
disabling MIR optimization is prepared as the next isolation control.

That control also fails: 300 frames, zero polygons/triangles, invalid
101.7282 FPS. Artifacts: `qbopt-c-native-only.uz8cm97z`. Thus disabling
promotion and other optional MIR passes does not remove this regression;
isolate the new address-state construction/lowering before changing loop
optimization. Both failing candidates remain uncommitted. The last accepted
renderer is still `0tmks4y1` (16.5042 FPS, 258/809).

### Restored address-state definition

The address-state regression was in the recursive walk, not the culling
loop: its `POP ES` at original `0413` defined a MIR state but retained a
physical destination operand. Lowering therefore defined no LIR value for
it, and allocation reloaded an unwritten spill slot over the restored ES.
Raise now binds explicit selector destinations to their state value.

Actual emitted code around the following array store:

```asm
; broken candidate             ; corrected candidate
pop es                         ; pop es
mov es,[bp-38h]                ;
mov [es:bx],di                 ; mov [es:bx],di
```

`test_restored_selector_defines_the_following_store_address` failed first
and fails when the physical destination is restored. Twelve focused
address/production-emission tests pass; changed-file ruff and ty pass.
The optimized `r_walk.obj` is 2157 bytes, versus the broken 2160 bytes.

The corrected build completed 300 frames at **16.4789 FPS**, with **258
polygons / 809 triangles** and a verified scene screenshot. Artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.zwctexv3`
(`BENCH.TXT`, `benchmark.png`). This restores rendering; it does not establish
a speedup over the preceding correct 16.5042 FPS run. Counter promotion
still spills. Full batch/commit gates remain outstanding.

### Culling register-pressure audit

The first allocation attempt spills the promoted counter (value 171), not
because of a call or clobber mask: its live interval crosses no clobber mask.
Its spill weight is 0.02796. The six general registers conflict with status
temporaries, address copies, three pointer recurrences and a bounding-box
value. Raising the counter's priority alone would move the spill elsewhere.

At the strength-pass input, induction analysis already recognizes all four
recurrences in the loop at `004a`:

| Seed | Step |
|---|---:|
| frustum pointer | 20 |
| frustum pointer + 4 | 20 |
| frustum pointer + 8 | 20 |
| 0 (counter) | 1 |

The pointer seeds are actual MIR: the load at `0010`, its add-four at `0016`,
and add-eight at `001c`. Recognition is therefore not the missing step.
The next optimization opportunity is sharing equal-step recurrences and
folding constant offsets into their address uses, preserving modular-width
semantics and address-space identities. It must not introduce registers
into MIR. The current strength pass creates recurrences but does not merge
these existing related ones.

```asm
; current loop tail             ; intended shared-address form, NOT emitted
add si,14h                      ; add pointer,14h
add dx,14h                      ; accesses use pointer+4 and pointer+8
add bx,14h                      ;
```

No performance change was made or claimed by this audit. Captured dumps:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-pointer-recurrences.as6xpp3q/r_walk-opt`.

### Recurrence-sharing prototype

`src/optimize/ivshare.rs` reconstructs a related recurrence from an equal-step
base plus its constant seed offset, at the same integer width. It reads no
machine registers or origin map. Two focused real-object tests cover sharing
the culling pointers and refusing unequal strides; changed-file ty/ruff pass.

The prototype is **not enabled in the default pipeline**. A diagnostic
emission shares both pointer phis but retains the latch updates and the
counter spill, so this is not yet a profitable loop optimization:

```asm
; before: loop header           ; prototype: loop header
mov es,[bp+0Ah]                 ; mov di,si
fld dword [es:si]               ; add di,8
                               ; mov dx,si
                               ; add dx,4
                               ; mov es,[bp+0Ah]
                               ; fld dword [es:si]
```

Both outputs still have three add-20 instructions at the latch and reload
the counter from `[bp-2Ah]`. The stage dump shows the updated DX-derived
pointer remains observable through an exit phi and the conservative native
return contract. Do not delete that dependency to obtain a better number.
Exit handling and displacement folding must make sharing profitable before
it becomes a default pass. No renderer run or speedup is claimed for the
prototype; the latest runtime-verified build remains `zwctexv3`.

### Lowered address forms

`src/backend/addressforms.rs` follows word-sized copies and constant additions
when selecting far memory operands. The effective displacement wraps at
16 bits and keeps the original selector. MIR is unchanged. This is enabled
independently of the recurrence-sharing prototype, which remains disabled.

Actual culling output removes an address-delivery copy:

```asm
; before                       ; after
mov di,dx                      ;
fld dword [es:di]               ; fld dword [es:di]
```

Allocation holds the address directly in DI in the second listing; the
unchanged spelling of the load is not evidence that its value was unchanged.
The emitted r_walk object shrinks from 2157 to 2155 bytes. Its three
pointer recurrences and counter spill still remain.

This also exposed a native-frame anchor bug: an inserted spill store after
POP shared its source address and was counted as a second restore. Both
reservation and release now identify a contiguous expansion with exactly
one original byte span. Missing or duplicated original anchors still refuse.
The real-object test failed first and fails with the old check reinstated.

Twenty-three focused address/frame/return tests pass, including emitted
positive, negative and wrapping displacements. The rebuilt object is
byte-identical to the one run for 300 frames: **16.3373 FPS**, **258 polygons
/ 809 triangles**, verified scene screenshot. This is below the preceding
16.4789 FPS single run, so no runtime speedup is established. Artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.jb8zjeld`
(`BENCH.TXT`, `benchmark.png`). Full batch/commit gates remain outstanding.

### Combined sharing and address selection

A diagnostic run combining the sharing prototype with address selection
emits 2143 bytes versus 2155 without sharing. It is not runtime-validated
or enabled by default. Dumps and the emitted object are under
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-shared-addresses.y3hjkcdm`.

The second-component load now uses the common pointer:

```asm
; separate recurrence          ; shared recurrence + address selection
fld dword [es:di]              ; fld dword [es:si+4]
```

The latch still has all three pointer updates. There are two distinct
reasons, not just the return contract:

- `v2_5` feeds exit phi `v2_7`, an actual native return operand.
- `v4_6` feeds exit phi `v4_9`, with no ordinary operand reader. A subsequent
  half-value audit found the hidden dependency: POP SI's result `v4_7`
  merges the preserved upper half from `v4_9`. `halves()` marks `v4_6` HIGH
  live and LOW dead. The caller-visible root model retains that high half.

The earlier conclusion that the second phi was unused was incorrect.
`dead()` does conservatively retain every phi around opaque operations,
but disabling that policy did not remove this update: its HIGH half remains
live independently. The experimental completeness flag was removed rather
than using it to bypass the dependency. A focused test records the HIGH-only
use and the retained producer. The next dependency is correct partial-value
propagation through word arithmetic/restores, alongside moving genuinely
observed final pointer computations to exits. Memory completeness alone
cannot justify either change.

### Word-arithmetic preservation dependency

The raise omitted upper-word carry when a word-sized destination was also
an explicit input. In the real culling body, `ADD SI,8` at `001c` therefore
failed to propagate the returned upper-word demand to its input. Word
arithmetic now records that preservation edge as well as its low-word read.
This changes recognition, not the MIR pass vocabulary or the return contract.

`test_word_arithmetic_carry.py` failed before the fix and again with the old
condition restored. The four recurrence/carry tests and six focused
carry/return/address-state tests pass. The production probe dumps every stage
under `/tmp/qbopt-c-production.RzZfcR/r_walk-opt`.

The emitted object is byte-identical to the last verified 2155-byte build:

```asm
; before                 ; after (unchanged)
add bx,14h               ; add bx,14h
mov dx,di                ; mov dx,di
add dx,14h               ; add dx,14h
add si,14h               ; add si,14h
```

No new FPS claim or redundant renderer run. This repairs the dependency
needed for sound simplification; it does not yet remove the extra updates.
Recurrence sharing remains disabled. Existing lint failures elsewhere in
`src/model/mir.rs` and the full integration/commit gate remain outstanding.

### Upper-word identity normalization

The following raise step now follows upper-word preservation through word
operations and phi cycles. A unique reaching source replaces only the
preservation edge; ordinary low-word operands remain unchanged. Multiple
reaching sources and whole-word replacements stop the proof. The earlier
scalar normalization also needed to propagate demand through arithmetic
merges, not just loads/copies.

The culling restore now carries the incoming value's upper bits directly,
not the last pointer increment. The regression checks that fact, stops at
a full-width replacement, and checks idempotence; disabling canonicalization
fails it. The old prototype test expecting the increment to remain HIGH-live
was updated because that dependency has now been proved redundant, not
because the caller's preservation requirement changed.

The actual emitted improvement is four copies before integer-to-float
conversions, not pointer-update removal:

```asm
; before                 ; after
mov ax,cx                ;
mov [bp-4],ax            ; mov [bp-4],cx
fild word [bp-4]         ; fild word [bp-4]
```

`r_walk` is 2147 rather than 2155 bytes. The default production probe is
byte-identical to the renderer-validated object. Ten focused carry, recurrence,
return and address-state tests pass; changed-file ruff/ty pass. The three
pointer increments and counter spill remain, and sharing stays disabled.

300 frames: **16.4852 FPS**, **258 polygons / 809 triangles**, matching
scene screenshot. This is within the recent single-run spread, not an
established performance improvement. Artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.vk18ji3d`
(`BENCH.TXT`, `benchmark.png`). Full integration/commit gate outstanding.

### Dead values across fully described opaque readers

MIR now distinguishes complete value reads from modeled computation and
complete memory footprints. The raise establishes `reads_complete` only
from a finite decoder read set and known call arguments. DCE keeps opaque
operations and their side effects, but their presence alone no longer
keeps every phi and definition alive. Unknown readers retain the old gate.

The real culling regression now removes the shared pointer's unused update
at `02dd`, while retaining the returned update at `02da` and every barrier.
Making the readers unknown retains the update. Restoring the old DCE gate
fails the test. Eight focused recurrence/carry/return tests pass.

A diagnostic combining sharing with DCE emits 2119 bytes versus 2147:

```asm
; before                         ; combined diagnostic
fld dword [es:di]                ; fld dword [es:si+4]
fld dword [es:si]                ; fld dword [es:si+8]
; ...                           ; ...
add bx,14h                      ; add si,14h
mov dx,di                       ; mov ax,di
add dx,14h                      ; add ax,14h
add si,14h                      ;
```

The diagnostic still reconstructs offsets in the header and spills the
counter. It is not the intended one-pointer loop yet. Sharing remains
disabled by default; DCE's completeness distinction is enabled. Stage dumps:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-shared-dead.gath8g0z`.

300 frames rendered correctly: **16.3310 FPS**, **258 polygons / 809 triangles**,
verified screenshot. This does not establish a speedup over the preceding
16.4852 FPS run. Artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.u4re6f7m`
(`BENCH.TXT`, `benchmark.png`). Full integration/commit gate outstanding.

### Dead address arithmetic after allocation

Address folding left `MOV AX,SI / ADD AX,8` in the shared-loop header even
though every +8 load used a displacement. The post-allocation overwritten
pass now tracks flag reads and definite writes alongside register byte lanes.
It removes register-only ADD/SUB/AND/OR/XOR only when every written lane and
flag is overwritten before use. Unknown instructions, live flags, carry
readers, memory operands and delivery constraints remain conservative.

```asm
; previous shared diagnostic    ; current shared diagnostic
mov ax,si                      ;
add ax,8                       ;
mov di,si                      ; mov di,si
add di,4                       ; add di,4
```

The object shrinks **2119 -> 2114 bytes**. The +4 reconstruction and two
pointer increments remain. Three focused cases cover removal, live flags,
and an ADC consuming carry; disabling arithmetic removal fails the positive
case. Existing byte-read/copy checks pass. The HARR test requiring an exact
`xor cx,cx` spelling fails with this removal both enabled and disabled;
it was not changed or counted as passing.

300 frames: **16.4979 FPS**, **258 polygons / 809 triangles**, correct scene
screenshot. No established speedup beyond the recent run-to-run spread.
Artifacts: `qbopt-c-native-only.053iwlcl` under the existing temporary root;
stage dumps: `qbopt-dead-address.kpt7mar9`. Sharing remains experimental;
the backend cleanup itself is enabled. Full integration/commit gate remains.

### Sink the returned pointer update to normal exit

The `exitsink` prototype moves pure ADD/SUB producers read only by a
single-input loop-exit phi. Operands and preserved upper bits are exported
through phis, and the producer's value identity is retained at its new
definition. The original instruction becomes an ownership marker. Unknown
readers, other uses, live produced flags and live entry flags refuse the move.
This is MIR-only; no register or origin-map decisions were added to the pass.

The current culling candidate finally has one pointer increment per iteration:

```asm
; before: loop tail            ; after: loop tail
add si,14h                    ; add si,14h
mov ax,di                     ; counter update and branch
add ax,14h                    ;
; counter update and branch   ; normal exit only:
                              ; mov dx,di
                              ; add dx,14h
```

The object is 2112 bytes versus 2114. Four focused sharing/exit tests pass;
the real-object exit test first failed while root liveness retained the old
definition, then passed when the definition itself moved. Ruff/ty pass.
Sharing and exit sinking are still not enabled by default.

300 frames: **16.6349 FPS**, **258 polygons / 809 triangles**, matching scene
screenshots. This single run is not a reliable speedup estimate. Artifacts:
`qbopt-c-native-only.fevtrexp` under the existing temporary root, including
`BENCH.TXT`, `progress.png`, `benchmark.png`; stage dumps are in
`qbopt-exit-sink.zxe3pg_l`. Integration/commit gate remains outstanding.

### Sharing and exit sinking in the normal pipeline

The strength phase now performs recurrence sharing, dead-value cleanup and
exit sinking before existing exit evaluation. Fixed-point rounds finish
sharing the related pointers. These steps no longer require diagnostic
monkeypatches.

Before enabling them, wider reads exposed a missing upper-word preservation
edge in the prototype. Sharing now proves that every incoming definition
carries the same loop-invariant upper source and attaches that source to the
reconstructed value. If a wider read exists and that proof is unavailable,
sharing refuses. The focused test failed without preservation and fails when
the merge is mutation-disabled; unknown preservation is also refused.

The normal `r_walk` emission is byte-identical to the verified 2112-byte
candidate, including its one-increment loop:

```asm
; previous default loop tail   ; current default loop tail
add bx,14h                    ; add si,14h
mov dx,di                     ; counter update and branch
add dx,14h                    ;
add si,14h                    ; normal exit: mov dx,di / add dx,14h
```

Eight focused sharing/exit safety checks pass, and new-file ruff/ty pass.
No repeat runtime run for identical bytes: the matching 300-frame result is
16.6349 FPS with 258 polygons / 809 triangles and verified screenshots in
`qbopt-c-native-only.fevtrexp`. Broad integration, the known HARR spelling
expectation, and the commit gate remain outstanding.

### Remaining C admission: surface builder

A fresh probe confirms `sb_build`'s frame plan itself is established; three
callee cleanup facts were missing. Manual disassembly of the linked inputs
establishes normal-return cleanup only:

| Symbol | Input | Return evidence |
|---|---|---|
| F_SCOPY@ | Borland CL.LIB, module F_SCOPY | `0019: retf 8` |
| MEMCOPY | UGLV.LIB, dosmem.asm | `02be: retf 12` |
| UGLBUILDSURF | UGLV.LIB, uglsurf.asm | `046f` and `0480: retf 12` |

No preservation or memory-effect promises were added. A diagnostic probe
with these cleanup facts emits `sb_build` as LIR: 2332 original object bytes
become 2227. Its FP sequence now omits waits:

```asm
; original                     ; emitted
les bx,[bp-18h]                ; les bx,[bp-18h]
wait                           ;
fld dword [es:bx+4]             ; fld dword [es:bx+4]
wait                           ;
fmul dword [bp-0B8h]            ; fmul dword [bp-0B8h]
```

Probe and stage artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-sb-contracts.sgmj1ww0`.
Renderer candidate `qbopt-c-native-only.phg3tu5v` incorporates this object
with the preceding verified BASIC/r_walk/d_faces/pl_trace objects. Runtime
validation is pending; this is not yet an accepted build or FPS improvement.

`r_span` remains refused: `034e..03e2` contains the unreferenced
`toggle_active` routine, not padding. No guessed entry or deletion was used.
The library contract tool also refuses the whole UGLV archive because an
unrelated `sndmixer.c` member contains unsupported 32-bit OMF records; the
UGLV cleanup facts above came from the specific members' raw disassembly.

### BASIC loop audit

A fresh optimized `d_surf.obj` (same production probe, current worktree)
still computes `i * 40` inside `LS_ANIMATE`; the spill reservation falls
from 22 to 20 bytes, not to zero. The argument-descriptor reloads remain.
The optimized counter's latch input merges the header counter with reloads
of `[bp-16h]` after calls. `B$LDFS`, `B$FMID`, and `LS_LCHAR` have unknown
memory effects in the current contracts, so those reloads cannot simply
be assumed equal to the pre-call value. The next useful prerequisite is
sound call-effect/escape information, not another multiplication rewrite.

```asm
; measured BASIC build        ; fresh BASIC rebuild
sub sp,16h                   ; sub sp,14h
; inside LS_ANIMATE: unchanged multiply-by-40 expansion
mov cx,ax                    ; mov cx,ax
shl cx,2                     ; shl cx,2
add cx,ax                    ; add cx,ax
shl cx,3                     ; shl cx,3
```

These are compiler-stage findings, not a runtime validation of the fresh
object. The 15.7925/15.9590 FPS comparison above held the optimized BASIC
objects fixed; it did not measure total optimizer benefit over original
compiler output.

A fully unoptimized baseline was then linked from the original 17 BASIC,
five C and two assembly objects, using the same library/assets and DOSBox
settings as the optimized run: 300 frames, 15.5943 FPS, 258 polygons,
809 triangles, matching camera and visible scene. Artifacts:
`/tmp/qbopt-render-baseline.YtB79D` (`BENCH.TXT`, `benchmark.png`). Against
15.9590 FPS with 17 BASIC modules and `r_walk` optimized, this single-run
comparison is +2.34% FPS. Reported mean culling time is 5.629 vs 4.047 ms;
drawing is 52.101 vs 52.427 ms. RDTSC calibration is nearly equal
(74.78350 vs 74.78258 MHz), but repeated runs are still needed to establish
the small overall gain reliably. Four C modules and the library remain
unoptimized in the optimized build.

### LS_ANIMATE frame-escape prerequisite

The original `d_surf.obj` distinguishes the loop counter at `[bp-16h]`
from a temporary string descriptor at `[bp-20h]`. At `02BB` and `02C4`,
`lea ax,[bp-20h]` supplies that descriptor to `B$SASS` and `LS_LCHAR`.
Before the frame-address change, the MIR dump printed both as `&?`:
`mir._operands` retained `ir.Address` as an opaque operand. This was not
evidence that no frame address escapes. Direct word-sized frame addresses
now raise as `FrameAddress(offset, width)`; the refreshed dump prints
`&frame(-0x20)`. Indexed and wider machine forms remain opaque.

```asm
; before                         ; after representation change
lea ax,[bp-20h]                  ; lea ax,[bp-20h]
push ax                         ; push ax
call far LS_LCHAR               ; call far LS_LCHAR
; later: no private-local proof gained yet
mov ax,[bp-16h]                 ; mov ax,[bp-16h]
```

`module.escaped` collects relocated data addresses, not frame addresses;
`mir._out_of_reach` only excludes the program-data segment. Neither proves
the counter private. Track the explicit addresses' escape and the runtime's
implicit access separately before narrowing call effects.
Do not extend the data-segment exclusion to all frame cells.

`src/analysis/frameescape.rs` now propagates explicit frame origins through
copies and phis to a fixed point. Other uses conservatively expose those
origins; opaque address sites are reported separately. Stage dumps include
the result. On the real `LS_ANIMATE` fixture the observed exposed origin is
`-0x20`, not the counter at `-0x16`. This describes explicit value flow only:
it is not a claim about runtime frame walking or allocation extents, and no
alias exclusion is derived from it yet. Tests cover the real fixture,
cyclic phi/copy propagation, calls, stores, returns, unsupported arithmetic,
and opaque addresses.

The real renderer fixture's two LEAs round-trip byte-for-byte; reverting
their operands to opaque makes `test_frame_addresses.py` fail. The production
`d_surf` rebuild succeeds with stage dumps, but this representation change
is not a measured speedup or a new renderer runtime validation.

QuickBASIC 4.5 `runtime/rt/string.asm` shows `B$LDFS` allocating a temporary
through `B$STALCTMP`; `strfcn.asm` shows `B$FMID` calling temporary-string
allocation/free routines and an error path. Their direct argument lists
alone do not prove transitive memory effects, and this source is not a
version-independent contract for the VBDOS renderer binary.

The VBDOS allocator audit found an instrument issue: depth-first graph
traversal exhausted its 256-function budget in error-handler descendants
before visiting the direct `B$FResizePpv` dependency of `B$AlcTmpSH`.
`tools/contracts.py` now visits breadth-first. A four-function budget
includes the allocator and all three direct dependencies (`FResizePpv`,
`ERR_OS`, `ERR_ST`); unknown deeper dependencies still suppress proofs.
The real-library regression fails with depth-first traversal restored.
This changes evidence coverage, not emitted assembly or runtime contracts.

The newly visible normal resize path calls block-move checks, allocation,
handle compaction, and free-block adjustment. Those dependencies still need
memory-region summaries before claiming the string calls preserve unrelated
frame locals. `LS_LCHAR` also calls `FLEN`, `FASC`, and BASIC frame-entry/exit
helpers; its short BASIC source does not establish purity.

### d_faces external cleanup audit

The native-frame refusal at `038A` is caused by missing external cleanup
facts, not a demonstrated malformed frame. Reading reachable RETF sites in
the original build gives these byte counts (no memory/preservation claims):

| Symbol | Cleanup | Return offset in defining segment |
| --- | ---: | --- |
| R_SPAN_DRAW_TO | 4 | 1245 |
| R_SPAN_FLUSH | 4 | 1221 |
| R_SPAN_EMIT_PTR | 30 | 0507, 0D5F |
| SB_BUILD | 28 | 0563 |
| UGLZMODE | 2 | 00AA |
| UGLPOLYTP | 16 | 0317 |
| UGLTRIF | 12 | 00B9 |
| UGLLINE | 16 | 00D0 |
| UGLTRITP | 14 | 00DF |
| UGLTRIT | 14 | 00D8 |
| F_FTOL@ | 0 | 002B |

SHA-256 identities: r_span.obj
`467deb49df54a08a7170c51ce6c718985cb25ef37f80756dddff2030dd616778`;
sb_build.obj `615e044d9262354b71967cf453199bfd452efc5c7f13b06e8706fd577d5260c2`;
UGLV.LIB `e260291c7a8979a4d041437b27bd874b57f660855575bd1a38c87c9c2d568067`;
Borland MATHC.LIB `160a64a8988b6b83d3ee5353c4d6b6f3e90e40e3a2f81c807feaf6b1abd025e3`.

With those diagnostic contracts, production reaches emission and refuses
`09F4: mov has 1 fixups and 0 fields to put them in`. Original instruction:
`mov [bx+0CA0h],eax`; lowered/allocated dumps show a destination with no
address and a BX base. This is the next emission defect to reproduce and
fix, not permission to discard the relocation. Stage dumps are in
`/tmp/qbopt-c-production.RzZfcR/d_faces-opt/038a-*`. Atomic refusal returns
the original 8536-byte object; no new game run or speedup is claimed.

The address reader omitted displaced BX operands while handling SI/DI.
Including BX preserves the named address, its index value, and relocation
field. The real `d_faces-borland.obj` regression fails before the fix and
with BX handling removed; all four C-address tests pass. The production
probe now emits a 7626-byte object. Object size includes metadata and is
not a speed estimate.

```asm
; broken lowering               ; corrected relocation-bearing form
mov [bx],eax                    ; mov [bx+symbol],eax
; no displacement field         ; disp16 field, zero until relocation
```

Runtime validation failed: adding this optimized `d_faces` to the previously
validated BASIC + `r_walk` + `pl_trace` build completed 300 frames at
39.39998 FPS, but the image is black and triangles are zero (expected 809).
Polygon count remains 258 and camera remains 232,-16,184.0313. This is a
miscompile, not a performance result. Artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.zfrce5cl`
(`BENCH.TXT`, `BENCH.BMP`, `benchmark.png`). The next correctness task is
to isolate the first incorrect stage; successful emission is insufficient.

The control with MIR optimization disabled also drew zero triangles
(39.1205 FPS). This localized the black scene below the optional passes.
Borland puts OFF16 addends in instruction bytes: the store at `09F4`
contains `0CA0h`, while its FIXUPP displacement is zero. Raising read only
the FIXUPP; emission zeroed the operand and lost the addend.

`src/objectfile/addends.rs` now moves code OFF16 addends into FIXUPP before
raising, after removing native-FPU linker patch records. Those patch records
point at instructions, not address addends, so the order matters.

```asm
; effective address before fix    ; after fix (including relocation)
mov [bx+arrayBase],eax            ; mov [bx+arrayBase+0CA0h],eax
```

The real-object regression checks every code OFF16 target sum, frame and
target identity, successful decoding and normalization idempotence. The
five focused addend/address tests passed; disabling normalization failed.

The corrected optimized `d_faces` emits 7910 bytes and renders a visible
scene: 300 frames, 16.8541 FPS, 261 polygons / 815 triangles. The reference
is 258 / 809, so this is not accepted as a correct speedup. Artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.r564v4ia`
(`BENCH.TXT`, `benchmark.png`). The next control retains normalization but
disables MIR optimization, keeping all other objects fixed.

That control completed at 16.6370 FPS, 261 polygons / 817 triangles, with
a visible scene (`qbopt-c-native-only.bgklttxn` beside the artifact above).
The backface comparison at original `0566` exposed a second defect:
`fcomp qword [4]` had a segment-2 relocation, but rebuilt `fcomp qword [0]`
had none. The LIR bridge supplied BARRIER semantics with no modeled
operands; assembly mistook this for a removed memory operand even though
it copied the original instruction bytes. The relocation was dropped.

The operand-ownership check now retains relocations on barriers carried
from an original byte span. `test_opaque_relocations.py` checks emitted
bytes and relocation ownership through the MIR/LIR bridge on the real
fixture; it failed first and fails again with the fix disabled. Both
served-load regressions still pass: genuinely removed operands must still
lose their relocations.

```asm
; broken                         ; corrected, linker-resolved operand
fcomp qword [0]                  ; fcomp qword [constant_0_01]
```

With the relocation restored, optimized `d_faces` emits 7966 bytes. The
300-frame run reports 16.3123 FPS and 258 polygons / 807 triangles: the
polygon count now matches, but two triangles still differ from the 809
reference. Artifacts: `qbopt-c-native-only.vc7c0fiv` beside the directories
above. This fixes the measured relocation loss, not the entire renderer
correctness problem. No full commit gate has run.

The same fixes with MIR optimization disabled complete 300 frames at
16.1088 FPS, 258 polygons / 809 triangles, matching the reference counts
and visible scene. All 28 object files were compared: only `d_faces.obj`
differs from the reference build. Artifact directory:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.a8xgnxyt`.
Thus the remaining 807-triangle result requires the optimized path;
the corrected non-optimized backend no longer reproduces it. Isolate
the changed MIR body before another pass-level experiment. The two
`clip_w` calls supply vertex counts used by the final triangle tally.

Body isolation (300 frames each, both relocation fixes retained):

| MIR optimization selection | FPS | Polygons / triangles | Artifact suffix |
|---|---:|---:|---|
| All except `clip_w` (`0094`) | 16.4204 | 258 / 807 | `kn8vwlm_` |
| All except `d_draw_faces` (`038a`) | 16.3847 | 258 / 809 | `s4zb5pp0` |

Artifacts are under the same `qbopt-c-native-only.*` temporary parent.
The skipped clipping helper's backend dump matches the non-optimized
control exactly; the first selection's caller matches the fully optimized
caller. The defect is isolated to optimizing the caller, not the helper.

Round-two hoist introduces an invalid dependency in the caller. The actual
SSA object at preheader `0b22`, anchored at `0b29`, is
`v209_1 = copy v4_120`, with uses `(v4_120, v4_120)` and merge metadata
`{v4_120: v209_1}`. Its source is still defined by the load at `0b54` in
loop block `0b2e`. `_invariant_run` and `_placement` exclude every merge
key from their dependency checks, including this genuinely consumed copy
operand. A preservation dependency and an explicit operand must be
distinguished; the copy cannot move above its definition. No fix for this
newly isolated defect has been applied yet.

LICM now includes explicit value and memory-address operands in both its
invariance and placement dependencies even when they also appear in merge
metadata. The real-object dominance regression fails before the fix and
under mutation; it and seven related LICM checks pass. This does not disable
LICM or move machine decisions into MIR. Emitted object size falls from
7966 to 7768 bytes because the invalid copy/spill chain disappears.

```asm
; before: preheader reads a slot only initialized inside the loop
mov bx,[bp-11Eh]
nop
mov [bp-120h],bx
; ... inside the vertex loop
mov dx,[bp+1Ch]
mov [bp-11Eh],dx
mov di,[bp-11Eh]
fld dword [di]
fmul dword [bp-0B8h]
mov di,[bp-120h]
fld dword [di+10h]

; after: pointer defined before both uses, no stale preheader copy
mov di,[bp+1Ch]
fld dword [di]
fmul dword [bp-0B8h]
fld dword [di+10h]
```

The corrected optimized run completes 300 frames at 16.5105 FPS,
258 polygons / 809 triangles, matching reference counts and visible scene.
Artifacts: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.kodssnh4`
(`BENCH.TXT`, `benchmark.png`). This is a single run, not a statistically
established speedup. Full integration/commit gates remain outstanding.

### Native outgoing stack arguments

`pl_trace`'s below-frame references are outgoing floating arguments, not
unreserved locals. Native stack validation now records accesses wholly
inside the active argument area. Lowering marks them before allocation;
frame growth relocates only marked accesses, never coincident spill slots.
Unallocated, indexed, or unproved accesses remain refused. A contiguous
expansion of one reservation-anchor instruction is accepted only when
exactly one instruction owns the original bytes.

```asm
; original recursive helper   ; optimized, with four spill bytes
sub sp,4                     ; sub sp,4
wait                         ;
fstp dword [bp-3Ch]           ; fstp dword [bp-40h]
wait                         ;
fld dword [bp+6]             ; fld dword [bp+6]
sub sp,4                     ; sub sp,4
wait                         ;
fstp dword [bp-40h]           ; fstp dword [bp-44h]
```

`pl_trace.obj` now emits with MIR optimization (1969 bytes; original 2908
includes floating linker-patch records, so the difference is not a code
speed metric). The stack-argument relocation and expanded-anchor tests
fail with their fixes removed. The real Borland fixture and source are
`tests/fixtures/regressions/pl_trace-borland.obj` and `pl_trace.c`.

With optimized `r_walk` and `pl_trace`, the 300-frame run completes at
16.3181 FPS, 258 polygons, 809 triangles, matching camera and scene.
Artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-c-native-only.4oklpul9`
(`BENCH.TXT`, `benchmark.png`). This is a single-run observation, not an
established speedup. `d_faces`, `sb_build`, and `r_span` remain unoptimized.

```asm
; original                       ; required placement for a 4-byte spill
push bp                          ; push bp
mov bp,sp                        ; mov bp,sp
sub sp,48h                       ; sub sp,48h
push si                          ; push si
push di                          ; push di
                                 ; sub sp,4  ; spill at [bp-50h]
```

```asm
; before and after: identical bytes, newly recognized linker patch sites
0008: wait             ; FIDRQQ: 9B D9 EE
0009: fldz
004d: wait             ; FIERQQ: 9B 26 D9 05
004e: fld dword es:[di]
005b: nop              ; FIWRQQ: 90 9B
005c: wait
```

`tests/fixtures/regressions/r_walk-borland.obj` and `r_walk.c` were copied from the
fresh main build above and qb-qrender main 1c9c30a, respectively. Four focused
discovery/refusal tests pass; disabling private-call traversal makes the
regression fail. No C optimization or C FPS gain is claimed.
