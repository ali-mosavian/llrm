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

Remaining: apply target-aware selection to reciprocal division and its
remainder reconstruction. Division has not yet been changed by this work.
The current MIR power-of-two division expansion also needs to participate
in that comparison rather than irreversibly expanding before lowering.
