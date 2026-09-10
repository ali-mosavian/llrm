# Qrender correctness gate

New optimization passes are paused until the optimized renderer builds and
runs correctly. Target: `qb-qrender/.claude/worktrees/qgl-poly-draw`, source
commit `2965fa9d91c8e5f14fd3cddd804219956c75e42c`.

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
