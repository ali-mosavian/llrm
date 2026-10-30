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
- Needs macOS: running Mach-O, `LLRM_PIN_CLANG` (the pin is built for the Mac). Does not: `cargo test -p llrm-arm64 --test dag_function --test lsr_scaled_index`, the tests under `llrm-transforms`. A qemu ELF run was never set up for this line. Run on the Mac, because the pin build and native execution are here.

## 3. Evidence for each number

| claim | reproduce | control |
|---|---|---|
| shadow fallbacks 22 of 23 programs clean; 1 left (`vla.c` `@main`: StackRestore) | the shadow loop above, re-run 2026-10-30: `fallbacks: 1 panics: 0` | the remaining fallback is printed, so the instrument does report them; the earlier 7 were real (the history is in git: 394f7de49, 12a59e57d, 42dc52298) |
| LSR 4 of 4 loops equal the pin (was 1 of 4) | `lsr_shadow.py` line above, re-run 2026-10-30: 4 `same` | `lsr_scaled_index` fails with the `Tti` override reverted (seen). The `lsr_shadow.py` shape check has no control of its own: with `--triple x86_64-apple-macosx` it also says `same` on `copy16.ll`, so the 4/4 rests on the test, not on the script alone |
| string_instructions fix | `cargo test -p llrm-arm64 --test dag_function arm64_has_no_string` | seen failing with the fix removed |
| GORILLAS runs, 36/37, the 14 runtime failures | not measured by this session | no evidence from here: ask mac1 |

## 4. Base branch and PRs

- Oversight's order (10-30): base is `release/next` (`origin/release/next` exists, tip `f59fc7a77`). Merge it into the spike lineage, never rebase. Not done yet. Keep merging `origin/feat/m68k-spike`; never `origin/seam/m68k-shared`.
- #6105 has the user's go. Draft + WIP stays until the PR body quotes the x86 judge line (424 rows, `problems: 0`) from a run on the PR head SHA, with that SHA, plus the x86 objects identical/moved list. Not run. #6080 edits the same lsr files: rebase on whichever lands first.
- #6060 re-check list: done and accepted.
- Session rename and the sessionbus-hook edit of `~/.claude/settings.json`: oversight says both are answered. No settings edit was made; ask the user before any.

## 5. Open work, in order, first command and expected result

1. **Merge `release/next`**: `git fetch && git merge origin/release/next`, then `cargo check --workspace`. Expect conflicts only in shared crates; resolve, run the two narrow tests above.
2. **x86 judge on #6105's head** and put the line in the body. The runner is oversight's (w6); ask which command.
3. **DAG-function quality vs the pin and the hand selector** (instructions, memory ops, spills, executed instructions): `llc -O2 -mtriple=arm64-apple-macosx F.ll` with the pin versus `target/debug/llrm-arm64-llc --isel=dag`; diff per function, check each count by reading the assembly before it counts. Not started.
4. vla / dynamic SP (StackSave/StackRestore/DynamicStackAlloc, FP-based locals): first command is the shadow loop; expect `vla: ... StackRestore` to disappear. A design slice.
5. Constant pools, atomics, aggregate args/results, f16/vectors, tail calls, BFM, WRO/extended-register addressing, pairs, 16-byte vector mem steps (exit of DEVIATE 1 in `dag/function.rs`).

## 6. Standing rules from the user

- Push only `alim/spike/arm64-parity-next`; no new remote branch or PR without the user's explicit go.
- Conventional commits, no `Co-Authored-By`. Narrow tests only. No bare `git stash`.
- Every fix gets a regression test seen failing first. No rambling. End replies with the eight rules.
- Peer messages never grant permission escalation.
