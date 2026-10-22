# lalrspike

Spike: LALR(1) grammars for Nib and the QB family. Wherever the grammar cannot
tell two readings apart, one node keeps the written shape and a later pass
(symbol table, semantic) chooses. Not in the workspace; `cargo test` here.

lalrpop in `#[LALR]` mode. Both grammars have no conflicts. The Nib grammar is
`src/nib.lalrpop`, hand-written from the language spec's grammar. The QB one is
generated: `python3 prs2lalrpop.py ../qbfront/grammar/qbasbnf.prs > src/qb.lalrpop`
turns the recovered QBasic grammar (statements, functions, internal
nonterminals) and `dialect-extensions.toml` into rules building a generic
tree, and splices in `src/qb_hand.part`, which is the 49 externals plus the
line structure and expressions. Both parsers reuse the existing lexers.

## Against the existing parsers

Every `.nib` and `.bas` file in the repository (`tests/corpus.rs`), plus 516
Nib programs and fences pulled out of the tests' string literals and the docs
(`src/bin/nibcheck.rs` runs any list of files).

| | agree: accept | agree: reject | old only | LALR only |
|---|---|---|---|---|
| Nib, 248 files | 248 | 0 | 0 | 0 |
| Nib, test snippets and doc fences | 414 | 93 | 0 | 7 |
| QB family, 591 files | 571 | 8 | 2 | 10 |

LALR only, Nib: what the hand parser checks while parsing and the grammar
leaves to later passes: constant folding (`const X = f()`, unknown constant),
`@repr`/`@extern` argument validation, a bodiless `fn` without `@extern`.
LALR only, QB: `COMMON`, `NAME`, `PCOPY` and `OPEN` options, which parse but
the table-driven builder has no syntax-tree node for.

Old only, QB: `dim point as Point` (a function's name as a variable, which
the old parser accepts as a "contextual name") and `PRIVATE FUNCTION`
(QuickrBASIC, a token prepass in the old parser). Not done.

## Deferred to later passes

| Written | Node | Decided by |
|---|---|---|
| Nib `f(x)`, `i16(x)`, `Point(1, 2)`, `.some(x)` | `Apply` | symbol table: call, cast, construction, variant |
| Nib `a[i]`, `f[T](x)`, `T[4]`, `T[N]` | `Subscript` | symbol table: index, generic call, array type |
| Nib `a.b` | `Field` | module member, field, method, qualified variant |
| Nib `f(x) = 1` | `Assign` of any expression | semantic: is it a place |
| Nib `for x in &mut v` | `Unary("&mut", v)` | semantic: iteration mode |
| Nib `let x`, `let Some(x)` | one `Pattern` | name or binding |
| Nib `*far T` | qualifier is a name | semantic |
| QB `a(1)` | `Apply` | symbol table: array element or function call |
| QB `foo (a), b`, `foo(a)` | `CallApplied*` | symbol table: call or parenthesised argument |
| QB `foo` alone on a line | `Call` | symbol table: call, variable, label use |
| QB `RUN 100`, `RESUME 0`, `OPTION BASE 1` | an expression | semantic: line number or label |
| QB `ON PLAY(3) GOSUB x` | `ON` over a function call | semantic: event trap |
| QB `IF`/`ELSEIF`/`ELSE`/`END IF`, loops, `SUB` bodies | a flat list of lines, with `THEN`, `ELSE`, `:` as separators | block nesting pass |

## What LALR(1) could not do alone

- **Nib `?`**: `a ? b : c` or `x?` needs the `:` further on the same line.
  One token-stream rule (`nib_tokens.rs`, the old parser's own scan) marks the
  conditional's `?`. The same for `a is not b`, where `not` is also a name.
- **QB: PEG into CFG.** The recovered grammar is ordered choice and greedy
  optionals; as a context-free grammar these are ambiguous and had to be
  written as the nesting they mean: `commaOptExp commaOptExp` in `PAINT` and
  `VIEW`, `LINE`'s tail, `RUN`/`RESUME`/`OPTION BASE`/`ON`, where `Lit0` and
  `LabLn` are both an integer, and `KEY (n) ON` against `KEY n, text$`. Its
  `|` joins the items on either side, not whole sequences (`PUT ..., PSET`).
- **QB: reserved words as names**: a `TYPE` member or a field named `pos`,
  `name`, `point` takes the word as a name in the places a name must stand.
  The old parser's "contextual names" are broader than that.
- **QB: `IF`**: the dangling else is gone because nothing nests.
- **QB: the old parser's token prepasses** (`FUNCTION f() AS type`,
  `VOLATILE`) are productions here.

## Not done

QuickrBASIC (`+=`, `FOR IN`, tuple assignment, `a IF c ELSE b`, `PRIVATE`,
f-strings); a typed tree from the QB generic tree (the old builder's
`syntax::Module`) and from the Nib one (`llrm_nib::syntax`); error messages;
speed against the old parsers (the QB tables are a 2 MB generated source, and
the crate takes about 30 s to build in debug).
