# Front end bucket (qbq-llrm d_faces, -O2): plan (2026-10-23)

Owner w-5. Ledger: ours 61.7 M, pin 41.8 M (+19.9). Plan due 10-26.

## Attribution

`llrm-c -m32 -O2` on `qbq-llrm/src/render/d_faces.c`, callgrind with `--trace-children=yes` (the C parser is a child process, so a plain run misses it).

| stage | M Ir | what |
|---|--:|---|
| wccq (Open Watcom parse + sema, child) | 17.7 | of which printf of the record stream 6.3, `GetSymList` 1.4 |
| record decode in `compile::selected_checking` (self) | 10.8 | text stream parsed back |
| `hir::unit` | 4.8 | records to HIR types/symbols |
| `translate::program` | 15.7 | `Body::eval` 10.3, `branch` 1.7 (11 functions, 363 evals) |
| `hir::mir::emit_module` | 16.8 | `emit_block` 7.7 (288 blocks), `declare_outside` 3.2, `facts::Index` 1.3 |
| sum | 65.8 | ledger bucket 61.7 (difference: ledger's own slot boundaries) |

Pin, same TU and flags, `perf stat instructions:u`: `-fsyntax-only` 30.8 M (lex, parse, sema), IRgen adds 17.2 M (48.0 M with process start-up).

## Reading

Parse and sema are cheaper than the pin's (17.7 against 30.8). Everything after the parse is the excess: 48 M for what clang's IRgen does in 17 M, and 11 M of it is only a text round trip (printf in the child, decode in the parent). QCport is under the pin because its sources are smaller per function; the cost here scales with blocks and expressions, not with parse.

## Rows (one change each, numbered)

| row | change | expected | check |
|---|---|--:|---|
| F1a | cgshim formats its own lines, `stream::parse` borrows (landed #4187) | -7.5 | perf stat with wccq 1059.7 -> 1052.2 M; callgrind `--trace-children` 1079.8 -> 1071.7 M (-8.1) |
| F1b | `stream::parse` line profile (debuginfo build, d_faces -O2): no defect. Top lines, Ir: token scan `Tokens::next` 1.26 M (12,157 tokens), ASCII check `text.bytes().all` 0.61 M, lines `Vec` collect 1.2 M, `value_of` 0.33 M, record push 0.13 M; 4.5 M in stream.rs, ~10 M with callees and allocation. The earlier 6.0 M self came from an `#[inline(never)]` build that moved helpers into it. Remaining ceiling: lazy lines + a memchr token scan, about -2.5 M; a binary format is not justified (the text scan is 3 M of the 10). Owner w-5, not scheduled; taken only if the board needs the last 2.5 M | -2.5 | callgrind `--trace-children`, same TU |
| F2 | MIR builder: `Builder::emit` 830 Ir per instruction (`create_instruction` 540, `insert` 230) over 4,727 instructions of d_faces; `Body::instruction` 1,520 Ir per instruction in all. No single defect in the children (value 1.9 M, place 0.7, store 0.6, convert 0.4); the row is the builder's per-instruction kernel, owner w-5, after the Pointers route | -3 | callgrind inclusive |
| F3 | `Body::eval`: attribute self cost by expression kind before changing anything | -3 | callgrind self by function |
| F4 | `hir::unit` canonical-type interning (`canonical_type` 0.9 M self) | -1 | callgrind |

F1 first: it is the one cost with no counterpart in the pin. F2 to F4 are attribution rows until a self-cost profile names a defect; none is claimed before it does.

## Not covered

Other programs, m16, QCport (already under the pin); the pin's 41.8 M is the ledger's, not re-derived here.
