# tools/llvm-spike

Scripts of the opt-as-mid-end spike (`LLRM_LLVM_MID=O2|Os`, `llrm-transforms/src/llvmmid.rs`). Paths are the spike's (`~/scratch/llvm-spike`).

    all32.sh TAG [ENV=VAL...]   m32: every vs-gcc kernel built with the LLVM mid end under ENV, run in the harness
    cmp.py llrmElf TAGElf [-v]  geomean and worst row of TAG against the baseline
    gate16.sh TAG [ENV=VAL...]  m16: the gate's 230 measurements (tools/bench/bench.py) under ENV
    cmp16.py TAG                the same per front end; rows that cannot be counted, by cause

ENV: `LLRM_LLVM_AFTER=1` runs opt after our mid end, `LLRM_LLVM_THEN=1` before it, `LLRM_LLVM_FLAGS`, `LLRM_LLVM_NATIVE=n32|n8:16`.
