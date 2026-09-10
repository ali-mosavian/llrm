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
remain intact. No optimized renderer runtime result exists yet.

## Remaining blockers

- **VBDOS entry:** nonzero BX at `B$ENRA` calls `B$HFirstAllocBlock`.
  The helper's allocating path reaches `B$HandleAlloc`, heap compaction,
  recursive dependencies and unresolved indirect transfers. The existing
  BX=0 contract does not cover this path. Do not infer register preservation,
  memory purity or pointer stability from its fast path.
- **Other runtime contracts:** first refusals include `B$FLEN`, `B$EXTS`,
  `B$SIN8`, `B$FREF`, `B$ASSN`, and `B$RDIM`.
- **Cross-module interfaces:** BASIC, C and assembly callees need verified
  contracts; examples are `R_POINT_LEAF`, `IN_HANDLE_TOGGLES`, `QGLSFNEW`
  and `QGLSFPSET`. A name alone does not establish an ABI.
- **Resolved backend refusal:** `view`'s inserted add at 0x243 was blocked
  by an unused flags phi. Lowering now prunes dead flag merges before
  checking lifetimes, retaining consumed conditions. The module reaches
  its next blocker, `QGLMOUSEPOS` at 0x280; output is still unchanged.

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
