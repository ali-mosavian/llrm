# InstCombine like-for-like against the pin

d_faces -O2, parity 121f6f17e. Same IR: the 18 inputs of the four combine slots (@677, @768, @801, @837) that the pipeline hands each function, ours via `llrm-pass --order combine@N`, the pin via `opt -passes=instcombine` on the same file, `InstCombinerImpl::run()` as the pin's total (its fixed 2.5 M per run is left out). Raw per-input Ir (ours, pin) and the totals: [instcombine-like-for-like.data.txt](instcombine-like-for-like.data.txt).

## Per run on equal input

| function (inclusive Ir) | ours | pin |
|---|---|---|
| whole pass | 46.2 M | 58.6 M |
| icmp (`simplify_icmp` / `simplifyICmpInst`) | 7.9 M | 16.0 M |
| GEP visit | 4.2 M | 6.9 M |
| select visit | 2.3 M | 6.1 M |
| known bits | 2.0 M | 7.5 M |
| non-zero | 0.8 M | 3.4 M |
| `simplify_instruction_with` | 11.3 M | 0.5 M (inlined in the pin) |

On equal input ours costs 0.79x of the pin. The largest slot input (walk/11, 1250 instructions) is 8.7 to 10.1 M against 11.5 to 12.9 M. No visit function is dearer. There is no per-visit structural miss to hunt.

## Whole pipeline

Ours: `combine` 69.1 M, of it 54.6 M in the visit loop (11,159 visits, 50 runs). The pin: 49.2 M, 43.0 M in the loop (12,025 visits, 76 runs). The excess is not the visit work on equal IR (above), so it is the IR the passes receive and what runs around the loop: 14.5 M outside the loop against the pin's 6.2 M.

Outside the loop, Ir (calls): self 2.5; `sink_block` 2.25 (11,159); `reverse_postorder` 1.31 (50); worklist push 1.16 (10,877); `with_dom_conditions_of` 1.06 (50; #4646 makes it lazy); `simplify_instruction_in` 0.97 (957); `FunctionAnalyses::of` 0.96 (50). The DomTree and AssumptionCache builds are down to this from 2.5 M (#4629, #4639).

## Cuts

The profile of a slot is flat: on the walk/11 @801 input the largest exclusive item is `combine::visit` at 2.1%, then `sink_block` 1.3%. No single cut of 0.5 M or more is visible inside the loop. Outside it, `sink_block` (201 Ir per visit, the same checks as the pin's lambda) and `reverse_postorder` are the two items above 1 M.
