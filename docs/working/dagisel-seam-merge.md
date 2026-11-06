# dagisel seam: m68k and mac2/aarch64 reconciliation (m68k side, 2026-10-28)
Branch to merge from: origin/feat/m68k-spike (pushed; commits 4c8c9eba56 ... 5855a1c088). Same note in the repo: docs/working/dagisel-seam-merge.md.

## What m68k landed in llrm-dagisel(-gen) (conflict surface)
- gen `emit.rs`: `Style` struct (paths, ns prefix, title, regen script, desc fn); x86 uses `Style::X86` (output byte-identical).
- gen `main.rs`: `--target RECORD` dispatches to new `target.rs`; `ids_rs.rs`: `render_target`; `inc.rs`: no-complex-pattern table; wasm's `099067a02f` (xforms omitted, CheckTypeRes) cherry-picked in `decode.rs`/`matcher.rs`/`interp.rs`.
- `llrm-dagisel/src/x86_hooks.rs`: `find_non_imm_use` is `pub`. No other llrm-dagisel edit.
- `tools/dagisel/`: `regen-target.sh`, `ids_target.cpp.in`, `instrinfo.py` (optional namespace arg). `llrm-x86/src/dag/actions.rs`: `init_actions` is `pub`.

## Proposed merge order
1. mac2's P1 (llrm-sdag: MVT/ISD/TargetLowering out of llrm-x86) lands first; m68k then changes two lines of Cargo.toml and its imports (`llrm_x86::dag` -> `llrm_sdag`). SD500 row goes.
2. mac2's P3 (X86Predicates split of Hooks) replaces m68k's no-op methods in `hooks.rs` (has_sse_unaligned_mem, is_def32, ...): m68k deletes them.
3. mac2's `--target aarch64` and m68k's `--target RECORD` are one flag: keep the record (`crates/target/llrm-<t>/dagisel.toml`) as the only input; aarch64 becomes a record. Where main.rs/emit.rs conflict, keep m68k's `Style` and `target.rs`, port aarch64's extras into the record fields.
4. Proof: `tools/dagisel/regen.sh` x86 output diff -rq clean, plus `tools/dagisel/regen-target.sh crates/target/llrm-m68k/dagisel.toml` clean (stale test `llrm-dagisel-gen/tests/stale_targets.rs`).

## Open for mac2
- Size policy for AArch64's 10.5 MB table; complex patterns with template args are carried as text in `COMPLEX_PATTERNS`.
