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

## What the fields mean

- `bytes` and `instructions` cover the exact selected function body after
  branch relaxation.  Relocation fields are normalized to zero, as they are in
  a fresh object before linking.
- `weighted_cost` is a ranking from the selected CPU profile.  A missing form
  makes it `null`; it is never silently assigned zero.
- loads, stores, branches, calls, address calculations, peak live values, and
  allocator spill markers are structural counts.  They are not elapsed time.
- rematerializations count the final allocated-LIR instructions that rebuild a
  value at its use instead of keeping it live or assigning a spill slot.
- dynamic operations are a profile-free CFG estimate: ordinary branches divide
  evenly and natural loops use the allocator's ten-iteration convention. Calls,
  interrupts, repeated instructions, irreducible control flow, and nonterminating
  estimates remain explicitly unmeasured because their hidden work is unbounded.
  A `null` here is not zero.

The report always writes raw qbopt assembly.  With `--references`, it also asks
Clang and `i686-elf-gcc` for freestanding i386 `-O3` assembly with SSE and
vectorization disabled.  GCC's reported target is checked before it is used.
A compiler without an i386 backend is reported as failed or unavailable;
output from the host architecture is not substituted.  Reference assembly is
advisory and does not become the target automatically.

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
