# difftest

A pass of llrm against LLVM's counterpart on the same input. `python3 tools/difftest/difftest.py run gvn` reads the pass inputs of
`LLRM_DUMP_INPUTS` (the directory `LLRM_DIFFTEST_INPUTS` names), takes llrm's answer from `--llrm-pass llrm-pass` (src/bin/llrm-pass.rs: `llrm-pass -O2 PASS FILE` runs the one pass alone, once, on the dump)
or, without it, from the next run's input of the same function (a proxy: it holds whatever ran between; 3,000 of 3,074 manifest runs
equal the pass alone, the others are listed by `proxy_equals_isolated`), runs `opt -passes=COUNTERPART` of `$LLVM_BIN` (default the pinned d1106deb71cc build, `tools/llvmpin.py`) on the input, and
prints per opcode what only one side wrote, and the first differing lines. `run PASS --inputs-of OTHER...` reads the inputs of other passes as PASS's when the manifest has none of its own (bdce); `dumps PASS DIR...` does the same for `llrm-c --dump DIR` step
files (an order branch's), `pair IN OUT --pass P` for one pair. `COUNTERPART` in the source is the table of llrm passes to opt pipelines.

Both sides are rewritten for LLVM the same way (`normalise`: llrm's calling-convention words, `declare internal`) and named alike
(`canonical`; also leaves out comments, function and parameter attributes, `align`, metadata attachments and phi order). A body that LLVM
cannot read after that is listed with its first error. The block order of the two answers is not normalised: a pass that moves blocks shows
as a large hunk with equal histograms.

`test_difftest.py` checks the output on a hand-made input where the answers agree and one where they differ.

`inlinediff.py BEFORE.ll AFTER.ll` (or `dumps DIR...`) compares the call sites llrm's inline stage inlined with those of `opt -passes=inline` at the same module, with LLVM's cost, threshold and reason (see its docstring); `test_inlinediff.py` has a module where a call over the threshold must show as llrm-only and one where both agree.

## A slot of LLVM's order

`llrm-pass [-On] --order PASS@SLOT FILE` runs one slot of the order alone (stderr names what stands in it); `difftest.py inputs PASS@SLOT DIR...`
compares it with `opt` on the inputs of `LLRM_DUMP_INPUTS=1 llrm-c --dump DIR`: every run, changed or not. `dumps PASS@SLOT` takes only the
steps that changed a body. A report over no function reads PREMISE FAILED.

## Joining a slot to parity

A translation joins the parity pipeline when this oracle reads clean on its pass. Run it for your pass, with the order branch's dumps (`llrm-c --dump DIR`) or the replay inputs, and the `llrm-pass` of your tree:

    python3 tools/difftest/difftest.py dumps PASS DIR/* [--llrm-pass target/release/llrm-pass]      # PASS: a key of COUNTERPART, e.g. earlycse, gvn, sroa

The first lines name the `opt` and its version (the pinned 24.0.0git d1106deb71cc, `tools/llvmpin.py`), then: `inputs N; failed (LLVM or llrm) 0; functions compared N; where either side changed the function: M, identical M; LLVM changed K of N; premise ok`. Clean is `identical` equal to the changed count, `failed 0`, `premise ok`, exit 0; the table under it then has no opcode rows. A premise failure (exit 3) means LLVM changed none or all of the functions and agreed everywhere: the run proves nothing. Quote the line and the opcode table in the PR; a remaining difference is listed with the first hunk of each function.

## The executed differential (`exec.py`)

Programs from `gen.py` (seeded; shapes ladder, sibling, loop_exit, widths) built by `llrm-cc -O2` and by the pin's `clang -O0`, run over 11 or 55 argument values; callees print their names, so a wrong or merged callee shows as a different trace.

    python3 tools/difftest/exec.py run --count 25 --targets x86_64-linux,x86_32-linux --bin target/release     # the batch step: 200 programs
    python3 tools/difftest/exec.py replay SEED SHAPE --bin target/release                                      # one program, both traces

A difference prints its replay command and the program reduced to the lines that still differ (a cut counts only if clang -O0 and -O2 agree on it). Exit 1 on any difference or broken step. Cost: 200 programs in about 1 s on 32 cores, no DOSBox.

Positive control (`fixtures/exec/`): `prefix_differs.txt` lists the 12 programs (seeds 0-9) that release/next without #6146 gets wrong at x86_64-linux; `LLRM_DIFFTEST_BIN=<release dir> LLRM_DIFFTEST_EXPECT=prefix|fixed pytest tools/difftest/test_exec.py` must find exactly those, or none on the fixed tree. `wrong_callee_cc.py` is a hermetic stand-in that calls c0 for c1: the harness must report differences against it.

Not caught: inputs outside the driven set, floating point, anything needing undefined behaviour, bugs the reference shares, m16 (a DOSBox sample is a separate step).
