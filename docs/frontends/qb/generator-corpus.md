# Generated QB source corpus

`tools/qbgen` vendors the source-semantic generators from qbasic-port commit
`e468001b32a96a70c44b5767790de71698ef57c0` (MIT).  Each vendored file was
checked against that Git object, not inferred from a p-code trace or adapted
from FreeBASIC.  The adapter has no dependency on the sibling checkout at run
time.

The ported set contains 2,131 cases, preserving the upstream generator
algorithms and their dimensions:

| Family | Cases |
| --- | ---: |
| ambiguous identifier | 287 |
| array access | 180 |
| assignment coercion | 76 |
| builtin arguments | 78 |
| control flow | 573 |
| expression promotion | 81 |
| name binding | 52 |
| parameter coercion | 139 |
| procedure calls | 265 |
| scanner source shapes | 185 |
| UDT access | 215 |

`families.smoke_cases()` adds 29 compact witnesses.  Those test a
parser-private AST category through syntax acceptance, actual semantic name
and type bindings observable in HIR, selected HIR operation shapes, and
per-dialect result gates.  They are deliberately additive: reducing or
deleting the full matrix is a failure even if the smoke layer passes.

Each generated case records a `DialectOutcome` for every profile it claims:
`accepted`, `syntax-error`, or `semantic-error`.  It is never a bare dialect
allow-list, so underscore and local-error gates cannot accidentally turn a
negative test into an accepted one.  Upstream negative assignment cases are
explicit semantic errors.  An upstream known failure, if present, is retained
as a `upstream-known-failure:` classification rather than silently becoming an
accepted llrm case.

`uv run python -m tools.qbgen OUTPUT` writes every `.BAS` source in ASCII with
CRLF lines, an 8.3 name, and a `MANIFEST.JSN` (also 8.3).  The manifest carries
input hashes, AST category, semantic/HIR expectations, dialect outcomes, and
DOS verdict text.  The output may therefore be mounted directly into the DOS
test environment.

The imported `generate_scan_pcode_cases.py` is handled specially: it supplies
only source scanner/lexer shapes and expected BASIC output.  Its p-code opcode
and type fields are intentionally not read by llrm and cannot become HIR or
MIR assertions.

The dedicated `uv run pytest --full tests/test_qbgen.py` gate runs generator
determinism, exact matrix cardinality, 8.3/CRLF output, negative-outcome
validation, mutation sensitivity, and four real frontend stage witnesses.
The exhaustive `tools.qbgen.verify.verify` run is opt-in because it launches
the frontend for all 2,160 generated and smoke inputs; this keeps generated
testing under the project's wall-clock budget while retaining a truthful full
sweep command.
