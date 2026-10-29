# LiveRegMatrix profile, d_faces m32 -O2

Parity `121f6f17e`, release `llrm-cc` with line tables, callgrind Ir, `-m32 -march=i486 -O2`, `qcstub/src/render/d_faces.c`. Total 746.9 M; the allocator (`greedy_rewritten`) 77.6 M.

| Function | Ir (inclusive) | Calls | Ir/call | Pin per call |
|---|---|---|---|---|
| `check_interference` | 7.45 M | 3424+ | 2.2 k | 0.82 k (`checkInterference`, 3.07 M / 3754) |
| of it `reg_mask_clobbered` | 3.50 M | 945 | 3.7 k | in the pin's line above |
| of it `query_check` | 3.26 M | 4584 | 0.7 k | in the pin's line above |
| `assign` | 2.49 M | 687 | 3.6 k | 2.8 k (2.02 M / 721) |
| `get_priority` | 0.53 M | 888 | 0.6 k | |
| `AllocationOrder::create` | 0.22 M | 944 | 0.2 k | pin 0.23 M |
| heap `pop` | 0.14 M | 946 | 0.15 k | |

The matrix side is about 10.8 M of 77.6 M. Against the pin it is ~4.5 M over (check_interference ~4 M, assign ~0.5 M), well under a tenth of the 61 M allocator gap. The gap is in `try_split` (42.9 M inclusive, 101 calls), which is SplitKit's.

The pin profile (`pin.cg`) is a different, smaller input: only the per-call ratios are comparable.

`reg_mask_clobbered` is already cached per vreg like the pin's `RegMaskVirtReg`; its cost is the per-segment merge with the call slots.

Tried and dropped: the pin's `checkRegMaskInterference` walk shape in `reg_mask_clobbered` (early return past `endIndex`, linear segment skip instead of a search per segment). Same object code, `reg_mask_clobbered` 3.50 M -> 4.27 M and the run 746.9 M -> 747.6 M: the linear skip costs more than the searches it replaces at these segment counts.
