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

### Arrays

Every array dimension starts at 0. A bound is only the upper one: `lower TO
upper` and `OPTION BASE 1` are errors, and `LBOUND` is the constant 0.

### Procedure frames

A procedure frames itself with `push bp`, `mov bp,sp` and `sub sp`, in place
of `B$ENRA` and `B$EXSA`. Its HIR stores zero at entry to each local some path
reads before assigning: every aggregate, and each number the use-before-def
analysis cannot prove written first. The frame is otherwise not cleared. The
backend lays those locals out as one block just below BP, and a block of 16
bytes or more is cleared with one `rep stosd` rather than a store per word. It
keeps the runtime's frame when the runtime needs one: when it has an error
handler or RESUME target, which the runtime reaches through its frame chain,
or a local STRING, for which VBDOS's `B$ENRA` reserves a string handle. An
inline frame has no runtime stack check, and `B$EXSA` no longer polls events
when such a procedure returns.

### Private procedures

`PRIVATE SUB` and `PRIVATE FUNCTION` define a procedure only its own module can
call: it has no public symbol, so a call from another module fails at LINK. One
that frames itself is near: its callers use `call` and it returns with `ret`,
its parameters starting at `[bp+4]`. One on the runtime's frame stays far. The
f-string prelude is private, so modules that each use f-strings link together.

### F-strings

`f"…"` (or `F"…"`) is a string expression. `{expression}` inserts a value,
`{expression:spec}` formats it with Python's format-spec mini-language, and
`{{` and `}}` are literal braces. A field cannot hold a string literal, since
BASIC has no escape for the quote that would end the f-string. Under the
Microsoft profiles, `f"x"` is still the name `f` followed by a string.

A value prints as Python's `str()` would print it: a float as the shortest
text that reads back as the same SINGLE or DOUBLE. The compiler checks each
spec against its value's type and reports Python's errors. The formatting
itself is BASIC, in `crates/qbfront/src/semantic/prelude.bas`, which the
compiler adds to programs that use f-strings. Its procedures reserve names
beginning `QUICKR_`. Numbers are formatted from their exact decimal
expansion, so `f`, `e`, `g` and `%` match Python digit for digit.

### Augmented assignment

`x op= value` means `x = x op (value)` for `+ - * / \ ^ MOD AND OR XOR`.
The target's subscripts are evaluated once, so `a(f()) += 1` calls `f` once.

### BREAK and CONTINUE

`BREAK` leaves the innermost `FOR`, `WHILE` or `DO` loop. `CONTINUE` starts its
next iteration: a `FOR` steps its counter first, and a `DO … LOOP WHILE` runs
its test. Both names are reserved.

### FOR … IN

`FOR x IN iterable … NEXT` runs its body once per element, with `x`
holding a copy: assigning `x` changes neither the iterable nor the iteration.
`AS type` declares `x`, and several loops may declare it with the same type.
Without it, an undeclared `x` takes the type of the elements: an array's
element type, STRING for a string, and INTEGER for `RANGE`, or LONG when a
bound is LONG, unsigned or floating.

| Iterable | Elements |
|---|---|
| `a()` or `a` | each element of one-dimensional array `a`, live |
| `RANGE(stop)`, `RANGE(start, stop[, step])` | Python's `range`; `x` must be an integer |
| any string expression | each character of a copy taken before the loop |
| `f(…)`, a FUNCTION `AS t()` | each element of the array it returns |

`RANGE` evaluates its arguments once, in order.

### Conditional, IN and chained comparisons

`a IF c ELSE b` evaluates `c`, then only the arm it picks. It binds loosest
and nests to the right: `x IF p ELSE y IF q ELSE z`. Both arms are strings or
both are numbers, which take the wider type.

`x IN (v1, v2, …)` compares `x` with each value until one is equal. `x IN s`
tests whether string `s` contains `x`, and `x IN a()` (or `x IN a`) whether an
element of array `a` equals `x`. `NOT IN` is the negation. Each is `-1` or `0`.

`a < b <= c` means `a < b AND b <= c`, with `b` evaluated once and `c` not at
all when `a < b` is false. Any run of `=`, `<>`, `<`, `<=`, `>`, `>=` chains
this way. A parenthesized comparison is a value again, so `(a < b) < c` keeps
QB's meaning.

### RETURN value

In a FUNCTION, `RETURN value` sets the result and leaves, as
`name = value: EXIT FUNCTION` does. A bare `RETURN` still ends a GOSUB, and
`RETURN label` is not available in a FUNCTION.

### Tuples

`a, b = x, y` evaluates every value, then assigns the targets left to right,
so `a, b = b, a` swaps. A FUNCTION `AS (t1, t2, …)` returns several values:
`RETURN x, y` inside it, and `q, r = f(…)` at the call. It compiles as a SUB
with a hidden BYREF parameter per result; the caller passes temporaries and
assigns them to the targets. A tuple is only a FUNCTION's result type.

### Record results

A FUNCTION `AS record` returns a record. It compiles as a SUB with a hidden
BYREF parameter the caller points at a zeroed temporary, so fields the
FUNCTION does not set are zero. Assigning the FUNCTION's name, or a field of
it, sets the result, and a call can stand anywhere a record can:
`p = make(1, 2)`, `make(1, 2).x`, or an argument.

### Array values

Arrays are values. `a() = b()` redimensions dynamic array `a` to `b`'s
bounds and copies each element. A FUNCTION `AS t()` returns an array: it
compiles as a SUB with a hidden BYREF array parameter. `a() = f(…)` erases
`a` and passes it as that parameter, so the result is built in place with no
copy; when the arguments name `a`, the result goes to a temporary first.
`RETURN r()` copies `r` into the result. `FOR x IN f(…)` iterates a
returned array. Only one-dimensional arrays are copied, and the target of an
array assignment must be dynamic: `DIM a() AS LONG`.

### String slices

`s(start:end:step)` is Python's slice of string `s`, counting from 0: `s(1:3)`,
`s(:2)`, `s(-3:)`, `s(::-1)`. Each part is optional, a negative index counts
from the end, and bounds outside the string are clamped. A slice can follow a
call, `f$(x)(1:)`, and another slice. Assigning to a slice splices:
`s(1:3) = "xyz"` replaces those characters whatever the new length, and an
extended slice such as `s(::2)` takes exactly as many characters as it has.
The slices run in the QuickrBASIC prelude.
