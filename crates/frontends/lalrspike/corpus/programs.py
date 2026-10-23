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


def write():
    out = HERE / "programs"
    out.mkdir(exist_ok=True)
    for old in out.glob("*.bas"):
        old.unlink()
    with open(HERE / "programs.jsonl", "w") as index:
        for n, program in enumerate(PROGRAMS):
            program["id"] = n
            (out / f"p{n:05d}.bas").write_text(program["text"])
            index.write(json.dumps({k: v for k, v in program.items() if k != "text"}) + "\n")
    print(len(PROGRAMS), "programs")


if __name__ == "__main__":
    write()
