# llrm-only MIR passes bucket (d_faces -O2 m32, tip 3b5ac93b68)

Owner: w-6. Bucket 201.8 M of 1292.4 M Ir, pin 0 (call-site rule: the pass's `run` inclusive;
`ledger-3b5ac93b68-O2.txt`, command `-m32 -mabi=sysv -march=i486 -O2`).
Route: parity = `llvm_order` (default; slots in `parity::REGISTRY`), legacy = `LLRM_NO_LLVM_ORDER` rounds. No pass here is legacy-only.

| Pass | M Ir | Parity route | Pin counterpart |
|---|--:|---|---|
| fold | 177.2 | 5 slots: fold@677, @768, @801, @837 (Simplify), @1242 (Cleanup); Old, `instcombine-port` | instcombine (the same 5 positions; ours runs the translated `combine` there too, so two instcombines per slot) |
| algebraic | 22.8 | slot algebraic@696; Old, `instcombine-port` | instcombine (a third stand-in at that slot) |
| addresssink | 0.53 | tail of `applied`, no slot | CodeGenPrepare (sinkCmpExpression, address sinking; row CG DEVIATE C1) and GVNSink |
| ports | 0.55 | early list of `applied`, no slot | none (x86 in/out narrowing to the port's memory; llvm has the intrinsic attrs) |
| gepoffset | 0.30 | tail, no slot | none exact: LSR/CGP address-mode formation |
| algebraic::Differences | 0.22 | strength slot after `lsr` | LSR's SimplifyInstructions cleanup (instsimplify) |
| maskscale | n/a | strength slot after `differences` (real mode only) | none (DEVIATE LP-7: LSR has no masked scaled-counter formula) |
| inferspace | 0.09 | slot inferspace@787; Old | infer-address-spaces (target pass: none on x86) |
| fixednarrow | 0.05 | tail, no slot | none (SelectionDAG expands `smul.fix`) |
| switchlower | 0.03 | tail, no slot | SelectionDAGBuilder switch lowering (isel), not a mid-end pass |
| window | 0.0005 | tail, no slot | none (huge-pointer windows: no llvm counterpart) |

`fold` is 88% of the bucket. The ten sum to 201.75 M.
