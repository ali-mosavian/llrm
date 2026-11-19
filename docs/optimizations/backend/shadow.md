> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/shadow.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/shadow.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/shadow.rs sha256:df64933e886281a396eab7735b54b26355f7303ae1f2ba2460df688c4133ff2f -->

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

From `LLRM_DEBUG=isel llrm-cc -m32 -O2` of `int id(int x) { return x; }`.

## Used by
`assemble::staged` (the choice and the dump), `assemble::machined` (the
row), `driver::flags` (`--isel=`).

[selectiondag-plan]:
https://github.com/42labs/llrm/blob/main/docs/working/selectiondag-plan.md
