# x86 Os inline trial: build the calls the size guard refused, keep what pays (scoping, no code, 2026-11-25)

Class: at `-Os` the SZ1/D11 size guard (`inline_size_guard_at_size`) keeps a call the pin's `OptSizeThreshold` 50 would inline, when the units say the code grows. The module is then smaller than the best bar and slower on a hot path. Row `QB-OS-STRINGS-INLINE` (owner x87, exit 2027-01-31) is one instance; no `strg_*` name appears below.

## Evidence (m64 Os, QB `string.c`, same build, only the compiler differs)
- String allocation `a$ + "x"` over a literal store: ours 251, pin clang 222 instructions (+13%), perf and callgrind agree. `bench_test.py` ALLOC_BUDGET 228 is an arm64-macOS guard, not this comparison.
- Raw: pin clang's IR has 0 calls of `strg_get`, ours 7; the two small callees are inlined by clang, out of line in ours (59 instructions per pass).
- Bytes: `string.o` .text ours 2,878, clang 3,284, gcc-15 3,482 (-12.4% under the best bar). Only clocks break the Os rule.
- Instructions are not cycles: the +13% is unproven as time (see protocol).

## 1. What the pin does, and the deviation
- Pin: the inliner's `OptSizeThreshold` 50 (+ `inlinehint` bonus) decides on `InlineCost` alone; there is no second compile. Its cost model prices the callee after the pin's own simplification, not the finished object.
- Ours: the size guard refuses growth; arm64 and m68k already build the refused sites inlined in a second compile and keep the finished code that prices better: DEVIATE OS-INLINE-TRIAL / OS-INLINE-TRIAL-CLOCKS (`Tti::opt_size_inline_finished_trial` + `..._band`, `walk.rs` marks the site as a `trial::Site`, driver compiles again with `with_inlined(sites)`).
- x86: new DEVIATE `OS-INLINE-TRIAL-X86` (owner x87, exit 2027-01-31) = enabling that same mechanism; no new pass. It is the inverse of the existing `OS-OUTLINE-TRIAL` (x86 driver, `Dimension::OutlineCalls`).

## 2. Mechanism (reuse, not new)
- `Tti::opt_size_inline_finished_trial()` true and a band (`..._band`, percent of the caller's bytes the model's growth may be; arm64 uses 5) in the x86 `Tuning`, per target (m32, m64) like `os_trial_clock_cap_percent`.
- `walk.rs` already records the refused site for any target whose Tti says so; the x86 driver (`llrm-codegen/src/driver/trial.rs`, `compiled_module_at_size`) gets one new `Dimension` (`InlineRefused`) next to `KeepHotCalls`/`OutlineCalls`: base compile notes the refused callees (`Noted`), the variant inlines them (`trial::with_inlined`), both go through `built`, the verdict is the finished-object one.
- General over any callee the guard refused; no names.

## 3. Pricing
- Verdict on the finished modules (as `OutlineCalls`): variant kept when it saves clocks at a price per added byte below `speed_inline_milliclocks_per_byte` (x86 already states it for O1/O2 speed inlining), and the clock model is `backend/executed.rs` (`work`/`executed`: block frequencies from `analysis::frequency`, proven trip counts, callee weights) over the differing functions only.
- Byte slack: the compiler cannot see the bar. The guard on the byte side is the band (module growth at most N% of the module's bytes, start 2%, tuned by the A/B) so the Os rule (bytes best) is held by construction and each cell above the bar gets a dated row. m68k's MBFI weights (#10808) are the same idea applied to a flat item list; x86 already has CFG weights, nothing to port.
- Clock cap: reuse `os_trial_clock_cap_percent` semantics inverted: no variant that adds work.

## 4. Fail-first tests (class level)
1. A module with a refused callee of 2+ sites on a hot path (callee 3 loads + store, caller called in a loop of known trip): `-Os` keeps the calls on base, inlines with the trial; asserted on the finished text (calls gone, the clocks priced by `executed` lower). Red on base.
2. A cold site stays a call: same module, caller executed once: variant not kept (bytes bigger, clocks equal).
3. Verdict is by finished object: a callee whose copies fold (units refuse, bytes shrink) is kept; one whose copies do not fold and growth passes the band is refused.
4. md5 of the other 6 targets unchanged (the Tti flag is x86-only).

## 5. A/B plan
- QB 126 hostperf objects x {m32, m64} x Os (and O2 for md5: no change expected): module bytes (none above the best bar or a dated row), the number of modules that change, trial verdict counts before/after.
- Clocks: `tools/analysis/qbclock.py bench` plus the `bench_test` alloc/frame/dyn cells, ours vs clang vs gcc, base vs head.
- Other 6 targets: md5 of their 126 objects unchanged. Compile cost: instructions of the compile (perf `instructions:u`) of the 10 largest modules; `llrm-codegen --lib`, `llrm-x86 --tests` in debug before a head.

## 6. Measurement protocol (cycles, not instructions)
- Quiet box (no cargo, no qemu running; `uptime` load < 1), CPU pinned (`taskset -c N`), governor performance, same binary pair built in one session.
- `perf stat -e cycles:u,instructions:u -r 11` per binary per program, runs interleaved A,B,A,B... (never all A then all B); report the median and the min of cycles; accept a delta only if the A and B ranges do not overlap.
- Programs: `r_N`, `q_N` loops (100k and 200k, difference per pass) for the alloc cell; the QB bench set for the rest.
- Reported per cell: instructions delta and cycles delta separately; the instructions-only +13% is not stated as a clock claim.

## 7. Compile cost
- One extra full backend compile for a module with at least one refused site (like OS-OUTLINE-TRIAL, reference +10% of compile instructions on those modules); modules with no refused site pay one extra walk flag. Cap the candidate callees per module (`..._max_callees`, as outline) so a module of many refused callees is not tried. Expected whole-corpus cost under +3%; measured in the A/B.

## 8. Risks
- A trial that spends slack under a bar it cannot see can move cells above gcc/clang: the band is the guard; each above-bar cell is a row (owner x87).
- Verdicts rest on modelled clocks: the cycles protocol above decides whether the model is right before the head goes up.
- Row `QB-OS-STRINGS-INLINE` stays until the trial lands; the stale `strg_gc` comment of `bench_test.py` (QB side) is folded into the next real head, as ruled.

## 9. Milestones (3-4 days)
D1 count refused sites per module over the 126 (how many modules would be tried), tests 1-3 red; D2 Tti + Dimension wiring; D3 A/B bytes + cycles; D4 PR (Pass: / Translation: / General solution: lines).
