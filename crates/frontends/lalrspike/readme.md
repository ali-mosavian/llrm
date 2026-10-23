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

`python3 bcdiff.py [--root DIR] QB45,PDS71,VBDOS FILE.bas...` runs Microsoft's
`BC.EXE` of all three families (QB 4.5, PDS 7.1, VBDOS; the directories are
`llrm-private-tools/qb45`, `/pds71`, `/vbdos`) and the grammar on each file
and lists the disagreements. BC runs on the dosrun fork of DOSBox-X
(`bcprobe.py`, a tqdm bar). Programs the grammar must take are those some BC
takes:

| corpus | some BC accepts | grammar takes them | grammar refuses |
|---|---|---|---|
| QB 4.5 examples, GORILLA, NIBBLES, VBDOS and PDS sources (128) | 123 | 123 | 0 |
| the repo's `.bas` files (599) | 560 | 560 | 0 |
| the test sources' programs (487) | 363 | 363 | 0 |
| mgl, in place so `$INCLUDE`s resolve (180) | 135 | 135 | 0 |

The last four refusals were the lexer's, shared with the table-driven parser:
the `@` currency suffix on names and numbers (PDS and VBDOS), VBDOS's doubled
quote inside a string, and `1_` as a continuation with no space. They are fixed
in `qbfront`'s lexer, each only under the dialects whose BC takes it, with
regression tests; the semantic layer refuses a CURRENCY name or number, as the
compiler has no such type.

Where every BC refuses and the grammar accepts (about
140): `OPTION EXPLICIT` in QB 4.5, dialect forms in the wrong dialect, missing
includes, `TYPE not defined`, `Duplicate definition`, `ON PLAY` without the
event option, and a few syntax errors a BC reports that the grammar is too
lax to. BC's own checks, not the parser's, are what the later passes have to
reproduce.

The first version of the runner took 3m50s for 128 files on three compilers,
211 s of it PDS 7.1's. Its `BC.EXE` is an NE image and the fork's debug-symbol
watch (`WatchForLoader`) rescans all of RAM for its 13 segments on every newly
executed page, about 4.5 s of host CPU a job for 40 emulated ms. `bcprobe.py`
sends `:nosymbols`, a job option added by `dosbox-nosymbols.patch` (apply it
to the `dosrun` branch of the fork and rebuild; a fork without it runs the
line as a shell command and PDS stays slow). With that, one shared queue of
(compiler, file) pairs over four machines per core: 28 s.

## A generated lexer

`src/qb_lexer.rs` is a [logos](https://docs.rs/logos) lexer with the old lexer's
token stream (`qbfront::generated_parser::Token`, same spans). logos dispatches on
the first byte and matches the reserved words with a DFA generated from
`qbasbnf.prs` by `gen_words.py` (`src/qb_words.rs`, 221 words, case-insensitive);
callbacks keep the old rules for numbers, strings, `REM`/`'` directives and the
`.`/digit context forms. QuickrBASIC's `f"…"` dialect falls back to the old lexer.
Logical lines are borrowed unless an `_` continues them.

`tests/qb_lexer.rs` is the check: equal tokens or equal error on every tracked
`.bas`/`.bi` in all four dialects, on every prefix of the first 60 lines of every
7th file (error paths), and on a list of odd forms. It passed on the first run.

Release build, 737 files, 766 KB: old lexer 33.5 ms (23 MB/s), logos 12.5 ms
(63 MB/s), 2.7x. The old one's cost was the linear scan of the token table, up to
three times per word, plus `String` per logical line.

### Feeding lalrpop from the lexer

`parse_qb_stream(source, dialect, &mut spans)` hands lalrpop an iterator over the
logos lexer: no `Vec<Token>`, no text for identifiers, strings and reals (the
parser needs kinds only; each token's span goes to `spans`, indexed as the tree's
`At` nodes, so a later pass re-reads the text from the span). `terminal()` is now an
array lookup instead of a hash lookup per token. QuickrBASIC's dialect is not
streamed. `streaming_tokens_to_the_parser_builds_the_same_tree` asserts the same
tree and the same spans as the vector path on every tracked program in three
dialects.

Same 737 files: old lexer + token vector + lalrpop 73.6 ms (10.7 MB/s); streamed
49.8 ms (15.8 MB/s). The parser itself is 36.9 ms of that, so lexing is now about
a quarter of the time and lalrpop's tables the rest.

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
