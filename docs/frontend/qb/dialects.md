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

VBDOS `/O` has an observable control-context rule for `NOT`: the integer
operation is still materialized, then the branch successors are exchanged if
`NOT` occurs anywhere in the condition. Thus `NOT 5` prints `-6`, while `IF
(NOT 5) THEN` takes the false arm; the exchange also survives an enclosing
arithmetic expression. HIR keeps the ordinary bitwise `not` value and records
the exchanged successor order on `IF`, `WHILE`, and `DO`. This is required by
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
