# F2: `hir::mir::emit_module`, the MIR builder

Owner: w-8 (2026-10-24). Pin d1106deb71cc. Row F2 of [frontend-bucket-plan](frontend-bucket-plan.md); F1 and `hir::unit` are w-5's, F3 (`translate::program`, [f3-plan](f3-plan.md)) w-1's.

## Profile (d_faces qbq-llrm, ledger command, dist of cc4ed08729, callgrind `--toggle-collect=llrm_hir::mir::emit_module`)
`emit_module` 16,150,456 Ir inclusive (the same figure as the unfiltered profile's 1.75% of 922.6 M). Callees: `Body::emit_block` 7.42 M (288 blocks), `declare_outside` 2.87 M (11 functions), `facts::Index::of` 1.28, `Body::new` 1.13, `Body::allocate` 1.11, `result_destination` 0.35, the rest under 0.15 each. Self cost in the subtree:

| Function | M Ir | What it is |
|---|--:|---|
| `HashMap<i64, i64>::insert` | 1.62 | `declare_outside` builds `values` (every value of the function) before it scans anything; `Body::new` and `allocate` insert the rest |
| `Types::intern` | 0.92 | the MIR type of a value or operand re-interned on each use |
| `memcpy` | 0.91 | |
| `declare_outside` self | 0.89 | four scans of every instruction (`call_sites`, the lifetime ops, `CopyBytes`, places) |
| `Body::instruction` / `place` / `value` | 0.75 / 0.67 / 0.43 | the per-instruction kernel (4,727 instructions) |
| `reserve_rehash<(i64, i64) -> MetadataId>` | 0.65 | a map grown by doubling, never sized |
| `value_type` / `operand_type` | 0.47 / 0.18 | |

## Pin counterpart
`CodeGenFunction::GenerateCode` 11.73 M + `CodeGenModule::Release` 1.62 M inclusive on the same file ([f3-plan](f3-plan.md)), shared with F3. The pin declares a callee when the first call is emitted (`GetOrCreateLLVMFunction`), caches converted types (`CodeGenTypes::TypeCache`), and `CGBuilder` makes an instruction in place with no side table of the function's values.

## Rows (one PR each; ledger Ir before and after; objects identical)
| # | Change | Pin | Target M | Date |
|---|---|---|--:|---|
| F2-1 | `declare_outside`: one pass over the instructions, no `values`/`places` maps built per function | `GetOrCreateLLVMFunction` at the first call | -1.2 | 11-17 |
| F2-2 | the MIR type of a value is interned once (`value_type`, `operand_type`) | `CodeGenTypes::TypeCache` | -0.5 | 11-24 |
| F2-3 | the `(i64, i64) -> MetadataId` map sized from the function's blocks | `DenseMap` reserved by the caller | -0.4 | 11-24 |
| F2-4 | `Body::instruction`/`place`/`value`: self-cost by operation before any change; no claim until a defect is named | `CGBuilder::Insert` | -1.0? | 12-04 |

Calibration (w-1's F3 PRs): a byte-loop cut lands about 55% of its attribution and a memo cut about 30%, so the targets above are those fractions of the figures in the profile table, not the figures. A row that does not move its number is dropped.
