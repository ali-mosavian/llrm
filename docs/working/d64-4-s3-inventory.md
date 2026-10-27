# D64-4 S3: float programs for tests/llc and the pin's forms

Plan: [x86-64-dag-plan](x86-64-dag-plan.md) (D64-4, slices S1-S5). These eight programs are S5's corpus, written first so S3 has a target.
Each prints integers only (floats scaled and cast), so S4's `printf("%f")` is not needed. The pin's gcc and clang -O2 agree on all eight.

Pin forms per program: `clang -target x86_64-linux-gnu -O2 -fno-vectorize -fno-slp-vectorize -S tests/llc/P.c` (scalar only: vectors are deferred).

| Program | Exercises | Forms the pin uses |
|---|---|---|
| fsum | f32/f64 loop sums, int to float, float to int | addsd addss mulsd mulss cvtsi2sd cvtsi2ss cvttsd2si cvttss2si movsd movss xorpd xorps |
| fcmp | every compare incl. NaN, select on a compare | ucomisd ucomiss cmpeqsd cmpneqsd cmpneqss cvtsd2ss divsd movd movq |
| fconv | int/long/unsigned to f32/f64, back, f32 to f64 and back | cvtsi2sd cvtsi2ss cvtsd2ss cvtss2sd cvttsd2si cvttss2si divsd divss |
| fnbody | struct arrays, sqrt, a double loop | addsd subsd mulsd divsd sqrtsd movapd movsd |
| fneg | negate, fabs, min/max, sqrt of f32 and f64, -0.0 | xorps xorpd andps andpd minss maxsd sqrtss sqrtsd ucomiss ucomisd |
| fcall | f32/f64 args and returns, mixed with ints, 9 f32 and 10 f64 args (stack), a value live across calls | addsd addss mulsd mulss subsd cvtsi2sd cvtss2sd movss movsd movq |
| fconst | pool constants: 0.0, -0.0, 1e300, denormals, f32 extremes | movsd movq cvttsd2si subsd |
| fselect | float loop counter, select on a float compare | ucomisd addsd addss mulsd mulss cvtss2sd xorps |

Not in the corpus: packed forms (the pin vectorizes fnbody without the flags above), long double (x87), `al` for variadics (S4).
Run: `tools/llc_check.sh fsum fcmp fconv fnbody fneg fcall fconst fselect` (now links `-lm`).
