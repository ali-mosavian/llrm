# Legacy parser removal record

The handwritten recursive parser and its scanner were removed after the
generated-table frontend became the production `qbfront::parse` path. There is
no fallback, p-code buffer, or second parser implementation.

## Completed gate

Before deletion, the generated parser was compared directly with the old
parser over the complete compatibility manifest:

| Measurement | Result |
|---|---:|
| Manifest identities | 113 |
| Old parser accepted | 79 |
| Generated parser structurally equal | 79/79 |
| Independently equal nonblank source lines | 1,848 |
| Old-only accepted paths | 0 |

The two `/G2` and `/G3` cases sharing `vbdos/g3long.bas` remain separate
manifest identities. `CDECL ALIAS`, explicit function result types, PDS/VBDOS
`ON LOCAL ERROR`, and VBDOS `OPTION EXPLICIT` were part of the equality gate.

After the production switch, all 87 existing parser/semantic regressions also
passed. That second gate caught syntax shapes absent from the 79 snapshots:
`AS ANY`, contextual reserved identifiers, disk `INPUT`, REDIM forms, and
positionless `GET`/`PUT`.

## Durable oracle

`frontends/qb/fixtures/legacy_ast_goldens/accepted-cases.tsv` indexes 79
human-readable AST snapshots. `frontends/qb/tests/generated_parser.rs` requires
every identity to parse and match its snapshot exactly. The snapshots are a
frozen migration record and must never be regenerated from the production
parser, because that would turn a regression into its own expected answer.

The generated lexer separately scans all 2,160 mechanically ported
qbasic-port source cases under QB45, PDS71, and VBDOS profiles, for 6,480
complete scans, checking nonempty monotonic token streams and valid spans.

## Universal syntax policy

The parser accepts the highest VBDOS syntax superset for every profile. QB45,
PDS71, and VBDOS remain semantic/runtime/ABI selections, not syntax gates.
This includes underscore identifiers, continuations, `OPTION EXPLICIT`,
`ON LOCAL ERROR`, and `CDECL ALIAS`.

The recovered `qbasbnf.prs` remains the generated base. Later syntax is kept
visible in `grammar/dialect-extensions.toml`; despite its historical filename,
those extensions are inherited universally and carry no dialect mask.

## Remaining language-completeness work

Removal parity is complete, but it is not a claim that every one of the 49
recovered external nonterminals has a rich typed implementation. Unmapped or
unsupported grammar actions still fail symbolically. The remaining work is to
model those source facts generally—especially COMMON/STATIC storage, CASE
relations, call-site SEG, and specialized file modifiers—then extend the
VBDOS-superset corpus. It must not reintroduce a second parser or encode a
runtime-profile rejection in syntax.
