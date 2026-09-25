# Generated-parser and generated-test coverage audit

This audit compares the recovered QBasic parser machinery in the pinned
`qbasic-port` source with the local QB/PDS/VBDOS compatibility ladder.  It is
an implementation acceptance checklist, not a claim that the frontend is
already language-complete.

## Facts established from the recovered source

The recovered production grammar and opcode catalog are
`crates/qbfront/grammar/qbasbnf.prs` and `peropcod.txt`. The original
`buildprs` outputs captured under `tests/fixtures/buildprs/qbasic-1.1` are goldens,
not compiler inputs. The vendored host generator parses the production inputs
deterministically and carries enough
information to regenerate the parser tables without DOS or a sibling checkout.
The grammar has:

| Grammar part | Count | What must remain true in qbfront |
| --- | ---: | --- |
| lexical tokens | 246 | Token spelling, lexical flags, declaration order, and the special contiguous type-character group remain generated data. |
| statement anchors | 115 | Statement dispatch originates from the generated anchor table, not a handwritten keyword switch that silently drifts. |
| function anchors | 84 | Built-in syntax dispatch originates from the generated function table; intrinsic meaning is resolved later. |
| internal nonterminals | 29 | `T_INT_NT_DISP` recursion, alternatives, optionals, repeats, `MARK`, `EMIT`, and `<INDEX>` behavior are interpreted by the table engine. |
| external nonterminals | 49 | Every generated `T_EXT_NT_DISP` name is registered to a typed local action and has an isolated test. |

The table is not an AST and it is not an IR.  Its job is to choose a production
and schedule named actions.  The local action boundary must produce source
spans and `syntax::{Statement, Expr, Declaration, Procedure}` pieces; semantic
analysis then resolves names, types, intrinsic overloads, runtime contracts and
the HIR.  Numeric `EMIT(op...)` words are stable *action identities* only.
Neither a p-code word buffer nor a scan/execute phase may appear between the
parser and HIR/MIR.

The table engine must preserve the original speculative-parse mechanism:

```text
token cursor + syntax-action checkpoint
        | table alternative / internal NT / external action
        +-- reject -> restore both
        `-- accept -> commit both
```

Restoring only the token position is insufficient: an alternative which emits
a declaration, mark, list item, or partial expression before failing otherwise
poisons the next alternative.  This is a parser correctness invariant, not an
error-recovery nicety.

### The 49 required external-action families

The original table deliberately delegates context-sensitive work rather than
encoding it in state bytes.  The local registration must retain all of these
families, even where more than one table name shares an implementation:

- declaration context: `ACTIONidCommon`, `ACTIONidShared`, `ACTIONidStatic`,
  `ConstAssign`, the five `Deflist*` forms, `IdFuncDecl`, `IdFuncDef`,
  `IdSubDecl`, `IdSubDef`, `IdParm`, `IdNamCom`, and `IdType`;
- expression and lexical boundaries: `Exp`, `LitString`, `Lit0`, `Lit1`,
  `CaseRelation`, `CommaNoEos`, `EndPrint`, `EndPrintExp`, `RwB`, `RwF`, and
  `RwBF`;
- name ambiguity and references: `Assignment`, all `IdAry*` forms,
  `IdArray`, `IdCallArg`, `IdFor`, `IdFn`, and `IdSubRef`;
- control-flow and line structure: `IfStmt`, `LabLn`, `Ln`, `Statement`,
  `StatementList`, and `ErrIfNot1st`; and
- bounded argument and printing forms: `NArgsMax3`, `NArgsMax4`, and
  `NArgsMax5`.

The action registration is an API surface.  A test which merely checks that an
unknown action errors does not cover it; each name needs an input which takes
its success path, an input which makes its nearest competing alternative win,
and an observable syntax or HIR consequence.

## What is a QB base rule, and what is not

`qbasbnf.prs` is the QBasic 1.1/QB-family parser baseline.  It is powerful
evidence for common tokenization, expression precedence, statement shape,
`PRINT` separators, array/reference ambiguity, procedure declarations, labels,
and `MARK` placement.  It is **not** evidence that PDS 7.1 or VBDOS accepts an
extra token merely because that token name happens to appear in the recovered
source.  The grammar itself contains historical token names and comments such
as removed `CDECL`/`ALIAS` support; token existence is not production
acceptance.

| Layer | Retain from the recovered table | Local profile additions requiring separate evidence |
| --- | --- | --- |
| Universal source syntax | 246-token lexer, 115 statement and 84 function anchors, generated precedence/structural forms, comments/DATA, `DEFxxx`, arrays, procedures, labels, ordinary file/graphics/event syntax, underscores, continuations, `OPTION EXPLICIT`, `ON LOCAL ERROR`, and `CDECL ALIAS`. | Unsupported forms remain named table/action obligations, never silent legacy fallback. |
| QB 4.5 semantic/runtime profile | The full source-syntax superset remains parseable. | QB 4.5 diagnostics, compiler directives, include behavior, type/runtime availability, memory model, and exact ABI. |
| PDS 7.1 semantic/runtime profile | The full source-syntax superset remains parseable. | `CURRENCY`/`CCUR`/`CVC`/`MKC$`; `/MBF`; `/FPa`; huge/far storage; row-major and checked-array modes; PDS quick-call and PDS-specific runtime/profile behavior. |
| VBDOS semantic/runtime profile | The full source-syntax superset remains parseable. | Date/time behavior, `SSEG`/`SSEGADD`, `/G3`, interrupt/absolute declarations, forms, controls, object/event, dialog, and finance runtime/ABI surfaces. |

ISAM and OS/2 `SIGNAL` work remain out of scope as directed.  Their names may
exist in a recovered token catalogue, but they must not inflate parser or
runtime coverage.

## Existing generated source families

`qbasic-port/tools` contains eleven source-test generators.  Its checked-in
generated Rust corpus currently has **2,131 cases**, far larger and more
combinatorial than the 113 handwritten DOS ladder cases.  The valuable input
generation must be vendored/adapted in-tree; generated expected p-code must
not be carried into qbfront.

| Generator family | Cases | QBfront use | Current ladder relationship / missing dimensions |
| --- | ---: | --- | --- |
| `generate_ambiguous_id_cases` | 287 | scalar/array/call/implicit-name ambiguity; success and known-reject partitions | The ladder has representative arrays and procedures, but no exhaustive ambiguity matrix or diagnostic oracle. |
| `generate_array_access_cases` | 180 | dimensions, element/reference form, bounded/static/dynamic arrays | Add lower-bound, column/row-major, huge/far and checked-mode variants per profile. |
| `generate_assignment_coercion_cases` | 76 | scalar, array, UDT-field and function-result stores; conversion rejects | Add 16-bit QB overflow, `CURRENCY`, MBF and fixed-string assignment profile variants. |
| `generate_builtin_arg_cases` | 78 | arity, parenthesization and malformed built-in calls | Replace p-code expectations with syntax shape plus semantic intrinsic-table selection.  Cover every locally supported intrinsic. |
| `generate_control_flow_cases` | 573 | nested `FOR`/`DO`/`WHILE`, exits, `GOTO`/`GOSUB`, unwind depth | Adapt the sources and outcome witnesses.  Do not import the p-code scan oracle; retain its control-flow construction dimensions. |
| `generate_expression_promotion_cases` | 81 | operator/type lattice and result type | Extend with long/float inline lowering, IEEE/MBF, `CURRENCY`, and exact BASIC boolean/rounding cases. |
| `generate_name_binding_cases` | 52 | `DEFxxx`, suffixes, shadowing, COMMON, function-result names, UDT collisions | Add profile-gated underscores, `OPTION EXPLICIT`, module/procedure default scope, and cross-module names. |
| `generate_param_coercion_cases` | 139 | BYREF/BYVAL, numeric/string coercion, intrinsic and function results | Add real-mode far/SEG/array/UDT, hidden float/string results, PDS quick-call, and VBDOS `/G2`/`/G3` ABI surface tests. |
| `generate_procedure_call_cases` | 265 | declaration/definition/call permutations and argument shapes | Add `DECLARE ... CDECL ALIAS`, `ABSOLUTE`, interrupt, and module-link variants with Microsoft-runtime observables. |
| `generate_scan_pcode_cases` | 185 | control-flow source combinations | Source generator is useful; p-code scanner assertions are explicitly excluded.  Recast as syntax CFG and emitted-DOS verdict tests. |
| `generate_udt_access_cases` | 215 | member access, aggregate/array/nested UDT combinations | Add packed layout, fixed strings, BYREF/BYVAL, and PDS/VBDOS extension records. |

The imported generator runner should emit deterministic DOS 8.3, CRLF `.BAS`
files and a manifest.  Every runtime-capable program prints only via bare
`PRINT`, so DOS redirection is meaningful; screen evidence remains a separate
`BSAVE &HB000`/`&HA000` artifact test.  Generated test names must be stable so
that a failing source, its HIR/MIR/LIR/assembly stages, and its DOS output are
all addressable by one case id.

## Comparison with the 113-case compatibility ladder

The local manifests contain 74 QB 4.5 cases, 16 additive PDS cases, and 23
additive VBDOS cases.  They are valuable end-to-end probes: their explicit
`parser`, `lowering`, and `runtime` fields prevent a parse-only success from
being presented as a runtime result, and their output/artifact checks are
intentionally nontrivial.  They do not cover every generated grammar choice.

The largest parser-facing gaps are:

1. No mechanical statement/function-anchor coverage map for all 115/84
   grammar entries.
2. No map proving all 49 external actions have both positive and rollback
   witnesses.
3. No in-tree import of the eleven source generators or a regeneration check.
4. No dialect-delta matrix that classifies an item as inherited, QB-only,
   PDS-only, VBDOS-only, rejected, reference-only, or excluded.
5. No negative corpus large enough to catch an overly permissive union grammar
   (especially CDECL/ALIAS, underscore/continuation, type/array forms, and
   compiler directives).
6. The help-gap inventory remains deliberately incomplete; its existing 1,865
   gaps mean 113 tests cannot be read as full-language compatibility.

## Acceptance metrics for replacing the hand parser

The generated parser may become the production parser only when all gates
below are measured and written to the test report.

1. **Generator fidelity.** Rebuild parser artifacts solely from the vendored
   grammar/opcode source; byte-compare `prstab`, `prsirw`, `prsorw`,
   `prsstate`, and `prsrwt` with their captured QBasic artifacts.  Check the
   dimensions 246/115/84/29/49 explicitly.
2. **Table/action completeness.** Assert that every statement and function
   anchor has a table offset, every external name has exactly one local action,
   every local action is reachable, and no action produces p-code.  Instrument
   branch/alternative hits while running the generated corpus; require one
   positive and one rollback/rejection witness for each reachable alternative.
3. **Source-generator fidelity.** Vendor all eleven generators and their
   small common renderer.  Regeneration in a clean tree must be byte-stable.
   Port all 2,131 QB source cases to parser/semantic tests or classify each
   nonportable expectation with a concrete reason.  The scan-pcode family is
   classified as "source retained, p-code oracle removed," never silently
   dropped.
4. **Legacy differential period.** Until the handwritten parser is removed,
   parse every source accepted by either parser with both.  Compare normalized
   syntax, spans, profile gate, and diagnostic class—not p-code.  A difference
   needs a minimized fixture in the same change.  Unsupported generated actions
   must fail explicitly, never fall back to the old parser.
5. **Dialect monotonicity and rejection.** Run the common QB corpus under all
   profiles where the original compilers accept it.  For every PDS/VBDOS delta,
   test accepted profile(s) and rejected earlier profile(s), and record the
   original compiler's severe-error outcome.  Do not infer acceptance from an
   OBJ existing on disk.
6. **Whole pipeline, reported separately.** For all 113 manifest cases report
   `lex -> table parse -> semantic HIR -> MIR -> optimized MIR -> OBJ -> LINK
   -> DOS -> exact verdict/artifact` counts.  The number at each boundary may
   only increase after fixing the preceding boundary; an end-to-end green count
   is never used to conceal parse failures.
7. **Mutation resistance.** For every generated family, mutate at least one
   relevant operator, bound, argument order, selector, or expected branch and
   demonstrate that its syntax/HIR assertion or its DOS verdict changes.  This
   prevents a wrong parse from passing because a later error happens to cancel
   it.

The old recursive parser can be deleted only after gates 1--6 are green for
its currently accepted corpus and gate 7 is exercised for each family.  The
table engine and generators then become reusable frontend infrastructure:
another language can supply a grammar/table, token adapter, typed action sink,
profile gates, and generator manifest without importing any QB semantic or
runtime behavior.
