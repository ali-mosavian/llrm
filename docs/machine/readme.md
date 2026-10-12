# Machine

What the x86 backend must know about real-mode hardware, for contributors to isel, allocation and the targets.

| Doc | Holds |
|---|---|
| [targets](targets.md) | The targets built and how a new one is added |
| [a32](a32.md) | 386 addressing in real mode behind the `67h` prefix |
| [metal](metal.md) | The `66h` prefix and what only hardware can answer |
| [pointer-lowering](pointer-lowering.md) | Whole-pointer offset lowering |
| [arithmetic-targets](arithmetic-targets.md) | CPU-dependent constant arithmetic |
