# lalrspike

The QB front's only parser is the hand-written recursive-descent one in `crates/frontends/qb/src/front/ast`. This directory holds the Nib LALR spike and the QB oracle tools (BC.EXE programs, results, `tools/oracle/qb-bc/bcprobe.py`, `verdict`, `checkfiles`); the QB LALR grammar and the first RD spike were deleted.

Spike: an LALR(1) grammar for Nib. Wherever the grammar cannot
tell two readings apart, one node keeps the written shape and a later pass
(symbol table, semantic) chooses. Not in the workspace; `cargo test` here.

The grammar is a hand-written lalrpop file (`src/nib.lalrpop`, from the
language spec's grammar) and reuses the existing lexer. The QB grammar
(`src/qb.lalrpop`, from the recovered `qbasbnf.prs`) was deleted; it is in git
history.

**LALR(1), checked.** lalrpop 0.23 always builds with its lane-table
algorithm and `#[LALR]` does nothing, so "no conflicts" from it means LR(1).
`python3 lalrpop2bison.py src/nib.lalrpop` converts a grammar (macros
expanded, `x?` and `x*` inlined as lalrpop does) and runs bison, which builds
true LALR(1) tables: no conflicts (106 terminals, 113 rules).

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

Environment: `LLRM_PRIVATE_TOOLS` is the directory holding `qb45`, `pds71` and `vbdos` (each with `BC.EXE`);
`DOSBOX_BIN` the dosrun fork's `dosbox-x` (else a `dosbox-x-fork` checkout beside the repository or in `$HOME`);
`MGL_DIR` a checkout of `ali-mosavian/mgl` (`corpus/real.py`, `tools/oracle/qb-bc/bcfix.py` only).

`python3 tools/oracle/qb-bc/bcdiff.py [--root DIR] QB45,PDS71,VBDOS FILE.bas...` runs Microsoft's
`BC.EXE` of all three families (QB 4.5, PDS 7.1, VBDOS; the directories are
`llrm-private-tools/qb45`, `/pds71`, `/vbdos`) and the grammar on each file
and lists the disagreements. BC runs on the dosrun fork of DOSBox-X
(`tools/oracle/qb-bc/bcprobe.py`, a tqdm bar). Programs the grammar must take are those some BC
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
executed page, about 4.5 s of host CPU a job for 40 emulated ms. `tools/oracle/qb-bc/bcprobe.py`
sends `:nosymbols`, a job option added by `tools/oracle/qb-bc/dosbox-nosymbols.patch` (apply it
to the `dosrun` branch of the fork and rebuild; a fork without it runs the
line as a shell command and PDS stays slow). With that, one shared queue of
(compiler, file) pairs over four machines per core: 28 s.

## The typed tree

`src/qb_ast.rs` is the recursive-descent parser building qbfront's `syntax::Module` directly,
from the logos lexer's tokens (with their text, and `type$`/`key%` kept as the names they spell).
Blocks (IF in both forms, FOR/NEXT, WHILE/WEND, DO/LOOP, SELECT CASE, SUB/FUNCTION/DEF FN, TYPE) are
recursion; `name(args)` stays an `Apply` for the semantic pass, as in the table-driven parser, so no
symbol table is needed between them; the existing `semantic::compile` is the pass that resolves it
and refuses what is not valid. Checked by `astcheck` (`-c` also compiles both modules) and
`tests/qb_ast.rs`:

* repo programs: 1,699 identical `Module`s, spans included; 24 identical but for the span of an
  omitted CIRCLE argument; 3 differ in one place (below); the new parser refuses none the old takes.
* through `semantic::compile` the two modules give the same output on 1,666 programs; the 3 that
  differ are the same one case.
* mgl and the private-tools sources, and the 487 programs harvested from test strings: the same,
  but for the places below.

Where the new tree differs from the table-driven parser's, on purpose (the old parser's behaviour is
wrong or loses something):

* precedence. Run through QB 4.5's own BC and LINK under DOSBox-X (`tools/dosbatch`), these print
  `2 ^ 3 ^ 2` = 64, `NOT 1 = 2` = -1, `NOT 0 + 1` = -2, `-2 ^ 2` = -4, `-3 * 2 ^ 2` = -12 and
  `7 MOD 3 * 2` = 1. The table-driven parser reads `^` right to left (512), `NOT` above the
  relations and sums (`NOT 1 = 2` is 0, `NOT 0 + 1` is 0), and a sign over a whole product
  (`-a * b` is `-(a * b)`). Its trees for the first two therefore compute 512 and 0; for the third the same
  value in a different shape. The new tree has QB's: `tests/qb_ast.rs::precedence_is_the_compilers`.
* a type suffix on a word that is also reserved is part of the name: `ERROR$`, `COLOR$(n)`, `key%`
  (the old lexer drops the suffix, so `COLOR$` was `COLOR`).
* `PUT (x, y), a` has no mode argument; the old parser takes one from the nearest earlier PUT/PSET/
  XOR token anywhere before it.
* `PRINT USING f$; a, b` keeps the separator the source gave.
* an empty slot in a PRINT list is an `Omitted` item rather than dropped.
* `CIRCLE (x, y), r, , c, s`: an empty slot keeps its place (the old drops it).

Not in this parser: QuickrBASIC's extensions (`+=`, `FOR … IN`, PRIVATE, tuples, f-strings), which
the table-driven parser reads in every dialect. Taken here that the table-driven parser refuses:
`ON [LOCAL] ERROR RESUME NEXT`, COMMON (as `Statement::Common`), `REDIM PRESERVE`, ISAM statements and
`OPEN … FOR ISAM`, `DEFCUR`/CURRENCY, `'$FORM` and VBDOS's form/control objects.

Speed, same 737 files: logos lexer + recursive descent to `Module`: 36.3 ms (21.6 MB/s);
the table-driven parser to the same `Module`: 232 ms (3.4 MB/s), 6.4x.

## The checker, against the real BC

`src/qb_check.rs` (with `qb_sigs.rs`, the intrinsic functions' signatures, and the generated
`qb_reserved.rs`) types a `Module` and refuses what BC refuses, per family (`Dialect::QuickBasic45`,
`Pds71`, `VbDos`). It is calibrated, not designed: every rule below is something the real BC.EXE does, found by
asking it. The loop:

* `corpus/statements.py` + `expand.py` generate a **statement catalog** (10,608 one-line cases: every
  intrinsic's shapes with right and wrong argument types, the trailing-comma and omitted-slot forms of
  LINE/CIRCLE/PAINT/PUT/PALETTE/…, and every reserved word of the three families used as a name in 22
  contexts). `corpus/programs.py` generates **4,838 small programs** (scopes, labels, blocks, procedures,
  types). `corpus/real.py` takes the **1,090 real programs**: the repo's `.bas` files, GORILLA,
  NIBBLES, QB 4.5's, PDS 7.1's and VBDOS's example sources, and mgl (run in place, so its
  `$INCLUDE`s resolve).
* `tools/oracle/qb-bc/{bcbatch,bcfiles}.py`, `corpus/real.py run` give each one to the three BCs (QB 4.5, PDS 7.1, VBDOS) on
  the dosrun DOSBox-X (`tools/oracle/qb-bc/bcprobe.py`, about 30 jobs a second), each alone. `tools/oracle/qb-bc/bcquick.py 'LINE'…` asks
  about a few lines; `gen_reserved.py` writes `qb_reserved.rs` from the verdicts.
* `compare.py`, `compare_programs.py` and `real.py compare` set our parse-and-check against BC's:
  **gap** (BC takes it, we refuse), **leak** (BC refuses, we take it).

| corpus | family | agree | gap | leak |
|---|---|---|---|---|
| 1,090 real programs | QB 4.5 / PDS 7.1 / VBDOS | 997 / 999 / 996 | **0 / 0 / 0** | 5 / 3 / 6 |
| 4,838 small programs | QB 4.5 / PDS 7.1 / VBDOS | 4,834 / 4,832 / 4,830 | 0 / 2 / 1 | 4 / 4 / 7 |
| the statement catalog (10,608) | QB 4.5 / PDS 7.1 / VBDOS | 10,591 / 10,594 / 10,590 | 0 / 1 / 5 | 14 / 11 / 12 |

(`.bi` files alone are not compared: BC does not check what an include declares until a program uses it.)
The leaks left are BC's resource limits (`Expression too complex` on 60 literal subscripts,
`Math overflow` in one QB 4.5 function), `0 PRINT` with `ON ERROR GOTO 0`, and a few reserved-word
corners; the real BC crashes on one program.

What BC does that no manual says, all of it now in the checker: no bare name is a statement unless a SUB
by that name was DECLAREd or defined above (`Equal sign missing`); a call above its DECLARE is not checked
against it; a DECLARE without a parameter list is never checked; `SUB p ()` is VBDOS's; `DIM a()` is
VBDOS's; `STATIC SUB`, `OPTION EXPLICIT`, `AS` after FUNCTION's parameters, array fields in a TYPE,
`REDIM PRESERVE`, ISAM, CURRENCY, `ERR = n` and reserved words as record fields are PDS's or VBDOS's or both;
arrays, scalars and CONSTs have separate namespaces, `x`, `x%` and `x AS INTEGER` are three names to BC
with rules of their own (and PDS/VBDOS refuse to mix a suffix with AS); a reserved word is no name for
anything but a field in PDS and VBDOS; a `_` ending a line continues it after any punctuation; COMMON and
DECLARE must precede the first executable statement; a label in a procedure may not repeat the module's;
`GOSUB` cannot leave a procedure but `RESTORE` and `ON ERROR GOTO` name labels anywhere or at module
level; an INTEGER argument that is a constant outside 16 bits is `Math overflow`; and an undefined line
number is an error where an undefined label name is one too. Pitfalls in the harness itself, each of
which made a verdict wrong until found: BC wants CRLF in `$INCLUDE` files; a trailing `' #id` comment
changes VBDOS's verdict on a line ending in a comma; bare messages arrive with a BEL in front
(`Label not defined: 10`); and a background job dies when the session's turn ends.

## Where the front lives now

The scanner, the typed parser (`ast`), the checker (`check`) and their tables (`kw`, `tokens`, `words`, `reserved`,
`sigs`, `classes`) are `qbfront::front`; this crate re-exports them as `qb_ast`, `qb_check` and so on and keeps the
recursive-descent comparators (`qb_rd`, `qb_tree`) and the oracle harness (`corpus/`).
`qbfront::parse` is `front::parse`: parse, then check, the first diagnostic being the error. QuickrBASIC is not in
the front yet (it has no BC to be held to, and its extensions are not ported), so that dialect still goes to the
grammar-table parser in `qbfront::generated_parser`; a source only that parser takes is refused with a message that
says the quickr profile takes it.

What the swap found: BC takes a subscripted array field of a record (`g.q(0)`), which neither parser did, and the repo's
own fixtures and tests held programs BC refuses (reserved words as names, a call above its DECLARE, `INSTR(60001, ..)`,
`1E+10!`), which were fixed to be programs BC takes.

## Errors a person can read

A stage that refuses a source knows two things: BC's name for the error ("Array not dimensioned") and where it is.
`qbfront::front::explain` turns that into words, and nothing else does:

* `messages.txt` has one line per BC error: `name | what is wrong | what to do`, with `{text}` (the source at the
  error), `{name}` (its leading identifier) and `{statement}` (the statement's first word) filled in. A new error needs a
  new line there and nothing else; `tests/diagnostics.rs` runs the 5,000-program corpus and fails if any error it raises
  has no line.
* A syntax error is worded from the statement it is in, the token found and, where the parser knew it, the token it
  wanted (`expected ')' in the PRINT statement, but found the end of the line`).
* The checker may name the word of a long span that is at fault (`err_about`), and the error then points at that word:
  the label in a GOTO.
* `front::parse` words the error and adds where the program is valid: "this source is valid for VBDOS (--dialect
  vbdos)". `front::parse_coded` leaves the BC name alone, for the tools that compare with BC.
* `qbfront::driver` prints `file:line:col: message`, the source line with the place marked, and the help; a compile
  error from the later passes (`line N: ...`) gets the file and line the person wrote, through the `$INCLUDE` map.

How a message gets its specifics, from the plainest to the most particular:

* `messages.txt` row for the error, and `messages.txt` variants: `name /WORD` is tried first when the error's text holds
  that word (`Identifier expected /TAB`, `Formal parameter specification illegal /BYVAL`), and `name /end` when it is
  at the end of the line.
* `syntax_forms.txt`: how a statement is written, shown as help for a syntax error in it.
* The parser names what it knows at the end of the file (`this FOR loop is never closed`) and for a closer with
  nothing, or the wrong block, open (`NEXT belongs to a FOR loop, but none is open here`).
* The checker passes a `Reason` (headline, help, the word at fault, a second place for `{other}`) where it knows more
  than the code says: both types of a mismatch, the argument of a bad call, the range an overflow missed.

`cargo run --release -p qbfront --example diagsurvey -- DIALECT FILE...` prints the distinct messages per BC error over a
set of sources; it is how the wording was reviewed over the corpus.
