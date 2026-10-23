# lalrspike

Spike: LALR(1) grammars for Nib and the QB family. Wherever the grammar cannot
tell two readings apart, one node keeps the written shape and a later pass
(symbol table, semantic) chooses. Not in the workspace; `cargo test` here.

Both grammars are hand-written lalrpop files (`src/nib.lalrpop`, from the
language spec's grammar; `src/qb.lalrpop`, with the recovered `qbasbnf.prs`
as the reference) and reuse the existing lexers. The QB one is the recovered
grammar's ordered choices and greedy optionals written as the context-free
shapes they mean; an earlier version generated from the `.prs` is in git history.

**LALR(1), checked.** lalrpop 0.23 always builds with its lane-table
algorithm and `#[LALR]` does nothing, so "no conflicts" from it means LR(1).
`python3 lalrpop2bison.py src/qb.lalrpop` converts a grammar (macros
expanded, `x?` and `x*` inlined as lalrpop does) and runs bison, which builds
true LALR(1) tables: no conflicts for either grammar.

## Against the existing parsers

Every `.nib` and `.bas` file in the repository (`tests/corpus.rs`), plus 516
Nib programs and fences pulled out of the tests' string literals and the docs
(`src/bin/nibcheck.rs` runs any list of files).

| | agree: accept | agree: reject | old only | LALR only |
|---|---|---|---|---|
| Nib, 248 files | 248 | 0 | 0 | 0 |
| Nib, test snippets and doc fences | 414 | 93 | 0 | 7 |
| QB family, 591 files | 572 | 8 | 1 | 10 |

LALR only, Nib: what the hand parser checks while parsing and the grammar
leaves to later passes: constant folding (`const X = f()`, unknown constant),
`@repr`/`@extern` argument validation, a bodiless `fn` without `@extern`.
LALR only, QB: `COMMON`, `NAME`, `PCOPY` and `OPEN` options, which parse but
the table-driven builder has no syntax-tree node for.

Old only, QB: `prelude.bas`, which uses QuickrBASIC's `x += 1` (a token
prepass in the old parser; as a production it clashes with `foo -1`, a call
with a negative argument). Not done.

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
- **QB: PEG into CFG.** Ordered choice and greedy optionals are ambiguous as
  a context-free grammar. Trailing optional arguments (`PAINT`, `VIEW`,
  `LINE`, `CIRCLE`) are one list of `, [exp]` whose length a later pass
  checks, and `B`/`BF` in `LINE` are names. `Lit0`, `Lit1` and `LabLn` are
  all an integer; `KEY (n) ON` is `KEY` of an expression. The recovered
  grammar's `|` joins the items on either side, not whole sequences
  (`PUT ..., PSET`).
- **QB: reserved words as names**: a function's name (`point`, `pos`) is a
  variable where no `(` follows, a `TYPE` member or field may be any word
  (`name`, `pos`), and `PRIVATE` is a name before `SUB`/`FUNCTION`. The old
  parser's "contextual names" are broader than that.
- **QB: `IF`**: the dangling else is gone because nothing nests.
- **QB: the old parser's token prepasses** (`FUNCTION f() AS type`,
  `VOLATILE`) are productions here.

## Not done

QuickrBASIC (`+=`, `FOR IN`, tuple assignment, `a IF c ELSE b`, f-strings); a typed tree from the QB generic tree (the old builder's
`syntax::Module`) and from the Nib one (`llrm_nib::syntax`); error messages;
speed against the old parsers (the generated parser sources are large, and the
crate takes about 30 s to build in debug).
