# difftest

A pass of llrm against LLVM's counterpart on the same input. `python3 tools/difftest/difftest.py run gvn` reads the pass inputs of
`LLRM_DUMP_INPUTS` (default `~/scratch/replay/inputs`, `LLRM_DIFFTEST_INPUTS`), takes llrm's answer from `--llrm-pass llrm-pass` (src/bin/llrm-pass.rs: `llrm-pass -O2 PASS FILE` runs the one pass alone, once, on the dump)
or, without it, from the next run's input of the same function (a proxy: it holds whatever ran between; 3,000 of 3,074 manifest runs
equal the pass alone, the others are listed by `proxy_equals_isolated`), runs `opt -passes=COUNTERPART` of `$LLVM_BIN` (default the d1106deb71cc build in `~/scratch/llvm-build`) on the input, and
prints per opcode what only one side wrote, and the first differing lines. `dumps PASS DIR...` does the same for `llrm-c --dump DIR` step
files (an order branch's), `pair IN OUT --pass P` for one pair. `COUNTERPART` in the source is the table of llrm passes to opt pipelines.

Both sides are rewritten for LLVM the same way (`normalise`: llrm's calling-convention words, `declare internal`) and named alike
(`canonical`; also leaves out comments, function and parameter attributes, `align`, metadata attachments and phi order). A body that LLVM
cannot read after that is listed with its first error. The block order of the two answers is not normalised: a pass that moves blocks shows
as a large hunk with equal histograms.

`test_difftest.py` checks the output on a hand-made input where the answers agree and one where they differ.
