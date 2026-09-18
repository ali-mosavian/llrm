# Code-quality measurement

`tools/quality.py` is the common static report for C output.  It measures the
bytes selected by the fresh OMF path, records the source and normalized
assembly hashes, and emits the assembly used for every number:

```sh
uv run python tools/quality.py bench/c/*.c --cpu all \
  --references --dump build/quality --json build/quality/report.json
```

The committed C corpus is `bench/c/`.  Every entry takes at least one runtime
argument so a reference compiler cannot replace the benchmark with its known
answer.  `expected.json` records those arguments and the independently checked
answer.  In particular, CRC uses the standard `"123456789"` check vector and
must return `0xcbf43926` for a zero salt.

`tests/test_cbench_e2e.py` is the semantic gate for that corpus. It compiles
all seven sources through the Watcom frontend and fresh OMF emitter, links
them into one medium-model DOS executable, and compares every 32-bit return
value with `expected.json`. Per-kernel marker files identify the first routine
that fails to return without mistaking a timeout for a wrong answer.

## What the fields mean

- `bytes` and `instructions` cover the exact selected function body after
  branch relaxation.  Relocation fields are normalized to zero, as they are in
  a fresh object before linking.
- `comparison` contains structural counts after recognized ABI-only frame
  setup, callee-save traffic, and teardown are removed while one return remains.
  Raw totals above are never changed. GCC/Clang comparisons use these normalized
  counts so a 32-bit flat-ABI prologue is not judged against a 16-bit far one.
- `weighted_cost` is a ranking from the selected CPU profile. A missing form
  makes it `null`; `weighted_status` and `unpriced_forms` name the reason and
  the table prints it as `UNPRICED[...]`. It is never silently assigned zero.
- loads, stores, branches, calls, address calculations, peak live values, and
  allocator spill markers are structural counts.  They are not elapsed time.
- rematerializations count the final allocated-LIR instructions that rebuild a
  value at its use instead of keeping it live or assigning a spill slot.
- Structural transactions retain their candidate stage metrics for audit, but
  mark them `tentative: true`. Gap attribution ignores those entries until an
  explicit `*-accepted` state enters the production pipeline. This distinction
  matters when a rejected peel or unroll briefly looks better than the body
  ultimately emitted. Their final optimized state is recorded as
  `mir-{unroll,peel}-rejected-*`; the suffix names the deciding gate:
  `residual-loops`, `unpriced`, `no-saving`, or `growth`.
- dynamic operations are a profile-free CFG estimate: ordinary branches divide
  evenly; a canonical MIR loop with a proven finite count uses that exact count;
  all other natural loops use the allocator's ten-iteration convention. The
  same basic-block construction, natural-loop analysis and frequency solver run
  over GCC/Clang assembly, so an unrolled reference is compared with the work a
  qbopt loop executes rather than with qbopt's much smaller static body. Calls,
  interrupts, repeated instructions, unresolved or indirect branch targets,
  irreducible control flow, and nonterminating estimates remain explicitly
  unmeasured because their hidden work is unbounded. A `null` here is not zero.

The report always writes raw qbopt assembly.  With `--references`, it also asks
LLVM/Clang and `i686-elf-gcc` for freestanding i386 `-O3` assembly with SSE and
vectorization disabled.  Those listings are the **best-case structural
reference**: they show the loop shape, expression count and memory traffic a
modern optimizer chooses when it is free of the medium-model ABI.  GCC's
reported target is checked before it is used.  A compiler without an i386
backend is reported as failed or unavailable; output from the host architecture
is not substituted.

They are deliberately not ABI-equivalent targets.  The references use flat
32-bit i386 addressing; qbopt emits a 16-bit medium-model ABI with far calls,
segmented data, restricted address forms and different frame layout.  A lower
count in a reference therefore cannot erase required segment, far-pointer or
ABI work in our output.  Every JSON report and structural comparison now carries
this contract explicitly.  Reference assembly remains advisory until a
candidate-ABI target has been inspected and derived by hand.

The one-line reference comparison prefers estimated executed instructions when
both functions have a complete estimate and says so in the label. It falls back
to a separately labelled **static instructions** ratio otherwise. This matters
for the corpus: i686 GCC expands the constant CRC input into hundreds of static
instructions, while qbopt keeps an outer loop. The old static-only headline
reported qbopt at `0.13x`, flattering it by almost eightfold; the CFG estimate
reports `1.19x` executed work instead. Static size remains in the JSON report
and is still a separate acceptance dimension.

## Target trust boundary

`bench/c/targets.json` deliberately starts empty.  A target is accepted only
when it is marked audited, cites nonempty evidence, supplies positive metrics,
and has an assembly hash different from the candidate being measured.  The
quality gate fails both missing targets and ratios over 1.10:

```sh
uv run python tools/quality.py bench/c/*.c --cpu 386 --gate
```

Therefore `NO TARGET` is an unfinished obligation, not a passing score.  Add a
target only after reading both assemblies and deriving the target by hand;
retain the cited derivation beside the registered metric.
