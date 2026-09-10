# Current target coverage

Measured **2026-09-10**, compiler revision **b126fb4**, with
`uv run python tools/opportunity.py --targets` over all **487 objects** in
`fixtures/omf` (**34 source programs**). These are model-weighted instruction
costs, including configured helper costs—not hardware timings. Default loop
weighting is ten iterations per nesting level, capped at three levels.

| Status | Configurations |
|---|---:|
| Comparable and within 1.5x | 289 |
| Comparable and above 1.5x | 0 |
| Provisional reference | 102 |
| No target | 96 |
| Unmeasured / refused emission | 0 |

**The goal is not complete.** The gate returns 1. Only 289/487 rows currently
have comparable references; this is coverage, not a project-completion
percentage. The architecture checklist remains independently binding.
Comparability here is the scorer's classification, not a new independent
audit of every registered reference. The 102 provisional rows comprise
78 event builds, 12 ordinary FPCSEX builds and 12 ordinary FPDEEP builds.
Recent regression fixtures in `fixtures/regressions` are outside this
default scan and are not implied covered by these totals.

## Largest comparable gap

IVCHAN is worst at **1.36x**: QB and VBDOS plain cost 762 against 560.
Other ordinary IVCHAN variants cost 756 (**1.35x**). NESTED reaches
1022/768 (**1.33x**). No comparable row exceeds the requested threshold.

## Floating-point cases

| Program | PDS cost | QB cost | VBDOS cost | Reference status |
|---|---:|---:|---:|---|
| FPCSE | 157 | 145 | 157 | Complete; all three are 1.00x |
| FPCSEX | 4462 | 4467 | 4452 | Provisional: 1340 reassociates additions and omits SINGLE rounding |
| FPDEEP | 1777 | 1572 | 1777 | Provisional: 1086 lacks a complete checkpoint/store observability proof |

These rows use `p-g2`, `q-O`, and `v-g3`. Do not divide by the provisional
numbers to claim success or justify relaxing floating-point behavior.

## Missing references

| Source program | Configurations without targets |
|---|---:|
| CHAIN | 15 |
| CMPORD | 15 |
| DIVMOD | 15 |
| FPEMU | 15 |
| JUMPS | 15 |
| PROCS | 16 |
| CM | 4 |
| JT | 1 |

Names come from object source headers, not filename prefixes. Event-enabled
configurations with existing plain targets also need event-preserving
references; their plain-program denominator is not comparable.

## Next work

1. Derive and validate strict FPCSEX and complete FPDEEP reference listings;
   retain source-order rounding, pending exceptions and observable stores.
2. Add independently derived references for the eight uncovered programs
   and event-enabled configurations. Do not scale targets from emitted costs.
3. Use those validated gaps to prioritize implementation alongside the
   [architecture checklist](architecture.md#high-impact-mir-passes).
   Runtime-sized array extents, precise call effects, remaining GVN-PRE,
   loop transforms and backend work are not declared done by this scan.

This refresh changes no emitted assembly: **before = after**. It replaces
stale status claims, not compiler behavior. Earlier measurements and detailed
investigations remain in [the historical record](target-coverage-history.md).
