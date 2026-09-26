# Parser extraction provenance

The parser reference is the tracked tree at:

```text
repository  ~/work/personal/qbasic-port
commit      e468001b32a96a70c44b5767790de71698ef57c0
```

The sibling checkout contains unrelated uncommitted IDE work. Extraction must
therefore read the named commit with `git show`/`git archive`, never copy the
checkout's working files. The commit identifies itself as a Rust port of the
QBasic 1.1 parser and records `130c33405ce168466a2d6f2ea70340bdde634960`
as the original parser-pipeline landing revision.

## Mechanism to retain

The useful tracked components are the token rules in `src/prslex.rs`, parser
control and rollback in `src/prsmain.rs`, `src/prsnt.rs`, `src/prsnt1.rs`,
`src/prsstmt.rs`, expression parsing in `src/prsexp.rs`, identifier and
declaration parsing in `src/prsid.rs`, and deterministic table generation in
`crates/buildprs/` plus the generated tables.

The reference parser is line-oriented and immediately emits infix p-code.
That output contract is deliberately not retained. An in-tree extraction is
accepted only when:

- `EMIT` actions have named typed semantic-action equivalents;
- parser alternatives checkpoint and roll back a syntax/HIR builder rather
  than a word buffer;
- no `pcode`, executor, scanner patch, IDE, display, audio, or runtime module
  is in the frontend dependency graph; and
- an ordinary build and test never read the sibling repository.

The upstream `Cargo.toml` says `license = "MIT"`, but that checkout has no
tracked standalone license file at the pinned revision. Before copying source,
the extraction commit must preserve available file notices and record the
license basis explicitly rather than inventing one.

## Current in-tree result

Production currently uses the typed recursive parser under `crates/qbfront`.
Alongside it, the pinned port's `crates/buildprs` generator sources are
vendored under `crates/qbfront/crates/buildprs`, and the recovered production
grammar inputs are `crates/qbfront/grammar/qbasbnf.prs` and
`crates/qbfront/grammar/peropcod.txt`. Captured `buildprs.exe` output remains
under `crates/qbfront/fixtures/buildprs/qbasic-1.1` as a golden, not as a
production input.

The generated-table parser under `crates/qbfront/src/generated_parser` builds
the local typed syntax tree directly. It does not import p-code as an IR and
does not call the recursive parser from generated actions. During migration,
the recursive parser is the production path and differential oracle; the
generated parser is test-only until full structural parity. The current
measured checkpoint is 1 of 113 complete compatibility sources and 1,567
standalone source lines matching structurally after the label/procedure round.

The VBDOS profile currently parses every qb-qrender BASIC module, including
logical-line continuations, procedure declarations/bodies, UDT field syntax,
IF/ELSEIF, FOR, DO, WHILE, SELECT CASE, fixed-length strings, based integer
literals, and an explicit allowlist of runtime statements. QB 4.5/PDS/VBDOS
acceptance differences still require compiler probes; passing the VBDOS corpus
does not by itself establish the other profiles.

## Evidence to preserve

The reference documentation maps the port to recovered Microsoft modules
such as `prslex.asm`, `prsexp.asm`, and `qbasbnf.prs`. Those paths are useful
provenance, not build dependencies. Dialect extensions for QuickBASIC 4.5,
PDS 7.1, and VBDOS require independent compiler probes; QBasic 1.1 behavior
alone is not evidence that the later compilers agree.
