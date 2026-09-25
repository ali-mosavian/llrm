# Generated QB parser integration

## Source and license provenance

The generated parser is derived from the tracked `qbasic-port` commit
`e468001b32a96a70c44b5767790de71698ef57c0` (2026-06-21).  Files are read
from that Git object, never from the sibling checkout's dirty working tree.
That package declares `license = "MIT"` in both its root and `buildprs`
`Cargo.toml`; the commit does not contain a standalone license file.  The
vendored file headers and this provenance record are retained rather than
inventing a notice that is absent upstream.

The grammar itself is recovered Microsoft QBasic 1.1 source.  It is the QB
base grammar and compatibility evidence, not a FreeBASIC approximation and
not evidence for PDS 7.1 or VBDOS-only additions.

## Reused unchanged

The following host-side inputs have no dependency on p-code and are vendored
byte-for-byte from the pinned commit:

- `crates/buildprs/Cargo.toml` and every `crates/buildprs/src/*.rs` generator
  module;
- production inputs `grammar/qbasbnf.prs` and `grammar/peropcod.txt`; and
- captured `prstab`, `prsirw`, `prsorw`, `prsstate`, and `prsrwt` goldens under
  `tests/fixtures/buildprs/qbasic-1.1`, used only to prove that the host generator
  still describes the recovered tables.

The crate build script uses those inputs plus
`grammar/ast-actions.toml` to generate `T_STATE`, internal and external
nonterminal dispatch tables, statement/function anchor tables, reserved-word
numbers, and typed `AstAction`/`ExternalAction` dispatch. An ordinary build has
no dependency on the sibling repository or a DOS tool. The action schema names
grammar `EMIT` identities and external nonterminals symbolically; the build
fails when a name is absent from the generated grammar or opcode catalog.

## Adapted, not copied

The runtime parser boundary is implemented locally under
`crates/qbfront/src/generated_parser/`:

| qbasic-port reference | Local responsibility |
|---|---|
| `prslex.rs` | Tokenize QB source while retaining local spans; qbfront applies the universal VBDOS syntax superset. |
| `prsnt.rs` | Interpret generated state bytes and dispatch named external actions. |
| `prsstmt.rs` | Build local `syntax::Statement` values; never emit an opcode. |
| `prsexp.rs` | Build local typed expression syntax with QB precedence. |
| `prsid.rs` | Build local name/index/member/declaration syntax. |
| `prsmain.rs` | Parse a module and preserve action checkpoints across alternatives. |

`prscg`, `pcode`, scanner patching, executor, IDE, display, audio, and runtime
modules are intentionally excluded. Numeric grammar words exist only as input
to the generated table decoder. The decoder immediately turns each one into a
typed `AstAction`; an unmapped action retains its symbolic grammar identity as
`Unsupported("op...")`. Numeric action IDs and p-code-shaped word buffers do
not enter syntax, HIR, or MIR.

## Boundaries

The table interpreter operates on typed parser/builder state. Each alternative
checkpoints both the token cursor, the typed `AstSink`, and every builder
collection or mode it can change. Rejection restores all of them; acceptance
commits them. `MARK` and `EMIT` invoke the sink immediately, internal
nonterminals recurse through `T_INT_NT_DISP`, and external nonterminals are
decoded to generated `ExternalAction` variants before calling the hand-written
syntax builder. Statement shapes such as PRINT, GOTO, DIM, and runtime
statements come from generated action metadata rather than a second spelling
allowlist in the parser.

The hand-written syntax types are the intended lossless source boundary. The
current model retains names and effective written type, fixed string lengths,
source-order array bounds and dynamic/shared form, parameter BYVAL/SEG state,
procedure kind/static state, and source spans. It does not yet distinguish an
equivalent suffix from an `AS` spelling, nor model FAR/HUGE/COMMON or explicit
dialect origin. Accordingly, the `retains` entries in
`ast-actions.toml` claim only facts the current syntax AST or statement builder
can represent; they do not generate fields. The build rejects a retained-fact
claim outside that explicit AST vocabulary. A dialect extension must first
gain a hand-written syntax field and parser behavior before the schema may
claim the new source fact.

Semantic analysis, not parsing, resolves suffix/default/`AS` types and names.
It is also where a selected compiler/runtime profile supplies canonical type
sizes and layout, storage/address space and allocation provenance, array
descriptor ordering and bounds behavior, floating representation/evaluation,
and call ABI/effects. Profile-derived calling distance or memory-model facts
therefore belong in rich HIR unless source syntax explicitly spells them. The
parser neither chooses layout nor resolves alias or ABI policy.

The generated parser is the production `qbfront::parse` path. Before that
switch it matched all 79 manifest identities accepted by the former recursive
parser, including 1,848 independently parsed source lines. Unsupported
generated actions fail explicitly; production has no legacy fallback.

QB 4.5, PDS 7.1, and VBDOS all parse the highest VBDOS syntax superset.
Later constructs remain named generated-table extensions so their source facts
are explicit, but those extensions are inherited by every parser profile.
Profile selection begins at semantic/runtime/ABI resolution, not syntax
acceptance.

## Verification gates

1. The vendored generator reproduces the checked-in QBasic goldens and table
   dimensions.
2. The generated lexer scans all 2,160 mechanically ported qbasic-port source
   cases under every runtime profile while preserving token order and spans.
3. All 79 compatibility identities accepted before the production switch
   match checked-in, human-readable AST goldens exactly.  Those snapshots are
   legacy-derived: when the syntax model gains a field, all cases must first
   match the previous snapshots after projecting only that representation
   change.  The snapshots then move to the new schema together, rather than
   teaching the test to ignore the new field.
4. Parse counts are reported separately from semantic HIR, optimized MIR, OBJ,
   LINK, DOS execution, verdict, and artifact counts.
5. No case becomes runtime-green from parser acceptance alone.

The 2026-09-20 snapshot-schema migration checked all 79 identities in one
process.  After projecting only `Print.using`, `CaseItem::Value`, optional
`LineInput.file` plus `prompt`, and `Procedure.exported`, 78 snapshots matched
their legacy form exactly.  The remaining `qb45/memorymodel` source had been
changed by `11c6d519` while its snapshot still described the previous program;
its replacement was reviewed as a source-contract change.  The checked-in
snapshots now use the complete syntax model and the test performs an exact
comparison with no compatibility filter.
