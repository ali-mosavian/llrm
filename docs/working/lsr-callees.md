# LSR callees against the pin, same IR — w-3, 2026-10-22

d_faces -O2 compiled WITHOUT `-fno-inline-functions` (the bench flag set has it: with it LSR is 19.2 M ours on the same tip and 12.84 M for the pin, 1.50x; w-5 on tip 911a555d8 measures 16.79 against 12.84, 1.31x, `--toggle-collect` on the pass in both). The table below is the no-flag input: ours by callgrind of the whole compile on parity 209933e85, the pin's by `opt -passes=loop-reduce` on the same module. Inclusive Ir. The ratios of the buckets stand; the totals depend on the input and the tip.

| bucket | ours | pin | note |
|---|--:|--:|---|
| `LoopStrengthReduce::run` | 25.06 M | 17.08 M | 1.47x |
| `IvUsers::of` / `IVUsers` | 5.55 M | 3.41 M | 4.7 of 5.5 M is `get_scev` first builds, 18.5 K a call against the pin's 16 K; our walk visits ~1.7x the users (470 against 277 calls), unexplained, under 2 M |
| `generate_all_reuse_formulae` | 7.12 M | 4.78 M | below |
| `implement_solution` | 4.29 M | 4.12 M | `expand` 4.22 against 3.93 M: parity |
| `get_exact_sdiv` (all callers) | 1.20 M | 1.23 M | parity |

`generate_all_reuse_formulae` is 2.3 M over: `generate_icmp_zero_scales` 2.53 M of which `get_mul_expr` 1.05 M (190 calls), `generate_cross_use_constant_offsets` 1.72 M of which `get_add_expr` 0.83 M (154 calls). What is over is building new SCEV nodes (`get_add_expr`, `get_mul_expr`: ~1.9 M), the ScalarEvolution core (w-5), not the formula loops. Nothing to change in `lsr_*.rs` for a measured win.
