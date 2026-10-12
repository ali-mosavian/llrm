# Optimizations

One note per optimization, each measured on a program. For contributors to `crates/opt` and the backend.

| Note | Subject |
|---|---|
| [cse](cse.md) | What CSE needs, measured on a long-arithmetic loop |
| [exact-floating-store-reuse](exact-floating-store-reuse.md) | An exact float store as a provider |
| [finite-array-reuse](finite-array-reuse.md) | Sharing a loaded array element |
| [floating-pop-reuse](floating-pop-reuse.md) | Preserving popping arithmetic operands |
| [constant-arguments](constant-arguments.md), [constant-call-memory](constant-call-memory.md), [constant-index](constant-index.md), [unit-step-ranges](unit-step-ranges.md) | Constant propagation |
| [whole-arguments](whole-arguments.md), [float-integer-values](float-integer-values.md), [numeric-argument-escape](numeric-argument-escape.md) | Argument raising |
| [lsr](lsr.md), [induction-counter-reuse](induction-counter-reuse.md), [invariant-shift](invariant-shift.md) | Loop strength reduction |
| [unrolling](unrolling.md) | Bounded unrolling |
| [local-dead-values](local-dead-values.md) | Dead values before an opaque reader |
| [harr](harr.md) | A BC object before and after |
