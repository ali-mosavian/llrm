# QB source generators

`tools/qbgen` is the in-tree successor to qbasic-port's source-semantic test
generators. `qbasic_port/` vendors the original deterministic algorithms and
their complete **2,131-case** source matrix: binding, promotion, assignment
coercion, arrays, procedure calls, UDT access, intrinsic argument checking,
control flow, ambiguous identifiers, scanner syntax, and parameter coercion.
`ported.py` adapts every source case; the smaller high-information cases are
an additive smoke layer, never a replacement for the full matrix.

Run this to materialize reproducible inputs:

```sh
uv run python -m tools.qbgen /tmp/qbgen
```

Add `--verify` for the deliberately expensive full parser/semantic/HIR sweep.

Every emitted `.BAS` name is DOS 8.3 and every source file is ASCII CRLF.  The
manifest records the parser-AST category, semantic binding/type facts, HIR
operations, and any executable output witness.  `tools.qbgen.verify.verify`
is an intentionally opt-in full frontend check; ordinary unit tests sample
the stage contracts to keep test time bounded.

The public frontend boundary is HIR, so Python cannot inspect parser-private
Rust nodes directly.  `syntax_checked` proves a case traversed the parser;
the manifest's AST label identifies the expected source node; HIR checks then
prove the binding/type effect is not merely accepted and discarded.  No
generated test treats p-code as an intermediate representation. The old
scan-pcode generator contributes source scanner shapes only; its opcode/type
expectations do not cross into qbopt.
