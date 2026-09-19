# QB source frontend

The executable progression is defined in [test-ladder.md](test-ladder.md).
Small BC-compared runtime probes are required before qrender or qb-quake module
substitution.
[footprint.md](footprint.md) records both BASIC-owned `BC_CODE` and complete
linked executable code (`BC_CODE + CODE`) for BC and the QB frontend after
each integration round; `.OBJ` file lengths are never used as the code-size
measurement.

Status: implementation in progress. The common HIR boundary and an in-tree,
dependency-free Rust source frontend now exist. All 17 qb-qrender BASIC
modules pass the VBDOS syntax frontend, and all 17 now complete HIR, semantic
MIR, physical MIR, LIR, allocation, and final inline-x87 staging. Full saved stage runs take
32-body `screen`, 26-body `model`, 15-body `r_bsp`, 7-body `mod_tex`, 6-body
`common`, and 5-body `snd` through allocated LIR and the QB-owned inline-x87
finalizer without changing MIR or the backend.

The implemented semantic slice emits integer/LONG/floating/array operations
and structured IF/FOR/DO/WHILE control flow directly as HIR. Procedure bodies,
scalar/UDT `BYREF`, packed UDT fields, dynamic-array descriptors, nested
fields, and whole far-pointer element/field addressing are implemented.
`REDIM` retains a logical descriptor identity, `ERASE` consumes either a local
descriptor place or incoming descriptor value, and numeric/string `SELECT
CASE` becomes ordinary HIR control flow.

Procedure declarations and definitions are reconciled during semantic
analysis. Each call site carries a stable callable-table ID after SUB/FUNCTION
kind, result use, arity, parameter mode, type, array/SEG status, alias, and
calling convention have been checked; later stages use the linkage name only
from that table.

QB intrinsic names are resolved during semantic analysis by a separate
declarative catalogue in
`frontends/qb/src/intrinsics.rs`. Each entry owns dialect availability, arity,
result class, effects, and an exact lowering variant (`Sin`, `Cos`, `Tan`,
`PointerOffset`, `PointerSegment`, `Floor`, and so on). Semantic analysis
consults that one catalogue for both numeric and string expressions, then
runs the variant's typed lowering algorithm. It never distinguishes two
intrinsics by comparing their source spellings again. This keeps recognition
and semantic identity out of name-by-name dispatch without pretending that
conversions, string descriptors, and inline transcendental expansion are
passive table data.

Measured string boundaries include fixed-string adaptation (`B$LDFS`),
assignment, comparison, concatenation, trimming, `MID$` expression/assignment,
`CHR$`, `LEFT$`, both `STRING$` forms, `STR$`, `VAL`, and `ENVIRON$`. Runtime
calls remain visible and effectful; numeric computation before and after them
remains typed and optimizable. Dynamic string-array arithmetic keeps a whole
16:16 pointer until the near string-arena descriptor offset is required.

Structured file I/O includes `FREEFILE`, `OPEN` in
INPUT/OUTPUT/APPEND/BINARY modes, `EOF`, counted `CLOSE`, `LINE INPUT`, `SEEK`,
and positioned/unpositioned record `GET`/`PUT`, disk `INPUT`, formatted
`PRINT`, and `DIR$`. The measured `OPEN` mode words
are 1, 2, 8, and 20h. `GET3`/`PUT3`, `GET4`/`PUT4`, and `SSEK` keep far record
pointers, LONG positions, and exact Pascal cleanup in the frontend ABI layer.

`DEF SEG` stores directly to the runtime-owned external `b$seg` cell and
`PEEK` loads that same cell in every procedure/module. This matches VBDOS
`R_BSP.OBJ` and `D_SURF.OBJ` while improving on BC's opaque `B$DSEG` call by
making the memory dependency visible. `ON ERROR` is resolved to function-side
HIR metadata rather than a false normal control-flow edge; the object adapter
materializes its measured `push cs`, relocated handler offset, and `B$OEGA`
registration after handler layout.

Fresh object emission now covers a BASIC-shaped source module: physical ABI
materialization, production MIR optimization, allocation, `backend.masm.Module`,
and the shared fresh OMF writer are connected. Defined functions are exported
as far symbols without BASIC type suffixes, and `retf n` uses the verified
Pascal parameter byte count. The frontend-owned envelope supplies the measured
48-byte `MODULE_CODE` header, `BC_SA` registration, BASIC segment classes,
descriptor segments, `B$CEND` module exit, and `B$OEGA` error registration.
All 17 qb-qrender BASIC modules now emit fresh objects. Together with the
unchanged C/assembly objects and a compatible measured uGL archive they pass
LINK and produce `QRENDER.EXE`. Real runs exposed and fixed DGROUP overflow,
an incorrect module `U_FLAG`, static-string near/far layout, and missing
`COMMAND$` resolution. The managed-local frame now matches the measured
runtime header and temporary-STRING allocation contract: a reduced
`B$DDIM`/`COMMAND$` executable prints and exits normally. Subsequent
fresh-`SYS` rounds fixed near STRING-array descriptor access, Pascal SUB call
order, positive parameter displacement preservation, bare zero-argument
FUNCTION resolution, the public `DX:AX` LONG-result boundary, effectful
zero-argument `TIMER`, and the hidden destination-pointer ABI for user
`FUNCTION ... AS SINGLE/DOUBLE` results. The earlier `ents.bin missing` result
was a broken integration instrument: both builds used a stale uGL archive
without `ZIP_CHECK`, and their response files omitted `QR_PROF.OBJ`. With the
ZIP-capable archive and complete object list, the all-BC control renders five
frames. Replacing only `SYS.OBJ` with the fresh object also renders and exits;
its structural counters match the control (`153` polygons, `365` triangles,
leaf `115`, and identical clipping counts). The timed camera state and BMP
hash differ because the two runs sampled different elapsed intervals, so this
is an executable mixed-build milestone, not a claim of deterministic image
identity or that qb-quake runs correctly.

A later isolated `SCREEN.OBJ` substitution initially returned immediately
after the `ugl` memory mark. Its module `$STATIC` numeric-array descriptor was
initialized in `BC_DATA`; inspection of the linked bytes and live DOS memory
showed that BASIC startup had cleared it. Static descriptors now occupy a
separate read-only `BC_CN` object with a far relocation to their `BC_DATA`
elements, matching BC. The fresh-SCREEN mixed build now reaches `font`, loads
the map, renders five frames, and writes `BENCH.TXT` with 153 polygons, 365
triangles, and camera leaf 115. Whole-program fresh substitution remains a
later gate.

The next all-fresh failure was two parts of one allocation-mode omission.
The lexer discarded `'$DYNAMIC`, so bounded `T_MIP_INF(1)` became a static
array and `B$RDIM` rejected it. Once its descriptor became runtime-owned, the
frontend still treated the allocation as DGROUP data. Direct comparison with
VBDOS `MOD_TEX.OBJ` showed the owned-array spelling: selector at descriptor
`+2`, adjusted offset at `+0Ah`. The QB semantic layer now constructs a huge
pointer from those fields. Listing comparison then found fresh `R_BSP` reading
numeric array formals from descriptor `+0`, while VBDOS uses selector `+2` and
adjusted offset `+0Ah`.

The all-fresh QGL poly-draw image now loads `dm3ish.bsp` and reaches its visible
`FIRE TO START` frame. After the descriptor correction, its first walk still
did not terminate: `WHILE NOT (nodenr AND &H8000)` materialized `NOT &H8000`
as the true value `&H7FFF`. VBDOS `/O` materializes the same integer result but
reverses a control expression's successors when `NOT` occurs anywhere in it.
HIR now records that successor order explicitly; no MIR or backend change was
needed.
[memory-model.md](memory-model.md) records the raw comparison.
`tools/qbstages.py` writes input, HIR, semantic MIR, optimized MIR,
physical MIR, LIR, every machine pass, and inline-x87 output adjacently under
`build/qbstages/*-round` so every transition can be inspected and diffed. MIR
uses stable three-address assignment notation (`c <- a op b`). LIR and every
later stage use Intel operand order and MASM spelling; pre-allocation operands
remain explicit virtual registers (`add v3, v2`), while allocated stages name
the physical registers. The final inline-x87 stage prints intrinsic bytes as
in-place MASM `db` directives rather than displaying them as calls.

The literal-data profile is now independently executable under all three
runtimes: QB 4.5 and PDS 7.1 receive their near `BC_CN` descriptor/payload,
while VBDOS retains its `BC_CN` bridge to private `FSL_CONST`. Microsoft and
fresh `PRINT "A"` probes all print the same result.

## Scope

The target is an Intel 386 or newer CPU running 16-bit real-mode DOS. This is
not a flat 32-bit target, and it is not an 8086/286 code-generation project.
The frontend links against the selected QB, PDS, or VBDOS runtime rather than
reimplementing that runtime.

FreeBASIC is not an implementation or compatibility authority for this
frontend. Its checkout supplies only useful `.bas` test ideas. Every adapted
case is made legal for the selected Microsoft dialect and its semantics, ABI,
runtime behavior, and expected result are re-established with QB, PDS, or
VBDOS artifacts before the case becomes a regression.

The parser aims to accept the selected dialect faithfully, including the
additions made by QB, PDS, and VBDOS. Native semantic expansion is deliberately
narrower. Only these families are lowered as inline computation:

- integer and `LONG` arithmetic and comparisons;
- floating-point arithmetic already represented by current MIR;
- the current set of recognized numeric math operations; and
- numeric array bounds, addressing, loads, and stores.

Other supported statements and functions are runtime calls or ordinary
control flow. They do not justify new MIR instructions. Syntax may be accepted
before its lowering is implemented; in that case compilation reports a
specific unsupported construct after semantic analysis.

## Pipeline

```text
source bytes
  -> dialect lexer
  -> parser and private syntax nodes
  -> declarations and name resolution
  -> type, storage, and control-flow resolution
  -> common HIR
  -> QB ABI/HIR-to-MIR adapter
  -> existing MIR optimizer and backend
  -> OMF linked with the selected BASIC runtime
```

The frontend owns everything above HIR that varies by language. The adapter
owns the QB-family ABI and runtime choices. MIR and everything below it remain
unchanged.

Fresh object emission reuses `qbopt.backend.masm.Module` and
`qbopt.backend.omfwrite`, the single fresh-OMF path already used by the WCC
frontend. The QB adapter supplies native far procedure entry/exit, callee
cleanup, argument order, ordinary data, runtime imports, the BASIC module
envelope, and its startup/descriptor fixups. Runtime-owned local cleanup and
the complete link plan remain frontend adapter work. These are ABI and object
layout facts, not MIR operations.

Dynamic-string array elements illustrate the real-mode split deliberately:
array arithmetic retains a whole 16:16 pointer, while QB's string arena is
near. The frontend projects the element descriptor's 16-bit offset before
`B$SASS`, `B$FLEN`, or `B$SCMP`, matching VBDOS listings; it does not ask MIR
or the backend to learn a BASIC descriptor special case.

The first part of that adapter is implemented in `qbopt/frontend/qb/abi.py`.
Calls retain typed, source-order operands through semantic MIR and its
optimizers. The adapter materializes the already-existing `ARG` operations in
the HIR side table's order, removes arguments from the physical `CALL`, and
attaches conservative runtime/user-procedure contracts. An unaudited BASIC
runtime cleanup remains a hard error rather than a guessed stack adjustment.

Three selections are kept independent:

- **dialect** controls accepted source and source semantics;
- **runtime family** controls descriptors, calls, startup, and library names;
- **target policy** is fixed to 386+ real mode and native x87 for inline
  floating operations.

Array order is a separate compiler option. The default is the QB-family
column-major order (first subscript varies fastest); `--array-order row-major`
models BC's `/R` switch used by qb-qrender. It is not inferred from either the
VBDOS dialect or VBDOS runtime.

Defaults may pair common combinations, but the implementation must not infer
one of these from another in scattered code.

## In-tree layout and process boundary

The current ownership layout is:

```text
frontends/qb/                 Rust source frontend executable
  grammar/                    recovered base grammar plus typed action schemas
  src/generated_parser/       generated tables, VBDOS-superset lexer, AST builder
  src/dialect_extensions.rs   declarative additions after QBasic 1.1
  src/syntax.rs               private syntax nodes
  src/semantic.rs             types, storage, CFG, and HIR JSON construction

qbopt/hir/                    language-neutral Python HIR decoder/verifier
qbopt/frontend/qb/            QB ABI, link plan, and HIR-to-MIR adapter
frontends/qb/tests/           focused parser/semantic regressions
tests/test_hir.py             common-boundary and MIR-lowering tests
```

The Python driver invokes the in-tree Rust executable and reads one versioned
HIR JSON document per compilation unit. This follows the useful process and
replay boundary of the current WCC `.cgs` frontend without copying its
code-generator-call vocabulary. It avoids a Python/Rust FFI dependency and
keeps parser crashes or diagnostics above the MIR boundary.

The JSON artifact is a stage dump as well as transport: tests can replay it
without rebuilding the parser, and failures can diff source, resolved HIR,
and MIR independently. It is not a cache key or long-term object format in
the first implementation.

## Parser source and extraction

The starting parser is `~/work/personal/qbasic-port`, a Rust port of the
QBasic 1.1 parser. It is valuable for its grammar behavior, tokenization,
backtracking, and name/type rules. Its p-code coupling is not brought into
this tree.

The minimum extraction is:

- token and source-position handling;
- grammar tables and parser control;
- parser state needed for backtracking and error recovery;
- declaration/name/type parsing; and
- build tooling needed to generate checked-in parser tables reproducibly.

The extraction excludes:

- the p-code buffer and numeric p-code opcodes;
- the p-code executor and runtime;
- scanner passes that patch generated bytecode;
- editor/IDE state; and
- QBasic's in-memory module format.

Grammar `EMIT` actions become typed semantic-action identifiers. Parser marks
become typed builder checkpoints; abandoning a parse alternative rolls the
builder back to its checkpoint. Actions build private syntax or semantic
nodes, never append untyped words to an instruction stream.

Responsibilities formerly hidden in p-code scanning become named semantic
passes:

1. collect declarations and default typing state;
2. resolve symbols, labels, procedure signatures, and parameter modes;
3. insert required source conversions;
4. classify storage and array representation;
5. construct explicit control flow; and
6. verify and emit HIR.

Copied code retains provenance and its compatible license notices. The
in-tree copy is reduced to what the source frontend actually uses; the new
compiler must not depend on a sibling checkout at build time.

## The object raiser is the semantic reference

The existing OMF frontend already answers the difficult lowering questions
for compiler output. Its main raising sequence in
[`mir.py`](../../../qbopt/model/mir.py) recognizes whole LONG values,
arithmetic runtime helpers, floating evaluation, and numeric array access.

For a small program accepted by a legacy compiler, the validation loop is:

```text
source --BC/PDS/VBDOS--> OMF --object raiser--> canonical MIR A
source ------new QB frontend/HIR adapter-----> canonical MIR B
                                             diff A and B
```

The two paths need not have identical block/value numbers. They must agree on
operations, widths, signedness, floating semantics, memory objects, call
effects, and control flow. The expected answer is also derived by hand for the
probe; agreement between two implementations is not sufficient when both can
share a mistaken assumption.

The object raiser is not deleted when source compilation works. It remains an
input frontend, a corpus oracle, and a way to inspect differences among the
legacy compilers.

## Accuracy policy

Parser fidelity and compiler-bug compatibility are different goals. The
frontend follows documented and measured QB/PDS/VBDOS language behavior. It
does not intentionally reproduce a legacy optimizer miscompile, such as the
measured VBDOS optimized `NOT` condition behavior.

Where manuals, the recovered grammar, and compiler binaries disagree, a
minimal executable probe against each relevant compiler is authoritative.
Results are recorded in fixtures with compiler, flags, diagnostics, and
runtime output, not summarized only in prose.

See [dialects.md](dialects.md) for profiles and evidence,
[abi.md](abi.md) for measured parameter/descriptor conventions, and
[memory-model.md](memory-model.md) for real-mode storage rules.
[parser-provenance.md](parser-provenance.md) pins the clean upstream parser
revision and the extraction boundary.

## Non-goals

This work does not:

- change MIR, optimization passes, lowering, register allocation, or emission;
- introduce p-code or an interpreter stage;
- support protected mode or a flat address space;
- generate 8086- or 286-specific code;
- synthesize the legacy floating-emulator patch protocol for new modules;
- natively implement the BASIC runtime;
- inline strings, files, graphics, events, or arbitrary library calls; or
- replace the current C or OMF frontends.
