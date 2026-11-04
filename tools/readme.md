# Tools

Scripts and Python tools for building, running, measuring and comparing llrm. The list, by task, is
[docs/reference/tools.md](../docs/reference/tools.md).

Run Python tools with `uv run --project tools python tools/<dir>/<tool>.py`; each tool's header has its options.
Format Rust with `tools/fmt.sh`. `toolchain/` (at the repository root, not here) holds what `build.rs` bootstraps:
Open Watcom's `wccq`, `jwasm`, `jwlink` and DOSBox-X.

## The identity gate

`identity.sh` is how a change proves it left a target's code alone. Build the base (a worktree at
`origin/main`, with its own `CARGO_TARGET_DIR` so the build is kept), then:

```sh
tools/identity.sh nib  BASE/llrm-nib target/release/llrm-nib
tools/identity.sh c    BASE/llrm-c   target/release/llrm-c [-m32]
QCPORT=... QCPORT_INC=... tools/identity.sh qcport BASE/llrm-c target/release/llrm-c
```

It prints how many programs came out SAME, and each one that is not: `DIFF` (the objects differ),
`STATUS` (one build compiled what the other refused) or `REFUSED` (both refused: no evidence either
way). A DIFF is a finding, or a base older than the change's branch: compare against `origin/main`
as it is. `test_the_identity_gate_passes_a_build_against_itself_and_fails_a_different_one` (`tests/toolchain.rs`)
keeps the instrument honest.

## Compile-time measurement

`measure.py`, `compile-cost.py`, `abmeasure.py` and `vsgcc/scaling_gate.py` time one thing: a compile by
`llrm-cc`, which is clang's front end (`-S -emit-llvm`, no passes) plus `llrm-llc`, the driver's whole process tree.
The cost is user-space instructions (`perf stat -e instructions:u`, children included) with the binary on a
fixed-length path and a constant-size environment (`llrmbin.pinned`, `measured_environment`), so a loaded host does not
move it. A step's own cost is `llrm-llc`'s `LLRM_DEBUG=time` `[instr]` rows. A target is a row of `llrm-cc`'s table
(`llrmbin.c_args`); it needs `LLRM_PIN_CLANG`, `LLRM_OW` and, at 16 bits, `LLRM_CLANG_M16`.
