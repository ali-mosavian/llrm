# Bounds-check policy and lowering

Bounds checking has its own `--bounds-checks` flag, default off, independent of
`--basic-semantics`. Both flags are recorded separately in manifests and output
configuration markers. Bench and stage-dump tools accept the same policy.

**Implemented first slice:** static numeric `B$HARY` accesses with a relocated,
segment-contained allocation become ordinary scalar MIR subtract/multiply/add
operations and a selector load at the raise boundary. No optimization pass
recognizes the helper or its register convention. With checks enabled, the
original checked helper remains. Dynamic/huge/string forms are not lowered yet;
unchecked emission refuses them explicitly rather than silently retaining checks.
Loop preguards are also pending.

For a zero-based INTEGER array the address computation changes from:

```text
before: push index; push rank; descriptor argument; call B$HARY
MIR:    adjusted = index - 0; bytes = adjusted * 2; address = base + bytes
after:  mov bx,[index]; shl bx,1; add bx,array; mov es,[selector]
```

The assembly excerpt is PDS ARRIDX's first access; `array` and `selector` stand
for relocated operands, not hard-coded addresses. `/D` tracing calls remain
barriers, so this is exposure of address arithmetic, not yet a claim that every
address has become a loop-carried induction value.

Regression fixtures `fixtures/regressions/arridx-bounds-{p-g2,q-O,v-g3}.obj`
are unchanged BC output from `suite/arridx.bas`, using each named configuration
plus `/D`. All three execute with output 1260, with checks both enabled and
disabled (six runs). This also exposed an independent INTO normal-path SSA bug:
invented register results made ARRIDX print 630/0. INTO now observes flags
without redefining registers; its exceptional memory/control barrier remains.

Runtime evidence: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-array-into-fixed-0o53uveo`.
Per-pass dumps: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-array-native-stages-l5_dgmvk`.

## Actual compiler output

Compiled the existing ARRIDX and HARR sources with each primary configuration
plus `/D`, on QB 4.5, PDS 7.1 and VBDOS. All six compilations report zero
severe errors. Objects and complete PDS ARRIDX stage dumps are at:

`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-bounds-codegen-kkj8rbla`

All three compilers emit three `B$HARY` calls for ARRIDX's three accesses
and two for HARR's two accesses. These calls compute addresses as well as
checking bounds. Deleting the calls would delete the address calculation.
They also emit `B$LINA` calls at statement boundaries; do not remove those
as bounds checks, because they implement tracing and Ctrl-Break handling.

PDS ARRIDX initially refused emission at its first `B$LINA` (0x36).
Its normal stack interface is now established from the shipped libraries;
trace/break handling retains conservative clobbers and memory/control effects.

## Runtime evidence

Inspected `tools/libdump.py B$HARY --limit 95` against all three shipped
libraries and the QB runtime `rt/dynamic.asm` body:

- Descriptor address arrives in BX; indices and dimension count are on the
  stack. The result is ES:BX. The ordinary return restores AX/CX/DX/SI/DI/BP.
- Rank is checked against descriptor byte +8. For each dimension, the
  adjusted index is compared against zero and its element count. Dimension
  records start at +14 and contain count and lower bound, four bytes each.
- Element width is at +12. The address calculation accumulates a 32-bit
  offset, adds the data offset, then adjusts the segment using `b$HugeShift`.
  A zero segment also reports an error.
- Caller stack cleanup is `2 * (dimension_count + 1)`, not a fixed size.
  The return is an indirect jump through saved runtime scratch storage.
- QB's entry is dynamic.asm 1:00dc; PDS/VBDOS use hugearr.asm 1:0004.
- VBDOS additionally tests descriptor byte +9 bit 0x80 and, when set,
  dereferences the data-offset field through another pointer. A shared
  unconditional descriptor-load sequence would therefore be wrong.

These linear listings establish the address calculation but are not complete
contracts for the error paths or indirect-return dependency. Do not derive
purity, error-handler effects or normal termination from them. In particular,
the checked helper is not a pure pointer expression.

`B$LINA` also differs from the available source: all three library bodies
contain a trace-flag test and branch after the break-check call, whereas the
local source body ends immediately after that call. Use the library and its
reachable dependencies when establishing its interface.

## Implementation boundary

1. Establish the debug and checked-array call interfaces, including variable
   stack cleanup and error paths. Keep the original error machinery until
   checked execution is represented soundly.
2. Raise a descriptor-backed array access into semantic address calculation
   plus a separate bounds obligation. Descriptor layout, register arguments,
   and huge-pointer encoding belong in the frontend/backend, never loop
   optimization passes.
3. Native mode drops the bounds obligation, not the address calculation or
   allocation operation. Unsupported helper forms must remain explicitly
   unsupported; a retained checked helper is not an unchecked success.
4. In preserve mode, derive the accessed range from the induction variable,
   step and actual loop trip count. Account for wraparound, negative steps,
   zero trips, conditional accesses and descriptor mutation.
5. The user explicitly permits bounds errors to be raised before the loop,
   even when this skips earlier iterations' writes/output. A failed pre-loop
   check may therefore report the BASIC error directly; a checked slow-loop
   version is not required merely to preserve error timing. Guard the check
   with loop entry so zero-trip loops do not introduce errors. Early reporting
   does not authorize checking conditional accesses that would never execute,
   or treating mutable bounds as invariant.

For inputs compiled without checks, the flag cannot simply recover missing
checks from a helper call: the access may already be inline machine code.
Adding checks there requires recovered array identity, rank and extents, not
an assumption that any indexed memory operand is an array element.
