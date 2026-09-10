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
