# CPU-dependent constant arithmetic

`wholeseg.emitted(data, cpu="386")` selects a tuning CPU explicitly;
the default preserves the existing 386 policy. `lower.lowered` accepts
the same option. Tuning changes instruction selection, not MIR semantics
or the minimum ISA (the output remains 386-compatible).

The backend compares binary and signed-digit shift/add/sub chains against
the direct multiply. The chain includes a source-preserving move in its
estimate; ties retain multiply. A signed-digit chain can implement seven
as `(x << 3) - x`, not three additions. Observed multiply flags still
prevent replacement.

486/P5/P6 and the other existing timing profiles use `cycles/timings.py`.
386 uses the existing opportunity scoreboard's representative arithmetic
ranking. These are approximate instruction-cost estimates, not benchmark
measurements, and do not yet model allocation spills, prefix penalties or
superscalar scheduling in the selection decision.

HOTLPX (PDS fixture), before target selection:

```asm
lea bx,[ecx+ecx*4]
shl bx,2
```

After selection: unchanged on 386/486/P5; on P6:

```asm
imul bx,bx,20
```

The full emitted object is 841 bytes for the chain, 837 for P6.
All three compiler variants pass the P6-output runtime check. The default
386 output is byte-identical for all 96 primary fixtures.

Signed positive constant division now compares a reciprocal sequence,
including remainder reconstruction, against IDIV during lowering.
`backend/division.py` derives the multiplier with LLVM's
`SignedDivisionByConstantInfo` algorithm. Zero, negative divisors and word
division retain the existing path. Its source-preserving moves and both
multiply results are explicit, so allocation sees every clobber.

The old midpoint comparisons have been withdrawn. `backend/timing.py`
records audited form/width-specific core-clock ranges independently of the
scoreboard. Selection compares the multiply's maximum with the divide's
minimum; an unknown timing profile retains IDIV. For division by seven,
386 and 486 retain IDIV. P6 also retains it pending exact-form evidence.
P5 selects the reciprocal: an estimate of 36 includes reserved copies and
one decode clock for each dword operand-size prefix, against IDIV's 46
core clocks alone (excluding its setup). The prefix rule is Intel
241430-004 section 24.3, printed page 24-3, PDF page 610.

These remain static estimates, not measured hardware timings. P5's PDS
object grows from 862 to 890 bytes. The default 386 object remains 862.
Three P5 runtime checks pass, one per compiler variant. The earlier
486/P6 experiments passed correctness checks but are no longer selected.

Before (division core):

```asm
mov ebx,7
mov eax,ecx
cdq
idiv ebx
```

After (P5, dividend in EBX, quotient in EAX, remainder in EBX):

```asm
mov ecx,92492493h
mov eax,ebx
imul ecx
mov eax,edx
add eax,ebx
mov ecx,eax
shr ecx,31
sar eax,2
add eax,ecx
mov ecx,eax
shl ecx,3
sub ecx,eax
sub ebx,ecx
```

`tools/stages.py --cpu P5 --dump DIR` records the selected backend stages.
Remaining: simplify quotient/remainder consumers in MIR, and make the
current MIR power-of-two division expansion participate in the comparison
rather than irreversibly expanding before lowering. Selection is still
conservative about register copies and does not model allocation spills or
full superscalar scheduling. Other profiles and the multiply-chain selector
still need the remaining timing audit; see `timing-audit.md`.
