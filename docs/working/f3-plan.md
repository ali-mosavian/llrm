# F3: the stage between the record stream and the MIR (`translate::program`)

Owner: w-1 (2026-10-24). Pin d1106deb71cc. Row F3 of [frontend-bucket-plan](frontend-bucket-plan.md); F1 and `hir::unit` are w-5's, F2 (`emit_module`) w-8's.

## Profile (d_faces qbq-llrm, ledger command, symbolized release build, callgrind)
`translate::program` 16.93 M Ir inclusive: `Body::function` 16.2 M, of which `Body::eval` 12.4 M and `Body::expression` 12.2 M (recursive: 363 evals; the nested instances count again). Self cost attributed to the subtree by call-edge weight (an approximation: callgrind keeps no call paths), 14.5 M of it:

| Bucket | M Ir | Functions |
|---|--:|---|
| a node field re-read from text | 1.48 | `i64::from_ascii_bytes_radix_impl` 0.83 (3,606 + 1,051 calls), `str::trim_matches` 0.65 |
| types | 1.93 | `Unit::canonical_type` 0.75, `Types::of` 0.65, `Types::c` 0.52 |
| maps and sets by node id | 2.6 | `HashMap<i64, ()>::insert` 0.52, `IndexMap<i64, Node>::get` 0.39, `IndexMap<String, i64>::get` 0.37, `HashMap<i64, Got>` 0.28 + rehash 0.18, `IndexMap<String, String>` 0.23, hashing 0.28 |
| allocation and copies | 2.1 | `__rust_alloc` 0.81, `memcpy` 0.62, `finish_grow` 0.26, `__rust_dealloc` 0.30, `String::clone` 0.20 |
| the walk itself | 1.9 | `expression` 0.49, `function` 0.35, `eval` 0.28, `instruction` 0.22, `converted` 0.19, `program` 0.21 |
| the rest | 4.5 | a tail under 0.18 M each |

## Pin counterpart
clang's `CodeGenFunction::GenerateCode` is 11.73 M inclusive on the same file and `CodeGenModule::Release` 1.62 M (ledger2 profile): about 13.3 M against our 16.9 M, so F3 alone is 3.6 M over, and the rest of the front end's gap is F1 and F2. The pin's AST holds typed fields (no text to parse), `CodeGenTypes` caches each converted type in `TypeCache`, `LocalDeclMap` is a `DenseMap`, and `CGBuilder` makes instructions in place.

## Rows (one PR each; ledger Ir before and after; objects identical)
| # | Change | Pin | Target M | Date |
|---|---|---|--:|---|
| 1 | the record stream's numbers are read by a byte loop, not `trim().parse()` (done, ledger -0.83 M; typing `hir::Node`'s fields stays an option) | AST fields are typed | -1.4 | done 2026-10-24 |
| 2 | `Unit::canonical_type` walks the alias chain without a copy (done, ledger -0.24 M); `Types::of`/`c` memoized per type id (open) | `CodeGenTypes::TypeCache` | -1.5 | 2026-12-01 |
| 3 | what a node evaluated to is a dense row and `drop_unread_scales` notes only scales (done, ledger -0.48 M; the `HashMap<i64, ()>` cost the profile gave F3 was mostly `LirBody::with_blocks`, not F3's) | `DenseMap`, `LocalDeclMap` | -2.0 | done 2026-10-24 |
| 4 | `String` clones and growth in `instruction`/`converted` | `CGBuilder` in place | -1.2 | 2026-12-08 |

The sum, -6 M, is more than the 3.6 M gap: the target is the pin's 13.3 M, and a row that does not move its number is dropped.

## Remaining gap (numbered row, 2026-10-24)
F3-R5: rows 1, 2a, 3 and 4 move `translate::program` by about 2 M Ir of 16.9 M (ledger, rows' PRs: -0.83, -0.24, -0.48, and row 4's), against the pin's 13.3 M: about 2 M of the 3.6 M gap stays. What is left is the long tail of the profile (4.5 M of functions under 0.18 M each), `Types::of`/`c` (1.17 M, the second half of row 2), and the `Vec<String>` the record stream builds (`hir::unit`, F1, not F3). Owner w-1, exit 2026-12-08: re-profile F3 at the tip and either close the gap with named rows or record why it stays.
