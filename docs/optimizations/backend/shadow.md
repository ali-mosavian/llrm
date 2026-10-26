> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/shadow.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/shadow.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/shadow.rs sha256:43e9f5cb9a3c086d688074f8d141f29b7117505d7b8abc7b33f192f1b0f0bb82 -->

# shadow

Which instruction selector runs, and the row each function leaves for
`tools/pairs/isel_shadow.py`.

## How it works
`--isel=old` (the default) is the MIR selector of `isel`; `--isel=dag` is
the SelectionDAG translation ([selectiondag-plan], group sdag-d). The DAG
path refuses every function until its first end-to-end piece lands, so the
harness counts refusals as it counts any other difference.

`LLRM_DEBUG=isel` prints one row per function once the route is chosen:
`[isel] NAME selector=old instructions=21 copies=3 executed=40 memory=12`.
`executed` and `memory` are `executed::executed`'s estimate (`-` where the
body has no finite one); `copies` are register-to-register `mov`s.
`LLRM_ISEL_REPORT=FILE` appends `NAME dag|old` per function: who selected
it. `ISEL_DUMP=1` also prints `=== ISel old` (or `dag`) right after
selection, before any machine phase.

## Example
A function of 3 instructions holding one register copy:

```text
[isel] id selector=old instructions=3 copies=1 executed=3 memory=0
```

From `LLRM_DEBUG=isel llrm-c -m32 -O2` of `int id(int x) { return x; }`.

## Used by
`assemble::staged` (the choice and the dump), `assemble::machined` (the
row), `driver::flags` (`--isel=`).

[selectiondag-plan]:
https://github.com/42labs/llrm/blob/main/docs/working/selectiondag-plan.md
