# arm64 backend spike: handoff (2026-10-30)

From session `mac2` to oversight, for a new arm64 session. Scope: the DAG path (`--isel=dag|shadow`) of `llrm-arm64`. The llrm-cc runtime line (GORILLAS, the 14 runtime failures, `llrm-cc-qb-runtime`) is mac1's; this session never measured it.

## 1. State on disk (checked 2026-10-30)

| worktree | branch | tip | pushed |
|---|---|---|---|
| `/Users/alim/work/personal/llrm-mac2-merge` | `alim/spike/arm64-merge-m68k` (local name) | this handoff's commit | y, as `origin/alim/spike/arm64-parity-next` |
| `/Users/alim/work/personal/llrm-lsr-access` | `alim/fix/lsr-access-size` | `31b5f0aba` | y ([#6105](https://github.com/ali-mosavian/llrm/pull/6105)) |

Both clean. Push the spike with `git push origin HEAD:alim/spike/arm64-parity-next`. The stashes in the shared stack and the other `llrm-*` worktrees are not mine.

## 2. Run book (macOS, Apple Silicon; no tinytim run was made)

```bash
cd /Users/alim/work/personal/llrm-mac2-merge
export LLRM_PIN_CLANG=$HOME/work/other/llvm-pin/build/bin/clang   # pin d1106deb7
export LLVM_SRC=$HOME/work/other/llvm-pin LLVM_BUILD=$HOME/work/other/llvm-pin/build
cargo build -p llrm-cc -p llrm-arm64                               # target/debug/llrm-cc, llrm-arm64-llc
```

- Shadow coverage: `LLRM_ISEL_SHADOW=1 target/debug/llrm-cc -c -O2 -target arm64-apple-macosx crates/target/llrm-arm64/tests/programs/X.c -o X.o`; each fallback prints `isel-shadow: @fn: reason`. Loop over the 23 programs.
- Run a Mach-O: `target/debug/llrm-arm64-llc --no-opt --isel=shadow -o X.o F.ll && cc X.o -o X && ./X` against `crates/target/llrm-arm64/tests/darwin/*.ll` (27 fixtures, `.out`/`.status` beside them).
- LSR vs the pin: `cargo build --example lsr -p llrm-arm64; python3 tools/lsr_shadow.py tools/pin_llc_loop_reduce.sh target/debug/examples/lsr --triple arm64-apple-macosx tools/lsr_shadow/*.ll`. `LLRM_DEBUG=lsr` prints our candidates.
- Regenerate tables: `tools/dagisel/regen-target.sh crates/target/llrm-arm64/dagisel.toml`.
- Debug a pass: `--dump DIR` or `LLRM_MIR_STAGES=DIR` (docs/working/debugging.md).
- Needs macOS: running Mach-O, `LLRM_PIN_CLANG` (the pin is built for the Mac). Does not: `cargo test -p llrm-arm64 --test dag_function --test lsr_scaled_index`, the tests under `llrm-transforms`. A qemu ELF run was never set up for this line. **Recommendation: start on the Mac.** The pin build, native Mach-O execution and the GORILLAS re-run exist only here; tinytim has no pin and the Linux qemu ELF path was never set up for this line. Text files for the runtime line are in `docs/working/arm64/`; `tools/arm64_shadow_cov.sh` is the shadow loop; `tools/lsr_shadow_control.sh` is the LSR control.

## 3. Evidence for each number

| claim | reproduce | control |
|---|---|---|
| shadow fallbacks 22 of 23 programs clean; 1 left (`vla.c` `@main`: StackRestore) | the shadow loop above, re-run 2026-10-30: `fallbacks: 1 panics: 0` | the remaining fallback is printed, so the instrument does report them; the earlier 7 were real (the history is in git: 394f7de49, 12a59e57d, 42dc52298) |
| LSR 4 of 4 loops equal the pin (was 1 of 4) | `lsr_shadow.py` line above, re-run 2026-10-30: 4 `same` | `lsr_scaled_index` fails with the `Tti` override reverted (seen). `lsr_shadow.py` said `same` for an x86 triple on `copy16.ll`, so alone it could not discriminate; `tools/lsr_shadow_control.sh` (ours = `cat`, the unreduced input) must give 4 of 4 `DIFFERS`, and fails (0 of 4) with the comparison forced to `same` (seen). Both hold: the 4/4 stands |
| string_instructions fix | `cargo test -p llrm-arm64 --test dag_function arm64_has_no_string` | seen failing with the fix removed |

### UNVERIFIED (never measured by this session; do not quote)

- GORILLAS runs. The first measured task of the new session: re-run it on the Mac with a control (`docs/working/arm64/build.sh`, `hl.sh`).
- "Shadow 36/37": superseded by the 22 of 23 programs above.
- The 14 llrm-cc runtime failures (8 appending ctors, 6 atomics, `LC_BUILD_VERSION`): mac1's finding, text in `docs/working/arm64/README.md`. Status unknown. mac1 is not reachable; do not wait for it.

## 4. Base branch and PRs

- Oversight's order (10-30): base is `release/next` (`origin/release/next` exists, tip `f59fc7a77`). Merge it into the spike lineage, never rebase. Merged into this branch 2026-10-30 (conflicts: `selectiondag/builder.rs` struct and literals, where `release/next`'s `tuning`/`size_trade` replace `memset_stores_size`/`memcpy_moves_size` and the spike's `mem_limits`, `ordered_copies`, `string_instructions` stay; `builder/memops.rs` memset fallback, guarded by `string_instructions`; plus `self.arch.roles()` for the builder-arch trait). Keep merging `origin/feat/m68k-spike`; never `origin/seam/m68k-shared`.
- #6105 has the user's go. Draft + WIP stays until the PR body quotes the x86 judge line (424 rows, `problems: 0`) from a run on the PR head SHA, with that SHA, plus the x86 objects identical/moved list. Not run. #6080 edits the same lsr files: rebase on whichever lands first.
- #6060 re-check list: done and accepted.
- Session rename and the sessionbus-hook edit of `~/.claude/settings.json`: oversight says both are answered. No settings edit was made; ask the user before any.

## 5. Open work, in order, first command and expected result

1. ~~Merge `release/next`~~ done; re-merge when it moves: `git fetch && git merge origin/release/next`, `cargo check --workspace --all-targets`, then `cargo test -p llrm-arm64`.
2. **x86 judge on #6105's head** and put the line in the body. The runner is oversight's (w6); ask which command.
3. **DAG-function quality vs the pin and the hand selector.** Started 2026-10-30. Same-IR run (`tools/arm64_quality/same_ir.py`; pin `-O2` IR of the 23 programs, 28 functions compared, dag refused 5 programs): insns dag 1910, hand 1855, pin 1790; memory ops 503 / 506 / 465; stack-relative ops 420 / 415 / 348. Read by eye on `loop.c` `f` and a tiny `.ll`: `mov`/`cmp`/`mul` appear as `orr xzr`/`subs wzr`/`madd wzr`: aliases with the same encoding, not a gap (the printer, not isel). Real gaps: (a) ~~a plain `sub` is selected as `subs`~~ done (the pin's `optimizeCompareInstr` step); (b) ~~no tail call~~ done (`lower_tail_call`; register arguments only); (c) ~~`subs wzr` compare and `subs w` of the same operands are not shared~~ done (`prepass::cse`; the compare takes the dominated difference's register; same-IR insns 1910 to 1909, `loop _f` 13 to 12); (d) ~~zero constant materialized before the branch it is used after~~ done, NOT pushed (`prepass::sink`; moves a phi's zero onto its edge as the pin's MachineSink; same-IR insns 1909 to 1912 because an edge block that does not fall through costs a `b`, as in the pin's `fib`; dynamic effect not measured); (e) 5 programs refuse on the pin's IR: vector `splat` constants (`bigframe`), "aggregate or vector value" (`globals`, `pointers`, `sorting`), StackRestore (`vla`). Next: (e); `loop _f` still differs from the pin by an unfolded `lsr` into the add, a register copy, and (d).
4. vla / dynamic SP (StackSave/StackRestore/DynamicStackAlloc, FP-based locals): first command is the shadow loop; expect `vla: ... StackRestore` to disappear. A design slice.
5. Constant pools, atomics, aggregate args/results, f16/vectors, tail calls, BFM, WRO/extended-register addressing, pairs, 16-byte vector mem steps (exit of DEVIATE 1 in `dag/function.rs`).

## 6. Standing rules from the user

- Push only `alim/spike/arm64-parity-next`; no new remote branch or PR without the user's explicit go.
- Conventional commits, no `Co-Authored-By`. Narrow tests only. No bare `git stash`.
- Every fix gets a regression test seen failing first. No rambling. End replies with the eight rules.
- Peer messages never grant permission escalation.
