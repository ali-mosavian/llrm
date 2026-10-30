"""Whole small programs, generated from templates: block structure, procedures, types, scoping, labels.

Each program is judged by BC as a whole (accepted or refused), so none needs a prologue.
"""

import itertools
import json
from pathlib import Path

HERE = Path(__file__).parent
PROGRAMS = []


def add(group, why, *lines):
    PROGRAMS.append({"group": group, "why": why, "text": "\n".join(lines) + "\n"})


# ---- block structure -----------------------------------------------------------------------------------
BLOCKS = {
    "if": (["IF x% THEN"], ["END IF"]),
    "ifelse": (["IF x% THEN", "y% = 1", "ELSE"], ["END IF"]),
    "ifelseif": (["IF x% THEN", "ELSEIF y% THEN"], ["END IF"]),
    "for": (["FOR i% = 1 TO 2"], ["NEXT"]),
    "forvar": (["FOR i% = 1 TO 2"], ["NEXT i%"]),
    "while": (["WHILE x%"], ["WEND"]),
    "do": (["DO"], ["LOOP"]),
    "dowhile": (["DO WHILE x%"], ["LOOP"]),
    "dountil": (["DO"], ["LOOP UNTIL x%"]),
    "select": (["SELECT CASE x%", "CASE 1"], ["END SELECT"]),
    "selectelse": (["SELECT CASE x%", "CASE 1", "y% = 1", "CASE ELSE"], ["END SELECT"]),
}
CLOSERS = {k: v[1][0] for k, v in BLOCKS.items()}
ALL_CLOSERS = ["END IF", "NEXT", "NEXT i%", "NEXT j%", "WEND", "LOOP", "LOOP UNTIL x%", "END SELECT", "END SUB", "END FUNCTION", "ELSE", "ELSEIF x% THEN", "CASE 1", "CASE ELSE"]
PRO = ["DIM x%, y%, i%, j%"]
for a in BLOCKS:
    open_a, close_a = BLOCKS[a]
    add("blocks", f"{a} alone", *PRO, *open_a, "y% = 2", *close_a)
    add("blocks", f"{a} unclosed", *PRO, *open_a, "y% = 2")
    add("blocks", f"{a} closed twice", *PRO, *open_a, "y% = 2", *close_a, *close_a)
    for closer in ALL_CLOSERS:
        if closer != close_a[0]:
            add("blocks", f"{a} closed by {closer}", *PRO, *open_a, "y% = 2", closer)
    for b in BLOCKS:
        open_b, close_b = BLOCKS[b]
        add("blocks", f"{a} in {b}", *PRO, *open_a, *open_b, "y% = 3", *close_b, *close_a)
        add("blocks", f"{a} in {b} crossed", *PRO, *open_a, *open_b, "y% = 3", *close_a, *close_b)
    add("blocks", f"{a} then orphan closer", *PRO, *open_a, "y% = 2", *close_a, "END IF")
for word in ["ELSE", "ELSEIF x% THEN", "CASE 1", "CASE ELSE", "END IF", "NEXT", "WEND", "LOOP", "END SELECT", "END SUB", "END FUNCTION", "END DEF", "EXIT SUB", "EXIT FUNCTION", "EXIT DO", "EXIT FOR", "RETURN", "RESUME", "END"]:
    add("blocks", f"{word} alone", *PRO, word)
# a SELECT with statements before the first CASE
add("blocks", "statement before CASE", *PRO, "SELECT CASE x%", "y% = 1", "CASE 1", "END SELECT")
add("blocks", "select with no case", *PRO, "SELECT CASE x%", "END SELECT")
add("blocks", "case after else", *PRO, "SELECT CASE x%", "CASE ELSE", "CASE 1", "END SELECT")
add("blocks", "two elses", *PRO, "IF x% THEN", "ELSE", "ELSE", "END IF")
add("blocks", "elseif after else", *PRO, "IF x% THEN", "ELSE", "ELSEIF y% THEN", "END IF")
add("blocks", "one-line if block mix", *PRO, "IF x% THEN y% = 1", "END IF")
add("blocks", "one-line if else", *PRO, "IF x% THEN y% = 1 ELSE y% = 2")
add("blocks", "one-line nested", *PRO, "IF x% THEN IF y% THEN i% = 1 ELSE i% = 2")
add("blocks", "one-line with colon", *PRO, "IF x% THEN y% = 1: i% = 2")
add("blocks", "one-line else colon", *PRO, "IF x% THEN y% = 1 ELSE y% = 2: i% = 3")
add("blocks", "if then nothing", *PRO, "IF x% THEN")
add("blocks", "if then colon", *PRO, "IF x% THEN: y% = 1")
add("blocks", "if then else only", *PRO, "IF x% THEN ELSE y% = 1")
add("blocks", "for step", *PRO, "FOR i% = 10 TO 1 STEP -1", "NEXT")
add("blocks", "for two vars next", *PRO, "FOR i% = 1 TO 2", "FOR j% = 1 TO 2", "NEXT j%, i%")
add("blocks", "for two vars next swapped", *PRO, "FOR i% = 1 TO 2", "FOR j% = 1 TO 2", "NEXT i%, j%")
add("blocks", "next list too long", *PRO, "FOR i% = 1 TO 2", "NEXT i%, j%")
add("blocks", "for same var nested", *PRO, "FOR i% = 1 TO 2", "FOR i% = 1 TO 2", "NEXT", "NEXT")
add("blocks", "exit for outside", *PRO, "EXIT FOR")
add("blocks", "exit for inside", *PRO, "FOR i% = 1 TO 2", "EXIT FOR", "NEXT")
add("blocks", "exit do inside for", *PRO, "FOR i% = 1 TO 2", "EXIT DO", "NEXT")
add("blocks", "exit do inside do", *PRO, "DO", "EXIT DO", "LOOP")
add("blocks", "exit while", *PRO, "WHILE x%", "EXIT WHILE", "WEND")
add("blocks", "exit for in sub", *PRO, "SUB t", "FOR i% = 1 TO 2", "EXIT FOR", "NEXT", "END SUB")
add("blocks", "do while until", *PRO, "DO WHILE x%", "LOOP UNTIL y%")
add("blocks", "do while while", *PRO, "DO WHILE x%", "LOOP WHILE y%")

# ---- procedures -------------------------------------------------------------------------------------------
ARGS = {"i": ("x%", "INTEGER"), "l": ("l&", "LONG"), "s": ("s!", "SINGLE"), "d": ("d#", "DOUBLE"), "t": ("s$", "STRING"), "c": ("5", "const"), "e": ("(x% + 1)", "expr"), "r": ("rec", "RecT"), "a": ("arr()", "array")}
for decl, define in itertools.product(["none", "match", "extra arg", "fewer arg", "type", "kind"], ["sub", "function"]):
    head = ["TYPE RecT", "a AS INTEGER", "END TYPE", "DIM x%, l&, s!, d#, s$, rec AS RecT, arr%(10)"]
    if define == "sub":
        body = ["SUB p (a%, b$)", "END SUB"]
        call = "CALL p(x%, s$)"
        d = {"none": [], "match": ["DECLARE SUB p (a%, b$)"], "extra arg": ["DECLARE SUB p (a%, b$, c%)"], "fewer arg": ["DECLARE SUB p (a%)"], "type": ["DECLARE SUB p (a&, b$)"], "kind": ["DECLARE FUNCTION p% (a%, b$)"]}[decl]
    else:
        body = ["FUNCTION f% (a%, b$)", "f% = a%", "END FUNCTION"]
        call = "x% = f%(x%, s$)"
        d = {"none": [], "match": ["DECLARE FUNCTION f% (a%, b$)"], "extra arg": ["DECLARE FUNCTION f% (a%, b$, c%)"], "fewer arg": ["DECLARE FUNCTION f% (a%)"], "type": ["DECLARE FUNCTION f& (a%, b$)"], "kind": ["DECLARE SUB f (a%, b$)"]}[decl]
    add("procs", f"declare {decl}, define {define}", *head, *d, call, "END", *body)
for call_style in ["CALL p(x%, s$)", "p x%, s$", "p (x%), s$", "p(x%, s$)", "CALL p", "p", "CALL p(x%)", "p x%", "CALL p(1, \"a\")", "p 1, \"a\"", "CALL p(s$, x%)", "x% = p(x%, s$)", "PRINT p(x%, s$)"]:
    for declared in (True, False):
        add("procs", f"call style {call_style!r} declared={declared}", "DIM x%, s$", *(["DECLARE SUB p (a%, b$)"] if declared else []), call_style, "END", "SUB p (a%, b$)", "END SUB")
for call_style in ["x% = f%(x%, s$)", "x% = f%(1, \"a\")", "x% = f%", "x% = f%()", "CALL f%(x%, s$)", "f% x%, s$", "x% = f(x%, s$)", "s$ = f%(x%, s$)", "x% = f%(s$, x%)", "x% = f%(x%)", "PRINT f%(x%, s$)"]:
    for declared in (True, False):
        add("procs", f"function call {call_style!r} declared={declared}", "DIM x%, s$", *(["DECLARE FUNCTION f% (a%, b$)"] if declared else []), call_style, "END", "FUNCTION f% (a%, b$)", "f% = 1", "END FUNCTION")
for header in ["SUB p", "SUB p ()", "SUB p (a)", "SUB p (a%, b%)", "SUB p (a%, a%)", "SUB p (a AS INTEGER)", "SUB p (a() AS INTEGER)", "SUB p (a%())", "SUB p (BYVAL a%)", "SUB p (a AS STRING)", "SUB p (a AS STRING * 5)", "SUB p (a AS ANY)", "SUB p (a AS Nope)", "SUB p% ()", "SUB p$ ()", "SUB p STATIC", "SUB p (x%) STATIC", "SUB P (a%)", "SUB p.q (a%)", "SUB 1p ()", "SUB p (a%", "SUB p a%", "SUB", "SUB p, q", "SUB p (a%,)", "SUB p (,a%)", "SUB print ()", "SUB x ()", "SUB p (a#, b!, c&, d$)", "SUB p (a AS INTEGER, b AS LONG)", "SUB p (a%, b AS STRING)"]:
    add("procs", f"header {header!r}", "TYPE RecT", "a AS INTEGER", "END TYPE", "DIM x%", "CALL p", header, "END SUB")
for header in ["FUNCTION f", "FUNCTION f ()", "FUNCTION f%", "FUNCTION f$ ()", "FUNCTION f! ()", "FUNCTION f# ()", "FUNCTION f& ()", "FUNCTION f () AS INTEGER", "FUNCTION f% () AS INTEGER", "FUNCTION f$ () AS INTEGER", "FUNCTION f () AS STRING", "FUNCTION f () AS RecT", "FUNCTION f (a%) AS LONG", "FUNCTION f () STATIC", "FUNCTION f (BYVAL a%) AS INTEGER", "FUNCTION p ()", "FUNCTION fn ()", "FUNCTION fna ()"]:
    add("procs", f"function header {header!r}", "TYPE RecT", "a AS INTEGER", "END TYPE", header, "END FUNCTION")
for body in [["f% = 1"], ["f = 1"], ["f$ = \"a\""], ["x% = f%"], ["f% = f% + 1"], ["EXIT FUNCTION"], ["EXIT SUB"], ["RETURN"], ["DIM y%", "y% = 1"], ["SHARED x%"], ["STATIC y%"], ["DIM SHARED y%"], ["CONST k = 1"], ["TYPE T2", "a AS INTEGER", "END TYPE"], ["DECLARE SUB q"], ["COMMON x%"], ["OPTION BASE 1"], ["DEF FNa% = 1"], ["DEFINT A-Z"], ["GOSUB l1", "EXIT FUNCTION", "l1:", "RETURN"], ["ON ERROR GOTO h", "h:", "RESUME NEXT"], ["f% = 1", "END"], ["SUB q", "END SUB"], ["DIM x%(5)", "REDIM x%(10)"], ["DATA 1", "READ x%"], ["x% = 1"]]:
    add("procs", f"function body {body!r}", "DIM x%", "FUNCTION f%", *body, "f% = 1", "END FUNCTION")
    add("procs", f"sub body {body!r}", "DIM x%", "SUB q", *body, "END SUB")
for first, second in [("SUB p", "SUB p"), ("SUB p", "FUNCTION p"), ("FUNCTION p%", "FUNCTION p%"), ("FUNCTION p%", "FUNCTION p&"), ("FUNCTION p%", "SUB p"), ("SUB p", "SUB P"), ("SUB p", "SUB p2")]:
    end1 = "END SUB" if first.startswith("SUB") else "END FUNCTION"
    end2 = "END SUB" if second.startswith("SUB") else "END FUNCTION"
    add("procs", f"two definitions {first} / {second}", first, end1, second, end2)
add("procs", "nested sub", "SUB p", "SUB q", "END SUB", "END SUB")
add("procs", "end sub with function", "SUB p", "END FUNCTION")
add("procs", "end function with sub", "FUNCTION f%", "END SUB")
add("procs", "statements between subs", "SUB p", "END SUB", "PRINT 1", "SUB q", "END SUB")
add("procs", "statements after sub", "SUB p", "END SUB", "PRINT 1")
add("procs", "recursion", "DECLARE FUNCTION f% (n%)", "PRINT f%(3)", "END", "FUNCTION f% (n%)", "IF n% > 0 THEN f% = n% + f%(n% - 1)", "END FUNCTION")
add("procs", "sub named like variable", "DIM p%", "CALL p", "SUB p", "END SUB")
add("procs", "variable named like sub", "SUB p", "END SUB", "p = 1")
add("procs", "param shadows", "DIM x%", "CALL p(1)", "END", "SUB p (x%)", "x% = 2", "END SUB")
add("procs", "param shared", "DIM SHARED x%", "CALL p(1)", "END", "SUB p (x%)", "SHARED x%", "END SUB")
add("procs", "shared undefined", "CALL p", "END", "SUB p", "SHARED zz%", "zz% = 1", "END SUB")
add("procs", "shared array", "DIM zz%(5)", "CALL p", "END", "SUB p", "SHARED zz%()", "zz%(1) = 1", "END SUB")
add("procs", "shared array no parens", "DIM zz%(5)", "CALL p", "END", "SUB p", "SHARED zz%", "END SUB")
add("procs", "dim shared sees", "DIM SHARED zz%", "CALL p", "END", "SUB p", "zz% = 1", "END SUB")
add("procs", "dim not shared", "DIM zz%", "CALL p", "END", "SUB p", "zz% = 1", "END SUB")
add("procs", "static var", "CALL p", "END", "SUB p", "STATIC n%", "n% = n% + 1", "END SUB")
add("procs", "function result string", "DECLARE FUNCTION f$ ()", "PRINT f$", "END", "FUNCTION f$", "f$ = \"a\"", "END FUNCTION")
add("procs", "function result wrong", "DECLARE FUNCTION f$ ()", "END", "FUNCTION f$", "f$ = 1", "END FUNCTION")
add("procs", "function no assign", "DECLARE FUNCTION f% ()", "END", "FUNCTION f%", "END FUNCTION")
add("procs", "def fn", "DEF FNa% (z%) = z% * 2", "PRINT FNa%(2)")
add("procs", "def fn used before", "PRINT FNa%(2)", "DEF FNa% (z%) = z% * 2")
add("procs", "def fn type", "DEF FNa$ (z$) = z$", "PRINT FNa$(\"x\")")
add("procs", "def fn wrong call", "DEF FNa% (z%) = z% * 2", "PRINT FNa%(2, 3)")
add("procs", "def fn multi", "DEF FNa% (z%)", "FNa% = z% * 2", "END DEF", "PRINT FNa%(2)")
add("procs", "def fn no params", "DEF FNa% = 7", "PRINT FNa%")
add("procs", "def fn name no suffix", "DEF FNa (z) = z * 2", "PRINT FNa(2)")
add("procs", "def fn in sub", "SUB p", "DEF FNa% (z%) = z% * 2", "END SUB")
add("procs", "def fn twice", "DEF FNa% (z%) = z%", "DEF FNa% (z%) = z%")
add("procs", "def fn no prefix", "DEF a% (z%) = z%")
add("procs", "def fn space", "DEF FN a% (z%) = z%", "PRINT FN a%(1)")
add("procs", "declare only", "DECLARE SUB p (a%)", "CALL p(1)")
add("procs", "declare twice", "DECLARE SUB p (a%)", "DECLARE SUB p (a%)", "CALL p(1)", "SUB p (a%)", "END SUB")
add("procs", "declare conflict", "DECLARE SUB p (a%)", "DECLARE SUB p (a$)", "CALL p(1)")
add("procs", "declare after use", "CALL p(1)", "DECLARE SUB p (a%)", "SUB p (a%)", "END SUB")
add("procs", "declare in sub", "SUB q", "DECLARE SUB p", "END SUB")
add("procs", "declare cdecl", "DECLARE SUB p CDECL (a%)", "CALL p(1)")
add("procs", "declare alias", "DECLARE SUB p ALIAS \"q\" (a%)", "CALL p(1)")
add("procs", "declare cdecl alias", "DECLARE SUB p CDECL ALIAS \"q\" (a%)", "CALL p(1)")
add("procs", "declare byval", "DECLARE SUB p (BYVAL a%)", "CALL p(1)")
add("procs", "declare byval arg", "DECLARE SUB p (BYVAL a%)", "DIM x%", "CALL p(BYVAL x%)")
add("procs", "declare seg", "DECLARE SUB p (SEG a AS INTEGER)", "DIM x%", "CALL p(x%)")
add("procs", "declare any", "DECLARE SUB p (a AS ANY)", "DIM x%, s$", "CALL p(x%)", "CALL p(s$)")
add("procs", "call absolute", "DIM x%", "CALL ABSOLUTE(x%, 0)")
add("procs", "call interrupt", "DIM x%", "CALL INTERRUPT(&H10, x%, x%)")

# ---- types -----------------------------------------------------------------------------------------------------
for fields in [["a AS INTEGER"], ["a AS INTEGER", "b AS LONG", "c AS SINGLE", "d AS DOUBLE", "s AS STRING * 8"], ["s AS STRING"], ["a%"], ["a"], ["a AS INTEGER", "a AS LONG"], ["a(5) AS INTEGER"], ["a(1 TO 5) AS INTEGER"], ["a() AS INTEGER"], ["r AS Inner"], ["a AS Nope"], ["a AS RecT"], [], ["a AS INTEGER * 2"], ["a AS STRING * 0"], ["a AS STRING * x"], ["a AS STRING * 40000"], ["print AS INTEGER"], ["a AS ANY"], ["a.b AS INTEGER"], ["a AS INTEGER, b AS INTEGER"]]:
    add("types", f"fields {fields!r}", "TYPE Inner", "q AS INTEGER", "END TYPE", "TYPE RecT", *fields, "END TYPE", "DIM r AS RecT")
for use in ["r.a = 1", "r.a = \"x\"", "r.s = \"x\"", "r.s = 1", "r.nope = 1", "r = r2", "r = 1", "x% = r", "PRINT r", "PRINT r.a", "PRINT r2.a", "r.i.q = 1", "r.i = 1", "x% = r.i.q", "x% = LEN(r)", "x% = LEN(r.a)", "s$ = r.s + \"a\"", "r.a = r.a + 1", "r.a(1) = 1", "rr(1).a = 1", "rr(1).s = \"x\"", "rr = r", "rr(1) = r", "r = rr(1)", "x% = rr(1).a", "x% = rr.a", "CALL p(r)", "CALL p(r.a)", "CALL p(rr(1))", "CALL q(rr())", "IF r.a THEN x% = 1", "IF r THEN x% = 1", "FOR r.a = 1 TO 2: NEXT", "SWAP r.a, x%", "SWAP r, r2", "GET #1, , r", "PUT #1, , r", "INPUT r.a", "LINE INPUT r.s", "READ r.a", "LSET r.s = \"a\"", "r.s = STRING$(3, \"a\")", "DIM r3 AS RecT", "DIM r AS RecT", "DIM r AS INTEGER", "DIM q AS Nope", "DIM r4(5) AS RecT", "REDIM r4(5) AS RecT", "x% = r2.a", "x% = LEN(RecT)", "s$ = MKI$(r.a)"]:
    add("types", f"use {use!r}", "TYPE Inner", "q AS INTEGER", "END TYPE", "TYPE RecT", "a AS INTEGER", "s AS STRING * 8", "i AS Inner", "END TYPE", "DIM r AS RecT, r2 AS RecT, rr(3) AS RecT", "DIM x%, s$", "DECLARE SUB p (z AS RecT)", "DECLARE SUB q (z() AS RecT)", use, "END", "SUB p (z AS RecT)", "END SUB", "SUB q (z() AS RecT)", "END SUB")
add("types", "type inside sub", "SUB p", "TYPE T2", "a AS INTEGER", "END TYPE", "END SUB")
add("types", "type after dim", "DIM r AS RecT", "TYPE RecT", "a AS INTEGER", "END TYPE")
add("types", "type twice", "TYPE T2", "a AS INTEGER", "END TYPE", "TYPE T2", "b AS INTEGER", "END TYPE")
add("types", "type same name as var", "DIM T2", "TYPE T2", "a AS INTEGER", "END TYPE")
add("types", "type unclosed", "TYPE T2", "a AS INTEGER")
add("types", "type recursive", "TYPE T2", "a AS T2", "END TYPE")
add("types", "type statement inside", "TYPE T2", "a AS INTEGER", "x% = 1", "END TYPE")
add("types", "type suffix name", "TYPE T2%", "a AS INTEGER", "END TYPE")

# ---- declarations pairs (what conflicts with what) -----------------------------------------------------------
POOL = [
    "x% = 1", "x = 1", "x$ = \"a\"", "x# = 1", "x& = 1", "x! = 1", "x%(1) = 1", "x(1) = 1", "x$(1) = \"a\"",
    "DIM x%", "DIM x", "DIM x AS INTEGER", "DIM x AS LONG", "DIM x AS STRING", "DIM x$", "DIM x AS STRING * 4",
    "DIM x%(5)", "DIM x(5)", "DIM x(5) AS INTEGER", "DIM x(5) AS LONG", "DIM x$(5)", "DIM x(1 TO 5)", "DIM x(5, 5)",
    "REDIM x%(5)", "REDIM x(5) AS LONG", "DIM SHARED x%", "DIM SHARED x%(5)", "STATIC x%", "COMMON x%", "COMMON SHARED x%",
    "CONST x = 1", "CONST x% = 1", "CONST x$ = \"a\"", "CONST x = y", "CONST y = 2",
    "PRINT x", "PRINT x%", "PRINT x$", "PRINT x(1)", "PRINT x%(1)",
    "DEFINT X", "DEFSTR X", "DEFDBL A-Z", "OPTION BASE 1", "OPTION BASE 0", "OPTION BASE 2",
    "ERASE x", "ERASE x%", "x% = x(1)", "FOR x% = 1 TO 2: NEXT", "SWAP x%, x", "INPUT x%",
]
for first, second in itertools.product(POOL, POOL):
    add("scope", f"{first} / {second}", first, second)
for line in POOL:
    add("scope", f"in sub {line}", "SUB p", line, "END SUB")
    add("scope", f"sub shared {line}", "DIM SHARED x%, x%(5)", "SUB p", line, "END SUB")

# ---- labels and jumps ----------------------------------------------------------------------------------------------
JUMPS = ["GOTO a", "GOSUB a", "ON x% GOTO a", "ON x% GOSUB a", "ON ERROR GOTO a", "RESUME a", "RESTORE a", "IF x% THEN a", "IF x% GOTO a", "IF x% THEN GOTO a", "IF x% THEN 10", "GOTO 10", "GOSUB 10", "ON ERROR GOTO 10", "RESUME 10", "RESTORE 10", "RUN a", "RUN 10", "GOTO 0", "ON ERROR GOTO 0", "RESUME 0", "GOTO 65535", "GOTO 65536", "GOTO -1", "GOTO a%", "GOTO a$", "GOTO x%", "GOTO"]
LABELS = ["a:", "a: PRINT 1", "10", "10 PRINT 1", "A:", "a%:", "a$:", "x:", "print:", "1a:", "a b:", "a.b:", "a:a:", "a::", ":a", "65535 PRINT", "65536 PRINT", "0 PRINT", "a: b:", "END: ", "a_: ", "REM: "]
for label, jump in itertools.product(LABELS, JUMPS):
    add("labels", f"{label!r} then {jump!r}", "DIM x%", jump, label)
    add("labels", f"{jump!r} then {label!r}", "DIM x%", label, jump)
for jump in JUMPS[:8]:
    add("labels", f"label in sub, jump from module {jump!r}", "DIM x%", jump, "END", "SUB p", "a:", "END SUB")
    add("labels", f"label in module, jump from sub {jump!r}", "DIM x%", "a:", "10 PRINT", "END", "SUB p", jump, "END SUB")
    add("labels", f"label twice {jump!r}", "DIM x%", "a:", "a:", jump)
    add("labels", f"same label in sub and module {jump!r}", "DIM x%", "a:", "SUB p", "a:", jump, "END SUB")

# ---- records with array fields, nested records, and records in arrays (appended: ids above stay) --------
RECORDS = [
    "TYPE Inner", "v(3) AS INTEGER", "w AS INTEGER", "END TYPE",
    "TYPE Rec", "a(5) AS INTEGER", "b(1 TO 3, 1 TO 4) AS LONG", "s(2) AS STRING * 4", "d(2) AS DOUBLE", "n AS INTEGER", "t AS STRING * 6", "END TYPE",
    "TYPE Outer", "i(2) AS Inner", "q AS Inner", "END TYPE",
    "DIM r AS Rec, rr(3) AS Rec, o AS Outer, oo(2) AS Outer", "DIM x%, i%, j%, s$, d#",
    "DECLARE SUB p (z AS INTEGER)", "DECLARE SUB q (z() AS INTEGER)", "DECLARE SUB m (z AS Rec)", "DECLARE SUB n2 (z AS Inner)",
]
RECORD_USES = [
    "r.a(1) = 1", "x% = r.a(1)", "x% = r.a(i%)", "x% = r.a(i% + j%)", "r.a(1, 2) = 1", "r.b(1, 2) = 1", "r.b(1) = 1", "x% = r.b(1, 2)", "x% = r.b(1)",
    "r.s(1) = \"x\"", "s$ = r.s(1)", "s$ = r.s(1) + r.t", "PRINT r.a(1)", "PRINT r.a(1); r.b(1, 1); r.s(1); r.d(1)", "r.a = 1", "x% = r.a", "PRINT r.a",
    "x% = r.n(1)", "r.n(1) = 1", "rr(1).a(2) = 3", "x% = rr(i%).a(j%)", "rr(1).s(1) = \"abc\"", "s$ = rr(2).s(0)", "o.i(1).v(2) = 3", "x% = o.q.v(1)", "x% = oo(1).i(2).v(3)",
    "oo(1).q.w = 1", "oo(1).i(2) = o.q", "o.i(1) = o.q", "o.q = o.i(2)", "CALL p(r.a(1))", "CALL p(r.a())", "CALL q(r.a())", "CALL q(r.a)", "CALL p(r.n)", "CALL p(rr(1).a(1))",
    "CALL m(r)", "CALL m(rr(1))", "CALL n2(o.q)", "CALL n2(o.i(1))", "CALL n2(oo(1).i(2))", "CALL n2(o.q.v(1))", "x% = LBOUND(r.a)", "x% = UBOUND(r.a, 1)", "x% = UBOUND(r.b, 2)", "x% = LBOUND(r.a())",
    "x% = LEN(r.a(1))", "x% = LEN(r.s(1))", "x% = LEN(o.i(1))", "SWAP r.a(1), r.a(2)", "SWAP r.a(1), x%", "SWAP r.s(1), r.s(2)", "SWAP r.a, r.a", "INPUT r.a(1)", "LINE INPUT r.s(1)", "READ r.a(1)", "DATA 1",
    "FOR r.a(1) = 1 TO 2: NEXT", "FOR r.n = 1 TO 2: NEXT", "LSET r.s(1) = \"a\"", "RSET r.s(1) = \"a\"", "LSET r.t = \"a\"", "PUT #1, , r.a(1)", "GET #1, , r.a(1)", "PUT #1, , r.a", "GET #1, , r", "ERASE r.a", "REDIM r.a(3)", "DIM r.a(3)", "DIM SHARED r.a(3)",
    "IF r.a(1) THEN x% = 1", "IF r.a(1) = r.a(2) THEN x% = 1", "SELECT CASE r.a(1)", "CASE 1", "END SELECT", "d# = r.d(1) * r.d(2) + r.a(3)", "r.d(1) = r.d(2) ^ 2", "r.d(i%) = r.a(j%) / 3",
    "x% = r.a(r.a(1))", "r.a(r.a(1)) = r.a(r.a(2))", "x% = r.a(rr(1).a(2))", "r.a(1) = r.a(2) = r.a(3)", "r.b(r.a(1), r.a(2)) = 1", "r.a(1.5) = 1", "r.a(\"x\") = 1", "r.a(1) = \"x\"", "s$ = r.a(1)", "x% = r.s(1)",
    "x% = ABS(r.a(1))", "s$ = LEFT$(r.s(1), 2)", "s$ = MID$(r.t, 2, 1)", "MID$(r.t, 2, 1) = \"x\"", "MID$(r.s(1), 1) = \"x\"", "s$ = STR$(r.a(1)) + CHR$(r.a(2))", "PRINT USING \"##\"; r.a(1)", "WRITE r.a(1), r.s(1)", "PRINT #1, r.a(1)",
    "x% = VARPTR(r.a(1))", "x% = VARSEG(r.a(1))", "s$ = VARPTR$(r.a(1))", "x% = SADD(r.t)", "POKE VARPTR(r.a(1)), 1", "x% = PEEK(VARPTR(r.a(1)))", "DEF SEG = VARSEG(r.a(1))", "BLOAD \"f\", VARPTR(r.a(1))",
    "CALL ABSOLUTE(r.a(1), 0)", "CALL INTERRUPT(1, r.a(1), r.a(2))", "x% = FRE(r.t)", "x% = r.a(1)(2)", "x% = r.a(1).b", "r.a(1).b = 1", "x% = r.a.b", "x% = r.nope(1)", "r.nope(1) = 1", "x% = r(1).a", "x% = r.n.a",
    "x% = r.A(1)", "x% = r.a%(1)", "x% = r.a&(1)", "r.a%(1) = 1", "r.a&(1) = 1", "r.s$(1) = \"x\"", "r.d#(1) = 1", "x# = r.d#(1)", "x# = r.d!(1)", "x% = r.n%", "x& = r.n&", "x% = r.t$", "s$ = r.t$",
    "ON r.a(1) GOTO lab", "lab:", "ON r.a(1) GOSUB lab2", "lab2: RETURN", "x% = r.a(1): r.a(2) = x%", "WHILE r.a(1): WEND", "DO WHILE r.a(1): LOOP", "DO: LOOP UNTIL r.a(1)",
    "OPEN \"f\" FOR RANDOM AS #1 LEN = LEN(r)", "FIELD #1, 4 AS r.t", "x% = LEN(rr(1))", "x% = LEN(rr(1).a(2))", "x% = LEN(Rec)", "x% = LEN(Inner)",
]
for use in RECORD_USES:
    add("records", f"use {use!r}", *RECORDS, use, "END", "SUB p (z AS INTEGER)", "END SUB", "SUB q (z() AS INTEGER)", "END SUB", "SUB m (z AS Rec)", "END SUB", "SUB n2 (z AS Inner)", "END SUB")
# the same in a SUB, a FUNCTION and with SHARED
for use in RECORD_USES[:60]:
    add("records", f"in sub {use!r}", *RECORDS[:-6], "DIM SHARED r AS Rec, rr(3) AS Rec, o AS Outer, oo(2) AS Outer", "DIM x%, i%, j%, s$, d#", "SUB t", use, "END SUB")
    add("records", f"in sub, local record {use!r}", *RECORDS[:-6], "SUB t", "DIM r AS Rec, rr(3) AS Rec, o AS Outer, oo(2) AS Outer", "DIM x%, i%, j%, s$, d#", use, "END SUB")
    add("records", f"in static sub {use!r}", *RECORDS[:-6], "SUB t STATIC", "DIM r AS Rec, rr(3) AS Rec, o AS Outer, oo(2) AS Outer", "DIM x%, i%, j%, s$, d#", use, "END SUB")
# type definitions with array fields
for fields in [["a(5) AS INTEGER"], ["a(1 TO 5) AS INTEGER"], ["a(-3 TO 3) AS INTEGER"], ["a(5, 5) AS INTEGER"], ["a(1 TO 5, 2 TO 6) AS LONG"], ["a(5) AS STRING"], ["a(5) AS STRING * 3"], ["a(5) AS Inner"],
               ["a(5) AS SINGLE", "b AS INTEGER"], ["a(5%) AS INTEGER"], ["a(n) AS INTEGER"], ["a(CONSTN) AS INTEGER"], ["a(0) AS INTEGER"], ["a(-1) AS INTEGER"], ["a(5 TO 1) AS INTEGER"], ["a(32767) AS INTEGER"], ["a(40000) AS INTEGER"],
               ["a(1 TO 5, 1 TO 5, 1 TO 5, 1 TO 5, 1 TO 5, 1 TO 5, 1 TO 5, 1 TO 5) AS INTEGER"], ["a(5) AS INTEGER, b(3) AS INTEGER"], ["a (5) AS INTEGER"], ["a(5)AS INTEGER"], ["a(5) INTEGER"], ["a(5)"], ["a(,) AS INTEGER"], ["a(5.5) AS INTEGER"], ["a(\"x\") AS INTEGER"], ["a(1 TO) AS INTEGER"], ["a(TO 5) AS INTEGER"]]:
    add("records", f"array field defs {fields!r}", "CONST CONSTN = 4", "TYPE Inner", "q AS INTEGER", "END TYPE", "TYPE Rec", *fields, "END TYPE", "DIM r AS Rec", "DIM n AS INTEGER", "r.a(1) = 1")
for opt in ["OPTION BASE 1", "OPTION BASE 0"]:
    add("records", f"{opt} with array field", opt, *RECORDS, "r.a(0) = 1", "r.a(1) = 1", "x% = LBOUND(r.a)", "x% = r.b(0, 0)")
add("records", "redim of record array", *RECORDS, "REDIM rr(5) AS Rec", "rr(1).a(1) = 1")
add("records", "dynamic array of records, field array", *RECORDS[:-6], "REDIM rr(5) AS Rec", "DIM x%", "x% = rr(1).a(1)")
add("records", "array of records passed whole", *RECORDS, "CALL zz(rr())", "END", "SUB zz (z() AS Rec)", "z(1).a(1) = 1", "END SUB")
add("records", "record param array field", *RECORDS[:-6], "DIM r AS Rec", "CALL zz(r)", "END", "SUB zz (z AS Rec)", "z.a(1) = 1", "z.b(1, 2) = 3", "END SUB")
add("records", "function record param array field", *RECORDS[:-6], "DECLARE FUNCTION f% (z AS Rec)", "DIM r AS Rec", "PRINT f%(r)", "END", "FUNCTION f% (z AS Rec)", "f% = z.a(1) + z.b(1, 2)", "END FUNCTION")
add("records", "common record", "TYPE R", "a(3) AS INTEGER", "END TYPE", "COMMON SHARED r AS R", "r.a(1) = 1")
add("records", "static record in sub", "TYPE R", "a(3) AS INTEGER", "END TYPE", "SUB s", "STATIC r AS R", "r.a(1) = 1", "END SUB")
add("records", "shared record in sub", "TYPE R", "a(3) AS INTEGER", "END TYPE", "DIM r AS R", "SUB s", "SHARED r AS R", "r.a(1) = 1", "END SUB")
add("records", "shared record no as", "TYPE R", "a(3) AS INTEGER", "END TYPE", "DIM r AS R", "SUB s", "SHARED r", "r.a(1) = 1", "END SUB")
add("records", "dotted var names beside a record", "TYPE R", "a(3) AS INTEGER", "END TYPE", "DIM r AS R", "r.a(1) = 1", "r.b = 1", "r.b(1) = 1")
add("records", "dotted array and record same stem", "TYPE R", "a AS INTEGER", "END TYPE", "DIM r(3) AS R", "r.a = 1")
add("records", "record same name as array", "TYPE R", "a AS INTEGER", "END TYPE", "DIM r AS R", "DIM r(3) AS INTEGER")
add("records", "def fn with field", "TYPE R", "a(3) AS INTEGER", "END TYPE", "DIM r AS R", "DEF FNa% = r.a(1)", "PRINT FNa%")

# ---- expressions: literals, folding, operators, conversions (appended) -----------------------------------------
EXPRS = [
    # literal boundaries
    "32767", "32768", "-32768", "-32769", "65535", "65536", "&H7FFF", "&H8000", "&HFFFF", "&H10000", "&HFFFFFFFF", "&H100000000", "&HFFFF&", "&H8000&", "&O177777", "&O200000", "&O777777777777", "&O37777777777", "&O40000000000",
    "2147483647", "2147483648", "-2147483648", "-2147483649", "4294967295", "99999999999", "1E38", "3.4E38", "3.5E38", "1E39", "1E-45", "1E-46", "1D308", "1D309", "1D-324", "1.7976931348623157D308", "1.8D308",
    "1.5", "1.5!", "1.5#", "1.5%", "1.5&", "15%", "15&", "15!", "15#", "40000%", "70000&", "3000000000&", "&H", "&HG", "&H1G", "&", "&Q1", "1.2.3", "1..2", ".", "1E", "1E+", "1D", "1E5E5", "12 34", "1 000",
    # arithmetic on constants
    "32767 + 1", "32767 + 1&", "32767& + 1", "-32768 - 1", "200 * 200", "200& * 200", "300 \\ 0", "300 \\ 0.5", "5 MOD 0", "5 MOD 0.4", "5 / 0", "5! / 0", "5# / 0", "0 / 0", "2 ^ 15", "2 ^ 16", "2 ^ 31", "2 ^ 32", "(-2) ^ 0.5", "-2 ^ 2", "10 ^ 40", "0 ^ 0", "0 ^ -1",
    "7 \\ 2", "7 / 2", "-7 \\ 2", "7 MOD -2", "-7 MOD 2", "32767 \\ -1", "-32768 \\ -1", "-32768 * -1", "-(-32768)", "ABS(-32768)", "ABS(-32769)", "SGN(-5)", "INT(-5.5)", "FIX(-5.5)", "INT(1E10)", "CINT(40000)", "CINT(32767.5)", "CINT(-32768.5)", "CLNG(3E9)", "CLNG(2147483647.5)", "CSNG(1E40)", "CSNG(1D39)", "CDBL(1)",
    "SQR(-1)", "LOG(0)", "LOG(-1)", "EXP(100)", "ATN(1)", "SIN(1E10)", "RND", "RND(1)", "TIMER", "5 + 5 * 5 ^ 2", "(5 + 5) * 5", "5 - - 5", "5 - -5", "- - 5", "NOT NOT 5", "5 + NOT 5", "2 * -3", "2 ^ -1", "2 ^ - 1", "-2 ^ -2",
    # logical
    "5 AND 3", "5 OR 3", "5 XOR 3", "5 EQV 3", "5 IMP 3", "NOT 5", "NOT -1", "-1 AND 255", "70000 AND 1", "70000& AND 1", "65535 AND 1", "&HFFFF AND &HFF", "1.5 AND 1", "1.5 OR 2.5", "40000 OR 1", "1 < 2", "1 = 1", "1 <> 2", "1 >= 2", "1 <= 2", "1 > 2", "1 < 2 < 3", "1 = 1 = 1", "(1 < 2) + 1", "-(1 < 2)", "1 =< 2", "1 => 2", "1 >< 2", "1 <> 2 <> 3",
    # strings
    "\"a\" + \"b\"", "\"a\" - \"b\"", "\"a\" * 2", "\"a\" + 1", "1 + \"a\"", "\"a\" < \"b\"", "\"a\" = 1", "1 = \"a\"", "-\"a\"", "NOT \"a\"", "\"a\" AND \"b\"", "\"a\" AND 1", "LEN(\"a\") + 1", "LEN(1)", "\"a\" + CHR$(65)", "\"\"", "\"a", "\"a\"\"b\"", "\"a\" \"b\"", "\"a\"; \"b\"", "STR$(5) + \"a\"", "VAL(\"5\") + 1", "VAL(5)", "\"5\" + 5", "\"a\" ^ 2", "\"a\" MOD 2", "\"a\" \\ 2", "\"a\" / 2", "\"a\" XOR \"b\"", "\"a\" > \"b\" AND 1", "STRING$(3, \"a\") + SPACE$(2)", "STRING$(3, 65)", "STRING$(40000, 65)", "SPACE$(40000)", "SPACE$(-1)", "CHR$(256)", "CHR$(-1)", "CHR$(65.5)", "ASC(\"\")", "LEFT$(\"abc\", -1)", "LEFT$(\"abc\", 70000)", "MID$(\"abc\", 0)", "MID$(\"abc\", 1, 70000)", "INSTR(70000, \"a\", \"b\")", "INSTR(\"a\", \"b\")", "INSTR(0, \"a\", \"b\")",
    # conversions between types
    "5 + 5.5", "5 + 5#", "5& + 5%", "5% + 5%", "5 / 5%", "5 \\ 5.5", "5! + 5#", "CINT(5#)", "CINT(\"5\")", "CLNG(5%)", "CSNG(5&)", "CDBL(\"5\")", "HEX$(255)", "HEX$(-1)", "HEX$(70000)", "HEX$(3000000000#)", "OCT$(8)", "OCT$(-1)", "MKI$(5)", "MKI$(70000)", "MKL$(70000)", "MKS$(1)", "MKD$(1)", "CVI(\"ab\")", "CVI(\"a\")", "CVL(\"abcd\")", "CVS(\"abcd\")", "CVD(\"abcdefgh\")",
    "PEEK(0)", "PEEK(65536)", "PEEK(-1)", "INP(&H3DA)", "INP(70000)", "FRE(0)", "FRE(\"a\")", "FRE(-1)", "FRE(-2)", "FRE(-3)", "CSRLIN", "POS(0)", "POS(-1)", "LPOS(1)", "ERR", "ERL", "ERDEV", "DATE$", "TIME$", "ENVIRON$(\"PATH\")", "ENVIRON$(1)", "COMMAND$", "INKEY$", "INPUT$(1)", "INPUT$(0)", "EOF(1)", "LOF(1)", "LOC(1)", "SEEK(1)", "FREEFILE", "VARPTR(i%)", "VARSEG(i%)", "SADD(s$)", "VARPTR$(i%)",
    "x%", "x%(1)", "x", "x$ + x$", "x& * x&", "x% * x%", "x# ^ x#", "x% \\ x%", "x% MOD x%", "x% AND x%", "x$ < x$", "x% < x$", "x$ = x%", "FNa", "FNa(1)", "FNa%(1)", "FNb$", "FNb$(1)", "FN a", "FN a(1)", "FNa (1)",
]
EXPR_CONTEXTS = [
    ("x% = {}", "x% = {}"), ("d# = {}", "d# = {}"), ("s$ = {}", "s$ = {}"), ("l& = {}", "l& = {}"), ("sg! = {}", "sg! = {}"),
    ("if", "IF {} THEN x% = 1"), ("print", "PRINT {}"), ("paren", "x% = ({})"), ("const", "CONST k = {}"), ("dim", "DIM a({})"), ("for", "FOR x% = 1 TO {}: NEXT"), ("select", "SELECT CASE {}\nCASE 1\nEND SELECT"), ("call", "CALL pp({})"),
]
for expr in EXPRS:
    for name, template in EXPR_CONTEXTS:
        add("exprs", f"{name} {expr!r}", "DIM x%, d#, s$, l&, sg!, a%(5)", "DECLARE SUB pp (z AS INTEGER)", "DEF FNa = 1", "DEF FNb$ = \"a\"", template.format(expr), "END", "SUB pp (z AS INTEGER)", "END SUB")

# ---- constant folding: every operator on integer-range boundaries, in a few contexts (appended) --------------------
FOLD_OPERANDS = ["32767", "-32768", "32768", "1", "-1", "0", "40000", "70000&", "1.5"]
FOLD_OPS = ["+", "-", "*", "\\", "MOD", "/", "AND", "OR", "XOR", "^"]
FOLD_CONTEXTS = [("x%", "x% = {}"), ("l&", "l& = {}"), ("call", "CALL pp({})"), ("arr", "x% = a%({})")]
for left in FOLD_OPERANDS:
    for op in FOLD_OPS:
        for right in FOLD_OPERANDS:
            expr = f"{left} {op} {right}"
            for name, template in FOLD_CONTEXTS[:3]:
                add("fold", f"{name} '{expr}'", "DIM x%, l&, a%(5)", "DECLARE SUB pp (z AS INTEGER)", template.format(expr), "END", "SUB pp (z AS INTEGER)", "END SUB")
for operand in FOLD_OPERANDS:
    for expr in [f"-{operand}", f"NOT {operand}", f"-({operand})", f"NOT ({operand})", f"ABS({operand})", f"INT({operand})", f"FIX({operand})", f"SGN({operand})", f"CINT({operand})", f"CLNG({operand})", f"CSNG({operand})", f"CDBL({operand})", f"{operand} + 0", f"0 + {operand}", f"({operand})"]:
        for name, template in FOLD_CONTEXTS[:3]:
            add("fold", f"{name} '{expr}'", "DIM x%, l&, a%(5)", "DECLARE SUB pp (z AS INTEGER)", template.format(expr), "END", "SUB pp (z AS INTEGER)", "END SUB")


import more  # noqa: E402

more.register(add)


def write(out=None):
    out = Path(out) if out else HERE / "programs"
    out.mkdir(exist_ok=True)
    for old in out.glob("*.bas"):
        old.unlink()
    with open(out / "programs.jsonl" if out != HERE / "programs" else HERE / "programs.jsonl", "w") as index:
        for n, program in enumerate(PROGRAMS):
            program["id"] = n
            (out / f"p{n:05d}.bas").write_text(program["text"])
            index.write(json.dumps({k: v for k, v in program.items() if k != "text"}) + "\n")
    print(len(PROGRAMS), "programs")


if __name__ == "__main__":
    import sys
    write(sys.argv[1] if len(sys.argv) > 1 else None)
