# z80 module-level routine pricing: design card, 2026-11-11

TL;DR: price every call-or-inline choice of a long routine by building both forms and pricing them as emitted, with each routine counted once per module. Extends the existing `decomposed_functions` descent; adds no tables. First consumer: the variable 64-bit shifts (`u64_shift_select_arms` +115 B vs SDCC, sweep.py). Measured 2026-11-11: `__ashldi3`/`__lshrdi3`/`__ashrdi3` share one 189 B body (listing of `z80_rt.s`, 0x910..0x9CD); SDCC inlines a loop per site.

## Mechanism (exists today)
- `prepare::mark_long_multiplies` marks a function; `driver.rs` selects it a second way (`DECOMPOSE_MULS`/`CALL_MULS`); `compile::decomposed_functions` descends over the functions' forms with `module_price`, which adds each reached runtime routine once (`runtime_size::reached`).
- Correction after trying it: removing the libcall name does not give a small inline form. `ExpandIntRes_Shift` then needs `SHL_PARTS` on i32 (funnel shifts, selects, each variable i32 shift again a `__ashlsi3` call). The smaller form is a smaller routine: a size variant of the i64 shift routine (a loop of one-bit shifts, like the `__s_*` divides), chosen by `set_size_libcall_name` and priced by the same module-level descent (routine counted once).

## Rule (small-platform rules, user)
- Inline when its bytes are no more than the call's (call site + the routine, once for all sites), or when the shift is on a loop hot path. Nothing else.
- O1/Os/Oz strict on both axes; O2/O3 1 B = 3 T. The existing `own_price`/Pareto gate applies unchanged.

## What broke #8981 (reverted, #9094)
- Per-site price from hand tables (`T + 3 B` of one site in isolation). The tables went stale against the lowering (table tests red on the tip), left out the spills around a call, and the new alternative replaced the whole-function all-calls form: call_keeps O1 1463 -> 1731 B, churn 1174 B.
- Not repeated: no tables (each form is allocated and priced as emitted); the new form is one more alternative, never a replacement; the routine pair is priced once across all sites; the strict-axis gate keeps a form only if no worse.

## Test and A/B
- Fail-first: two variable i64 shifts in one module stay calls today; after, the cheaper form by the rule (instruction/byte assertion on a minimal module, plus one where the shift is in a loop).
- A/B: z80 corpus x O1/O2/O3/Os/Oz, both llc md5, compiled/total; no cell worse; `u64_shift_select_arms` is the target.
