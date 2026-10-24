# x_bubble zero-end counter generator: trial (2026-10-22)

Tried the plan's generator step (`generate_zero_end_counters`, DEVIATE LZ1): after `GenerateScales`, an ICmpZero formula `reg(B) + -1*reg(IV)` gains `reg(IV - B)`, and every use of IV gains `reg(a + s*B) + s*reg(IV - B)`. Patch: `docs/working/count-to-zero-xbubble-trial.patch`.

Result (m32 -O2, tools/rows/row.sh, base 911a555d8 dist, same machine model):
| program | clocks base -> trial | code B |
|---|--:|--:|
| x_bubble | 88652 -> 96525 (+8.9%) | 280 -> 296 |
| queens | 174716 -> 169587 (-2.9%) | 296 -> 312 |
| quicksort | 193619 -> 193322 (-0.2%) | 1260 -> 1468 |

Why x_bubble is worse: the model picks the zero-end counter for the fill loop (`inc eax; jne`, no `cmp`) but the address becomes `_a[edx+esi]` with `mov esi,eax; shl esi,2` each trip. Our isel does not fold `add(x, shl(y,2))` under a symbol into `[sym+x+y*4]`; the model prices the scaled index as foldable. The stride-4 counter that main's loop uses (`add 4; je`, address base + counter) is not among the formulas: it needs the ICmpZero scale-by-4 formula paired with the address's `reg({0,+,4})`, which the step above does not pair (the address is `reg({0,+,4})` alone, no scaled register).

Dependency: x_bubble waits for SelectionDAG C1 (sdag-c's `matchAddress`), the translation of the fold below; the old isel is not patched. sdag-c is asked to add x_bubble's fill loop to the select shadow and report whether `add(x, shl(y,2))` under a symbol selects `[sym+x+y*4]`. The generator stays unapplied until then.

Next, in order: (1) the address fold, from C1; (2) pair the stride-4 forms in the generator; (3) rerun rows. Owner w-5, by 2026-11-06.

sdag-c (2026-10-22): `@_a + (x + (y<<2))` and `(@_a + x) + (y<<2)` both select `[_a + x + y*4]` in the C1 matcher (unit test `symbol_plus_two_registers`, #3779); the real _bench_x_bubble is not through the shadow yet, refused at the type legalizer (SetCC i1 promote, sdag-b).
