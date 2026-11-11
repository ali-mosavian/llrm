# wasm32 cause cards WSZ-6..31 (llrm-wasm-b, 2026-11-11)

TL;DR: re-measured on head 0bc8a1390b (#8983) at all four levels: WSZ-31 x87rotate and pattern_fill O3 are no longer above the pin; WSZ-33 is fixed in #8983. Of the rest, 7 rows have a named cause and 15 are open with the layer and the first diff known.

Instrument: `tools/wasm_corpus.sh` (bytes vs the pin), then per program the function and opcode diff (`wasm-objdump -d`), then the same-IR check: the pin's `-emit-llvm` IR through `llrm-wasm-llc -filetype=obj`. Same-IR bytes equal to ours = backend; equal to the pin = mid-end. Traps: our last dump is after LSR/CGP and the pin's `-emit-llvm` is before, so `getelementptr`/`phi`/`add` deltas are not evidence; `llrm-wasm-llc` reruns loop passes, so the check is invalid for loop-heavy programs (idioms, memfuncs_runtime, loop_interchange, pattern_fill).

## Named causes

| row | cell (B vs pin) | layer | cause | pin source | next |
|---|---|---|---|---|---|
| WSZ-10 | fcmp_all +5, all levels | backend | the program stores **volatile** `float`/`double` constants; the pin's `replaceStoreOfFPConstant` takes a volatile store (`isNormalStore`) and makes `i32.const`+`i32.store` / `i64.const`+`i64.store`; ours leaves `f32.const`+`f32.store` because the wasm combiner has `store_fp_constant_as_i64` off (the hook gates volatile and `f64`; default false, on for x86 and arm64). Non-volatile adjacent FP constants already merge to `i64.store` on both sides | `replaceStoreOfFPConstant` [L24395](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L24395); ours `dag_combiner.rs` `volatile_blocks` | turn the hook on for wasm; fail-first test `fp_const_store.rs` |
| WSZ-12 | overflow_builtins +4, all levels | backend | `smul.with.overflow.i32` widens at legalization to `sext(load)`; the pin folds it to `i64.load32_s` after legalization, ours leaves `i32.load`+`i64.extend_i32_s` (x2, and `_u` x2). Min repro: one `smul.with.overflow.i32` of two loads | DEVIATE SD205 (ext of load only before operations are legal), `dag_combiner.rs` L28; owner sdag-b, review 2026-11-27 | pull SD205 forward or fold it for this site class; test = min repro |
| WSZ-15 | data_int64 +2, O1..Os | backend | `trunc (srl (load i64), 60)`: the pin narrows to an `i32.load` at +4 and a shift; ours keeps `i64.load`+`i32.wrap_i64` | `reduceLoadWidth`; ours DEVIATE SD230 (no `reduceLoadWidth`) | translate `reduceLoadWidth` for truncate |
| WSZ-17 | high_byte +1, all levels | backend | `signed_high`: `(sext_inreg (srl X,8), i8)` stays `shr_u`+`extend8_s`; pin makes `shr_s` (X is `AssertSext i16`). IR is identical on both sides | `visitSIGN_EXTEND_INREG` [L17478](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L17478); ours `visit_sext_in_reg` has no srl case | add the fold; test on `signed_high` |
| WSZ-18 | bitfield_long_long +12, O2/O3/Os | mid-end | 5-byte packed struct: ours keeps `i32.store8 12` then reads `i64.load`; the pin folds the 12 into one constant and reads bytes. Same-IR 150 vs pin 149 | SROA odd-size DEVIATE | SROA split of the 5-byte alloca |
| WSZ-6 | mulconst_i64 +22, O1..Os | mid-end | `m1` and `m47` stay as 3 B functions with 2 calls in `main`; the pin inlines both and drops them. Same-IR = pin | inliner: SZ1/SZ2 | join the SZ2 trial (`m1`, `m47` costs) |
| WSZ-28 | phi_slot_unknown_arm +1, O2..Os | backend | ours `load16_u`+`extend16_s`, pin `load16_s`. Same-IR = ours | `(sext_inreg (extload))` after legalization; ours `sext_in_reg_of_load` | confirm with the WSZ-12 fix |

## Open (layer known, cause not read)

| row | cell | layer | first diff |
|---|---|---|---|
| WSZ-20 | function_table +4 Os | mid-end (same-IR = pin) | ours keeps a loop with `load`+`call_indirect`; the pin has none |
| WSZ-21 | huffman_decode +64 O3 / +8 O2 | backend (O3 IR identical; same-IR = ours) | one more local, `local.get`/`local.set` +16 each, wider local indices |
| WSZ-8 | shift_int32_variable +16 O1, +12 Os | mid-end +12, backend +4 | `icmp` +2 in `main`; `local.get/set` +4 |
| WSZ-9 | shift_int64_variable +12 O1/Os | mid-end | same-IR = pin; `i32.const`+2, `local.set`+2 |
| WSZ-11 | byte_multiply +4 O1 | backend | `i32.lt_u` -2, `i32.ne` +2, `local.tee` +2 |
| WSZ-13 | tail_calls_edges +3 | backend | `vfwd`: `i32.const`+`i32.add` +1 (before #8749) |
| WSZ-14 | variadic +3 O1 | backend | `i32.add`+`i32.const` in `main` |
| WSZ-16 | compare_to_int64 +1 O1/Os | backend | `i32.ne`/`i32.and` vs pin `select` |
| WSZ-19 | div_constants_short +1 O3 | backend | `i32.and`+`i32.const` -1 |
| WSZ-22 | idioms +43 O2/O3 | mid-end (loop shape) | first differing decision: the reverse-copy loops over a 16-bit count. Ours' runtime-unroll prologue counter is `i16` (`%prol.iter`, `icmp eq i16 .., %xtraiter`), which costs `i32.const 65535; i32.and` + a second counter (hunks at wasm insn 903-976); the pin's final IR has no `i16` compare or phi. Whether the pin widens the IV before unroll (IndVarSimplify) is not confirmed |
| WSZ-23 | loop_interchange +3 | loop-heavy | `shr_u`/`shl` for `and` |
| WSZ-24 | memcpy_libcall +8 O2/O3 | mid-end +2, backend +6 | `block`/`end` +2, `i32.xor` +2 |
| WSZ-25 | memfuncs_runtime +7 O2 / +35 O3 | after mid-end | mid-end checked: at the end of the mid-end both IRs hold the compare `icmp ult (n-1), 3` once at the top of the outer loop and once more later (ours stage 21: lines 22, 188; pin: 26, 193), so it is not shared by a mid-end pass. The pin's asm keeps the compare result in a local set at the top; ours keeps `n-1` and recomputes `lt_u` at each use. CGP sinks the compare on both sides (ours `39-cgpaddr`, pin `-print-after=codegenprepare`). The machine pass that differs is not found; our machine_licm.rs hoists constants only (DEVIATE 1) |
| WSZ-26 | object_extent_loop +4 | mid-end (ours-llc-on-pin-IR 1469 < pin 1489) | locals +1, `i32.shl` +1 |
| WSZ-29 | stack_arrays_copy +16 O2 | mid-end | two `i8` copies where the pin copies one `i16` |

## Closed or moved
- WSZ-31 x87rotate: `rot` is 14 B below the pin at O2/O3; not above.
- WSZ-27 pattern_fill: O2 +1, O3 not above (WSZ-33 fix in #8983).
- WSZ-33: fixed in #8983 (shared SE in runtime unroll; no revisit of new loops). x86_64 O2 `known_loads_walks` +32 B on that PR is a faithful fold; the source is the missing vectorizer (the pin tags scalar remainder loops `llvm.loop.unroll.runtime.disable`).
- WSZ-32 alloca_frame: DEVIATE W11 (`frame.rs`), owner llrm-w9-tests pending confirmation, exit 2026-11-30.

Not a cause (checked): `xor (fcmp oeq), true` stays in our IR where the pin has `fcmp une` (`combine_not.rs` folds only `ICmp`, pin `foldNot` any cmp); both give the same bytes in `fcmp_all`.
