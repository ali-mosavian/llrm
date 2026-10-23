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
| QB, `ali-mosavian/mgl` (180 `.bas`, 39 `.bi`) | 194 | 15 | 0 | 10 |
| QB programs in the tests' sources, 487 | 411 | 20 | 42 | 14 |

LALR only, Nib: what the hand parser checks while parsing and the grammar
leaves to later passes: constant folding (`const X = f()`, unknown constant),
`@repr`/`@extern` argument validation, a bodiless `fn` without `@extern`.
LALR only, QB: `COMMON`, `NAME`, `PCOPY` and `OPEN` options, which parse but
the table-driven builder has no syntax-tree node for.

mgl: the 15 both refuse are mostly lexer errors (a stray CR, an unterminated
string, a backtick) or constructs the old builder lacks. The 10 LALR-only
use dotted names with a reserved word in them (`UGL.Z.OFF%`) or `If(` with no
space, which the old parser stops at. Found and fixed with mgl: dotted and
keyword-led `CONST` names (`TFX.MASK%`, `mod.mem`), `ERROR$`, and
`foo (a) * b` as a call whose first argument goes on past its parenthesis
(arithmetic continuation only; `foo (a) = b` stays an assignment).

Old only, QB test-source programs: 41 of the 42 are QuickrBASIC (`+=`, `FOR
i AS INTEGER IN ...`, f-strings, `a IF c ELSE b`); the other is `VOLATILE` as a
type name. The 14 LALR-only are error cases the old parser refuses later.

Old only, QB files: `prelude.bas`, which uses QuickrBASIC's `x += 1` (a token
prepass in the old parser; as a production it clashes with `foo -1`, a call
with a negative argument). Not done.

## Against the real BC.EXE

`python3 bcdiff.py [--root DIR] ~/llrm-private-tools/qb45 FILE.bas...` runs
Microsoft's `BC.EXE` (QB 4.5) and the grammar on each file and lists the
disagreements. BC runs on the dosrun fork of DOSBox-X (`bcprobe.py`; four
booted machines per core, a forked job per file, a tqdm bar). 1,300+ programs:
the QB 4.5 examples, `GORILLA.BAS` and `NIBBLES.BAS`, the repo's files, the
test sources' programs and mgl (in place, so its `$INCLUDE`s resolve).

**BC accepts and the grammar refuses: none.** BC refuses and the grammar
accepts: about 250, which are `OPTION EXPLICIT`, the VBDOS/PDS/Quickr forms
(`AS` return types, `UNSIGNED`, `BYTE`), missing includes, `TYPE not defined`,
`Duplicate definition`, and a few syntax errors BC 4.5 reports that the
grammar is too lax to (`DIM a() AS INTEGER` outside `$DYNAMIC`). BC's own
checks, not the parser's, are what the later passes have to reproduce.

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
  variable where no `(` follows, any statement word but the structural ones
  (`NAME`, `WIDTH`, ...) is one in an expression or a declared name (not at a
  statement start, nor `ERROR`/`IS`/event words, which clash), a `TYPE` member
  or field may be any word, and `PRIVATE` is a name before `SUB`/`FUNCTION`.
- **QB: `IF`**: the dangling else is gone because nothing nests.
- **QB: the old parser's token prepasses** (`FUNCTION f() AS type`,
  `VOLATILE`) are productions here.

## Not done

QuickrBASIC (`+=`, `FOR IN`, tuple assignment, `a IF c ELSE b`, f-strings); a typed tree from the QB generic tree (the old builder's
`syntax::Module`) and from the Nib one (`llrm_nib::syntax`); error messages;
speed against the old parsers (the generated parser sources are large, and the
crate takes about 30 s to build in debug).
