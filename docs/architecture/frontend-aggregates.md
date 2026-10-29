---
architecture-index: front-end aggregates
architecture-order: 230
---
# Front-end aggregates: the plan

This page rests on an instruction count of the first dump of `d_faces`; no rows were run.

rows total 0 / with counters 0 / failed 0 / missing 0: no output row has been measured for this page.

TL;DR: the entry gap of 596 code instructions is mostly two front-end shapes, not aggregates: 403 are branch conditions and scaled indices (section 2), and about 46 are the aggregate expansion. llrm-cc copies and zero-fills an aggregate as a run of word loads and stores (`translate.rs` `copy`, any size) where clang emits one `llvm.memcpy` or `llvm.memset` (`EmitAggregateCopy`, `emitStoresForConstant`). The plan makes llrm-cc emit the intrinsics, checks that isel's small-size lowering gives what the expansion gave, and reviews `memcpyopt`'s re-merge of the expansion. But the aggregate expansion is only about 46 of the first dump's 341-instruction gap to clang (1.2% of the dump): most of the gap is two other front-end shapes, named below with their owners, and they are not aggregates.

Rule for the code: each Rust function is a translation of the C++ function named in its first comment (permalink at `d1106deb71cc`); anything not translated is a numbered DEVIATE with its fact, taken only with oversight's approval.

## 1. The entry gap of `d_faces`, by opcode

Instrument: `llrm-cc -m32 -O1 --dump` of `render/d_faces.c` (corpus `dfaces`, hash `53880fe70961`, from `tools/corpora.py`), its first dump `01-globalopt.ll`, against the pinned clang 24.0.0git (`d1106deb71cc`, `tools/llvmpin.py`) `-O1 -Xclang -disable-llvm-passes -emit-llvm` of the same file with the foreign flags of `compile_time.sh`. Instructions are the lines of a function body that are not labels; opcodes are read from the text; calls are split by callee. llrm at `b2fd0b0eb`.

llrm 3,636, clang 3,295: a gap of 341. Of clang's, 255 are no code: 228 `llvm.lifetime` markers and 27 other `llvm.*` calls (`fmuladd` and kin). Counting only code, the gap is 596.

| opcode | llrm | clang | diff | what it is | owner |
|---|---|---|---|---|---|
| `zext` + `icmp ne` after a comparison | 136 / 222 | 11 / 104 | +125 / +118 | 118 chains of `icmp; zext i1 to i32; icmp ne 0; br` where clang branches on the `icmp` | `llrm-hir` `mir.rs` `terminator` (`Branch` tests `value != 0`; `truths`, the comparison's `i1`, is used for `assume` only) |
| `mul` | 199 | 8 | +191 | 167 of them `mul idx, size` feeding a `getelementptr i8`; clang's GEP is typed and takes the index | `llrm-cc` `translate.rs` `scaled` |
| `store` / `load` | 427 / 1,057 | 385 / 1,026 | +42 / +31 | the word expansion of aggregate copies and zero-fills, and spill-like round trips of locals | `translate.rs` `copy`; the rest unclassified |
| `fmul` / `fadd` | 59 / 36 | 32 / 13 | +27 / +23 | clang contracts `a*b+c` into `llvm.fmuladd` (28 calls); the target has no fused op, so it is expanded again | not a gap: no fused op on x87 |
| `trunc` / `sext` / `alloca` | 68 / 352 / 175 | 41 / 329 / 153 | +27 / +23 / +22 | unclassified | open |
| `call` lifetime, `llvm.*` | 0 | 228, 27 | -255 | markers | no code |
| `phi` / `select` / `br` | 0 / 0 / 277 | 11 / 3 / 300 | -11 / -3 / -23 | clang's `&&`/`||`/`?:` as `phi`/`select` | the HIR emits blocks and slots |
| `call` memcpy | 0 | 2 | -2 | an aggregate copy clang does not expand | `translate.rs` `copy` |
| the rest (`add`, `sub`, `fcmp`, `ret`, user calls, ...) | | | within 4 | | |

So the two shapes that are not aggregates are 236 (the chains, 6.9% of the dump) and 167 (the scaled indices, 4.9%): 403 of the 596 code instructions. By the end of the pipeline (`04-pipeline.ll`) `zext` is 9 and `mul` 49 (clang's first dump: 11 and 8): the optimizer removes the chains and most scalings, so they cost compile time at the entry; the aggregates' 46 reach the output unless `memcpyopt` re-merges them.

## 2. The two front-end shapes, and what clang emits

The order of work is B1, B2, then the aggregates (A1 and after): they are the larger part of the gap, they sit in the same two front-end files, and the aggregates' 46 instructions are the smallest.

**B1. Branch on the comparison's `i1`.** Clang tests a condition with [`EmitBranchOnBoolExpr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/clang/lib/CodeGen/CodeGenFunction.cpp#L1901), which for a comparison branches on the `icmp` itself ([`EvaluateExprAsBool`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/clang/lib/CodeGen/CGExpr.cpp#L242)). llrm: `translate.rs` makes a compare a `Bool`-shaped HIR value; `mir.rs` emits it as `icmp` then `zext`/`sext` to the C type and records the `i1` in `truths` (L2819); the `Branch` terminator (L3417) then tests the widened value `!= 0` and does not look in `truths`, which only `assume` reads (L2849). Rule: a branch on a value that is a comparison's widened result branches on the recorded `i1`. It covers every condition that is a comparison (`if`, loops, `&&`, `||`, `?:`) and the same chain the optimizer would take two rounds to fold; it changes no value that is used as an integer.

**B2. The scaled index.** Clang's subscript is [`emitArraySubscriptGEP`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/clang/lib/CodeGen/CGExpr.cpp#L4920) over the element type: `getelementptr T, ptr, idx`. llrm: Open Watcom's tree has `ptr + idx * size`; `scaled` (`translate.rs`) keeps the product as an integer `mul` (with `nuw` for an unsigned index, stated there because only the front end knows the C type) and `mir.rs` `PtrOffset` (L3064) emits `getelementptr i8, ptr, product`. Rule: a displacement that is `mul idx, K` with `K` a constant, and used only as the displacement, is emitted as a `getelementptr` of a K-byte element with the index, carrying the `nuw`/`nsw` the front end stated. It covers every array subscript and pointer step of a constant-size element.

Both leave the optimizer's input as clang's, and the first-dump numbers are the predictions: -236 for B1, -167 for B2 (and the extra `getelementptr` forms disappear with them).

DEVIATE D4 (pending): MIR pointers are opaque and Open Watcom's tree does not carry the C element type to the GEP, so B2's element type is a K-byte integer or array, not the C type; GEPs of the same stride are alike, so nothing that compares GEPs by source type is affected (`memory.rs` decomposes both).

## 3. What clang does

| clang | does |
|---|---|
| [`CodeGenFunction::EmitAggregateCopy`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/clang/lib/CodeGen/CGExprAgg.cpp#L2249) | an aggregate assignment or argument copy is one `llvm.memcpy` of the type's size, alignment from the lvalues, volatile as the access (never a `memmove`) |
| [`emitStoresForConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/clang/lib/CodeGen/CGDecl.cpp#L1197) | a local's constant initializer: a scalar is one store; all-zero, or more than 32 bytes with at most 6 stores after a zeroing, is a `memset` of zero plus the stores ([`shouldUseBZeroPlusStoresToInitialize`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/clang/lib/CodeGen/CGDecl.cpp#L1003), [`emitStoresForInitAfterBZero`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/clang/lib/CodeGen/CGDecl.cpp#L956)); a repeated byte over 32 bytes is a `memset`; a small initializer is a store per element; the rest is a `memcpy` from an unnamed constant global |
| [`EmitAutoVarInit`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/clang/lib/CodeGen/CGDecl.cpp#L1951) | the caller of the above |

## 4. What llrm does

`translate.rs` `copy` lays `size` bytes down as loads and stores, widest first (4, 2, 1), at any size. A zero-fill or constant fill is stores. The MIR then has `size / 4` pairs per copy, which `memcpyopt` merges back (`MAX_PAIRS` 64 pairs, kept where they cost more than `loopidioms::copy_price`) and `Merge` turns runs of stores into a `memset` (MemCpyOpt's `tryMergingIntoMemset`; `lru` is a row of it, see [loopidiom-port](loopidiom-port.md)). The back end already lowers `llvm.memcpy` and `llvm.memset` of a constant length: `isel.rs` `memcpy` (`MEMCPY_MOVES` = 8 pairs, widest first, else `rep movsd` with `movsw`/`movsb` for the tail) and `memset`.

## 5. Stages and gates

| stage | content | gate |
|---|---|---|
| A0 | this page; the classification above; the rest of the gap (trunc, sext, alloca, the extra loads) named by owner | every opcode of the gap has an owner |
| B1 | the `Branch` terminator branches on the recorded `i1` | fail-first unit test on the emitted MIR of `if (a < b)`: no `zext`, no `icmp ne`; first dump of `d_faces` -236; rows rule, per-row worse counts first, since a different canonical form at the entry moves every later pass |
| B2 | `PtrOffset` of `mul idx, K` is a typed-stride `getelementptr` | the same for `a[i]`; first dump -167; identical or better output on the corpus |
| A1 | `llrm-cc` emits `llvm.memcpy` for an aggregate copy and `llvm.memset` / stores for a constant fill, by clang's rules above (`EmitAggregateCopy`, `emitStoresForConstant`); the HIR gets the operations if it has none | unit tests: the emitted MIR of a struct copy, a zero-init, a small and a large initializer; the first dump of `d_faces` loses the expansion |
| A2 | isel: a small `memcpy`/`memset` lowers to the per-word expansion it replaces (same pairs, same order, same width), larger to `rep` | emitted code of the corpus identical where the expansion was inlined; per-row worse counts first |
| A3 | `memcpyopt`'s re-merge reviewed: what is left of it once the front end emits the intrinsic, and `tryMergingIntoMemset` (the `lru` finding) | the pass keeps only what other passes produce |
| A4 | corpus: the 7 programs of `tests/run` with struct copies, QCport, and the vsgcc programs with struct copies (the 71 vsgcc rows alone would not move) | rows |

Rows: for every stage that changes output, the number of rows worse than base for each of clocks, executed instructions, memory operands and code bytes at each level (-O1, -O2, -O3, -Os) on m16 and m32, per-row worse counts first, then the geomean, then the worst row. The reference is the minimum of gcc 13.4 and the pinned clang; no waived rows.

## 6. DEVIATEs

All pending oversight; none is built before it is approved.

- D1. A local's constant initializer too large for stores is copied from an unnamed constant global in clang. On a flat unit llrm already lays the initializer under `_TEXT`; a 16-bit unit has no read-only data segment of its own here (fact: the front end's segment layout). The `memcpy` source is that object.
- D2. clang's `!tbaa.struct` on the copy is not carried: MIR has `!tbaa` on accesses only. Copies of aggregates with mixed field types lose the field types; alias analysis treats the copy as a plain `memcpy`.
- D3. `loopidioms`'s memmove direction tags (`llrm.forward`/`llrm.backward`) are not used by an aggregate copy: copies from the front end are `memcpy`.

## 7. Not in this plan

- #1053 (load elimination across loop trips): code exists on a local branch (`looploads`, with a linear-index change in `memory.rs`), tests pass, no corpus measurement was made; unmeasured, dropped here.
- #1871 (loop idioms): measured in [loopidiom-port](loopidiom-port.md): no row recovered beyond MemCpyOpt's merge.
- #1878 (`sroa` and `ssa flow` rows of the fjoins axis): unmeasured, dropped here.
