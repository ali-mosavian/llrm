# QB runtime and semantic profiles

There is one source language: the highest VBDOS syntax superset. QB 4.5 is a
subset of PDS 7.1, which is a subset of VBDOS, so later syntax is accepted for
every target profile. The selected profile starts affecting the program after
parsing, in semantic rules, runtime interfaces, compiler options, layouts, and
ABI choices.

## Initial profiles

| Profile | Source syntax | Runtime pairing normally tested |
|---|---|---|
| `qbasic11` | VBDOS superset | QBasic interpreter behavior; compilation coverage may be a subset |
| `qb45` | VBDOS superset | QB 4.5 BCOM/BRUN runtime |
| `pds71` | VBDOS superset | PDS 7.1 runtime |
| `vbdos10` | VBDOS superset | VBDOS runtime |

The dialect is recorded once in the program profile. Feature checks query the
profile rather than being spread through action code as version comparisons.

## Profile-controlled behavior

A profile may affect:

- declaration, `DEFxxx`, and implicit-typing rules;
- static/dynamic array forms and limits;
- module, `COMMON`, event, and error-handling semantics;
- compiler directives and metacommand behavior;
- constant-expression rules and conversion diagnostics; and
- recovery and diagnostic points where those affect accepted input.

Original QuickBASIC 4.5 and PDS 7.1 reject underscores while VBDOS accepts
them, but that is compatibility evidence about the historical compilers, not a
restriction in this frontend. Compiler switches still differ, and VBDOS alone
provides the measured `/G3` mode. `/G3` informs ABI compatibility; the new
compiler always targets 386+ real mode.

Default typing is resolved semantically, not converted into explicit parser
types. `DEFINT`, `DEFLNG`, `DEFSNG`, `DEFDBL`, and `DEFSTR` accept comma-separated
letters and letter ranges. Direct runs of QB 4.5, PDS 7.1, and VBDOS establish
that a directive applies throughout its module regardless of source order;
suffixes and explicit `AS` clauses take precedence. A procedure starts with
the module defaults and then applies its own body-level directives to local
declarations and implicit locals. The isolated `default-types.bas` fresh-object
probe prints `2 2 2` followed by `4 4 8 0` under all three Microsoft runtimes.

VBDOS `/O` has two observable control-context rules for `NOT`. At the top level
it does not materialize the integer operation: `WHILE NOT (mask AND bit)` tests
the masked operand directly and exchanges its successors. When `NOT` is buried
under another operator, VBDOS materializes the bitwise value and still
exchanges the final branch; the exchange survives an enclosing arithmetic
expression. Thus value-producing `NOT 5` remains `-6`. HIR records the direct
operand test for the top-level case and the materialized value plus exchanged
edge order for the nested case. This is required by
the common BASIC mask spelling `WHILE NOT (flags AND bit)` and keeps the fact
out of MIR optimization and machine lowering.

## Evidence hierarchy

The QBasic grammar at
`~/work/ms/msdos_60/45/qb5/ir/qbasbnf.prs` is the parser baseline, not proof
of the later dialects. Additions are established using, in order:

1. a minimal source probe compiled by each relevant original compiler;
2. the compiler's diagnostic text and severe-error count;
3. linked program behavior under the matching runtime;
4. recovered Microsoft source, help, and manuals; and
5. representative real programs.

An emitted `.OBJ` is not proof that compilation succeeded because the BASIC
compilers emit objects after severe errors. Every acceptance fixture captures
and checks the diagnostic result.

Each dialect delta gets a compact record containing:

- source input;
- compiler/version and relevant flags;
- accept/reject result and diagnostic;
- AST/HIR expectation if accepted; and
- runtime output when semantics, rather than grammar, are in question.

The probe matrix runs on QB 4.5, PDS 7.1, and VBDOS whenever the construct
exists in more than one family. One compiler configuration is not used as a
proxy for all three.

## Grammar organization

The implementation keeps the recovered QBasic grammar as its generated base
and layers later PDS/VBDOS forms through declarative named extensions. Those
extensions are universal; no parser action carries a runtime-profile mask.

Semantic actions use named operations such as “declare procedure,” “form
array reference,” or “build conditional.” Numeric p-code opcodes are not part
of the grammar API.

Parser acceptance and lowering support have separate fixture fields. This
allows the parser to become dialect-complete without falsely claiming that a
new statement has a correct current-MIR lowering.

## Bring-up order

VBDOS is the first full-program profile because QB Quake/qb-qrender exercises
its extended source language and large-program behavior. That priority does
not make VBDOS syntax the common grammar. Small probes for all profiles begin
with the lexer/parser extraction so early architectural choices do not bake
in VBDOS-only behavior.

QB Quake is a late integration corpus, not the specification. A construct
first discovered there is reduced to a small dialect fixture before its
implementation is changed.

## QuickrBASIC (`quickr`)

`quickr` is the one profile that is not a Microsoft compiler. It is the VBDOS
profile plus the extensions below, and it pairs with the VBDOS runtime. The
Microsoft profiles reject every extension, so the compatibility ladder stays
faithful.

### Sized integers

| Spelling | Width | Range |
|---|---|---|
| `BYTE`, `SIGNED BYTE` | 1 | -128 to 127 |
| `UNSIGNED BYTE` | 1 | 0 to 255 |
| `INTEGER`, `SIGNED INTEGER` | 2 | -32768 to 32767 |
| `UNSIGNED INTEGER` | 2 | 0 to 65535 |
| `LONG`, `SIGNED LONG` | 4 | -2147483648 to 2147483647 |
| `UNSIGNED LONG` | 4 | 0 to 4294967295 |

`SIGNED` and `UNSIGNED` modify an integer type wherever `AS` takes one: `DIM`,
`REDIM`, `COMMON`, `STATIC`, `SHARED`, parameters, `FUNCTION` results and
`TYPE` fields. Every plain type is signed and `UNSIGNED` makes it unsigned.
`DEF{,U}{BYTE,INT,LNG}` set the default type of a letter range, and
`C{,U}{BYTE,INT,LNG}` convert to the type they name, as `DEFINT` and `CINT` do.
There are no new suffixes.

Arithmetic follows C. An operand narrower than `INTEGER` widens to `INTEGER`.
Otherwise the result has the wider width, and it is unsigned when either
operand of that width is unsigned. Comparison, `\`, `MOD` and conversion to
floating point use the unsigned form for unsigned operands. A store narrows
modulo the destination width. A constant outside the destination's range is
a compile-time overflow, except that a signed constant becomes the bit pattern
it spells in an unsigned type of its width: `&HFFFF` fills an
`UNSIGNED INTEGER`. A `FOR` loop over an unsigned counter keeps a signed
`STEP`.

`PRINT`, `INPUT`, `READ`, `STR$` and the numeric intrinsics see a sized integer
as the Microsoft type that holds its values: `INTEGER` for the bytes, `LONG`
for `UNSIGNED INTEGER` and `DOUBLE` for `UNSIGNED LONG`.

### Declarations

`OPTION EXPLICIT` is always on: using a variable that no `DIM`, `REDIM`,
`COMMON`, `STATIC`, `SHARED`, `CONST` or parameter declares is the error
"Variable not defined". Reading a local variable on a path where nothing has
assigned it is a warning. The program still compiles, and the variable reads
as zero.

### Procedure frames

A procedure frames itself: `push bp`, `mov bp,sp`, `sub sp`, and an inline
`rep stosw` that zero-fills its locals, in place of `B$ENRA` and `B$EXSA`. It
keeps the runtime's frame when the runtime needs one: when it has an error
handler or RESUME target, which the runtime reaches through its frame chain,
or a local STRING, for which VBDOS's `B$ENRA` reserves a string handle. An
inline frame has no runtime stack check, and `B$EXSA` no longer polls events
when such a procedure returns.

### F-strings

`f"…"` (or `F"…"`) is a string expression. `{expression}` inserts a value,
`{expression:spec}` formats it with Python's format-spec mini-language, and
`{{` and `}}` are literal braces. A field cannot hold a string literal, since
BASIC has no escape for the quote that would end the f-string. Under the
Microsoft profiles, `f"x"` is still the name `f` followed by a string.

A value prints as Python's `str()` would print it: a float as the shortest
text that reads back as the same SINGLE or DOUBLE. The compiler checks each
spec against its value's type and reports Python's errors. The formatting
itself is BASIC, in `frontends/qb/src/semantic/prelude.bas`, which the
compiler adds to programs that use f-strings. Its procedures reserve names
beginning `QUICKR_`. Numbers are formatted from their exact decimal
expansion, so `f`, `e`, `g` and `%` match Python digit for digit.
