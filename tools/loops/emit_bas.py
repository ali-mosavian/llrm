"""
A case as QuickBASIC 4.5, for llrm-qb and for BC, which validates the oracle.

BASIC has INTEGER, LONG, SINGLE and DOUBLE; no unsigned or byte types, no
pointers, shifts, CONTINUE or local static arrays. A case that needs one is
not expressible here, and says so. Parameters pass by reference, as QB does.
"""

from __future__ import annotations

from spec import (
    Int, Float, Struct, Ptr, Case, Array, Const, Var, Bin, Neg, Cast, Cmp, Not, Logic, Load, AddrOf, PtrAdd,
    Deref, Len, CallE, Assign, For, While, DoWhile, If, Break, Continue, Return, CallS, I16, I32, F32, F64,
    written_arrays, walk, backing,
)


class NotBasic(Exception):
    pass


TYPE = {I16: "INTEGER", I32: "LONG", F32: "SINGLE", F64: "DOUBLE"}
SIGIL = {I16: "%", I32: "&", F32: "!", F64: "#"}
RESERVED = {
    "abs", "and", "as", "asc", "call", "case", "cint", "clng", "close", "cls", "color", "common", "const", "data",
    "date", "def", "dim", "do", "else", "end", "eqv", "erl", "err", "error", "exit", "field", "fix", "for", "fn",
    "function", "get", "gosub", "goto", "if", "imp", "input", "int", "is", "key", "len", "let", "line", "list",
    "loc", "lock", "log", "loop", "mod", "name", "next", "not", "on", "open", "or", "out", "peek", "poke", "pos",
    "print", "put", "read", "rem", "return", "run", "screen", "seek", "sgn", "shared", "sin", "sqr", "static",
    "step", "stop", "sub", "swap", "tab", "tan", "then", "time", "timer", "to", "type", "until", "val", "wait",
    "wend", "while", "width", "write", "xor", "random", "base", "option", "erase", "redim", "sound", "play",
    "draw", "paint", "view", "window", "using", "seg", "string", "integer", "long", "single", "double", "any",
    "lbound", "ubound", "signal", "stick", "strig", "pen", "event", "open", "cos", "atn", "exp", "hex", "oct",
    "chr", "str", "space", "left", "right", "mid", "instr", "lcase", "ucase", "ltrim", "rtrim", "csng", "cdbl",
}


def btype(kind) -> str:
    if isinstance(kind, Struct):
        for _, field in kind.fields:
            btype(field)  # a TYPE holds only what BASIC has
        return kind.name.upper()
    if kind not in TYPE:
        raise NotBasic(f"no BASIC type for {getattr(kind, 'name', kind)}")
    return TYPE[kind]


def ident(name: str) -> str:
    """A QB name: letters and digits only, never a keyword."""
    cleaned = "".join(ch for ch in name if ch.isalnum())
    if cleaned.lower() in RESERVED or not cleaned[:1].isalpha():
        cleaned = "v" + cleaned
    return cleaned


class Emitter:
    def __init__(self, case: Case, number: int):
        self.case = case
        self.number = number
        self.types = case.types()
        self.labels = 0
        # innermost first: (kind, continue label)
        self.loops: list[tuple[str, str]] = []

    @property
    def fname(self) -> str:
        return f"F{self.number}{SIGIL[self.ret]}"

    @property
    def ret(self):
        if self.case.ret not in (I16, I32):
            raise NotBasic(f"a {self.case.ret.name} result")
        return self.case.ret

    def gname(self, array: Array) -> str:
        return f"G{self.number}{ident(array.name)}"

    def type_of(self, e):
        from oracle import Machine

        return Machine(self.case, "bas").type_of(e)

    def const(self, value: int, kind) -> str:
        if isinstance(kind, Float):
            text = f"{value}{SIGIL[kind]}"
        elif kind == I32:
            text = f"{value}&"
        elif kind == I16:
            text = str(value)
        else:
            raise NotBasic(f"a {kind.name} constant")
        return f"({text})" if value < 0 else text

    def expr(self, e) -> str:
        if isinstance(e, Const):
            return self.const(e.value, e.type)
        if isinstance(e, Var):
            kind = self.types[e.name]
            if isinstance(kind, Ptr):
                raise NotBasic("a pointer")
            btype(kind)
            return ident(e.name)
        if isinstance(e, Bin):
            kind = self.type_of(e.a)
            btype(kind)
            a, b = self.expr(e.a), self.expr(e.b)
            op = {"/": "\\", "%": "MOD", "&": "AND", "|": "OR", "^": "XOR"}.get(e.op, e.op)
            if e.op in ("min", "max", "<<", ">>"):
                raise NotBasic(f"the operator {e.op}")
            if e.op == "/" and isinstance(kind, Float):
                op = "/"
            return f"({a} {op} {b})"
        if isinstance(e, Neg):
            return f"(-{self.expr(e.a)})"
        if isinstance(e, Cast):
            source = self.type_of(e.a)
            btype(source), btype(e.type)
            if isinstance(e.type, Float):
                return self.expr(e.a)  # exact, and BASIC converts as it assigns
            return f"{'CLNG' if e.type == I32 else 'CINT'}({self.expr(e.a)})"
        if isinstance(e, Cmp):
            op = {"==": "=", "!=": "<>"}.get(e.op, e.op)
            return f"({self.expr(e.a)} {op} {self.expr(e.b)})"
        if isinstance(e, Not):
            return f"(NOT {self.expr(e.a)})"
        if isinstance(e, Logic):
            return f"({self.expr(e.a)} {e.op.upper()} {self.expr(e.b)})"
        if isinstance(e, Load):
            text = f"{self.array_ref(e.array)}({', '.join(self.expr(one) for one in e.index)})"
            return text + (f".{ident(e.field)}" if e.field else "")
        if isinstance(e, (AddrOf, PtrAdd, Deref)):
            raise NotBasic("a pointer")
        if isinstance(e, Len):
            return f"(UBOUND({self.array_ref(e.array)}, {e.dim + 1}) + 1)"
        if isinstance(e, CallE):
            raise NotBasic("an external call")
        raise NotBasic(f"no BASIC for {e}")

    def array_ref(self, name: str) -> str:
        array = self.case.array(name)
        if array.where == "local":
            raise NotBasic("a local array")
        if array.bytes > 30000:
            raise NotBasic("a static array this large does not fit DGROUP")
        btype(array.elem)
        return self.gname(array) if array.where == "global" else ident(name)

    def label(self) -> str:
        self.labels += 1
        return f"L{self.number}x{self.labels}"

    def assign(self, s: Assign) -> str:
        return f"{self.expr(s.place)} = {self.expr(s.value)}"

    def block(self, stmts, depth: int) -> list[str]:
        out = []
        pad = "    " * depth
        for s in stmts:
            if isinstance(s, Assign):
                out.append(pad + self.assign(s))
            elif isinstance(s, For):
                out += self.for_(s, depth)
            elif isinstance(s, While):
                label = self.label()
                self.loops.append(("DO", label))
                body = self.block(s.body, depth + 1)
                self.loops.pop()
                out += [pad + f"DO WHILE {self.expr(s.cond)}", *body, *self.tail(label, body, pad), pad + "LOOP"]
            elif isinstance(s, DoWhile):
                label = self.label()
                self.loops.append(("DO", label))
                body = self.block(s.body, depth + 1)
                self.loops.pop()
                out += [pad + "DO", *body, *self.tail(label, body, pad), pad + f"LOOP WHILE {self.expr(s.cond)}"]
            elif isinstance(s, If):
                out += [pad + f"IF {self.expr(s.cond)} THEN", *self.block(s.then, depth + 1)]
                if s.orelse:
                    out += [pad + "ELSE", *self.block(s.orelse, depth + 1)]
                out.append(pad + "END IF")
            elif isinstance(s, Break):
                out.append(pad + f"EXIT {self.loops[-1][0]}")
            elif isinstance(s, Continue):
                out.append(pad + f"GOTO {self.loops[-1][1]}")
            elif isinstance(s, Return):
                out += [pad + f"{self.fname} = {self.expr(s.value)}", pad + "EXIT FUNCTION"]
            elif isinstance(s, CallS):
                raise NotBasic("an external call")
            else:
                raise NotBasic(f"no BASIC for {s}")
        return out

    @staticmethod
    def tail(label: str, body: list[str], pad: str) -> list[str]:
        used = any(line.strip() == f"GOTO {label}" for line in body)
        return [f"{label}:"] if used else []

    def for_(self, s: For, depth: int) -> list[str]:
        pad = "    " * depth
        kind = self.types[s.var]
        btype(kind)
        var = ident(s.var)
        counted = s.cond != "!=" and (
            not isinstance(s.step, Const) or (s.step.value > 0) == (s.cond in ("<", "<="))
        )
        if counted:
            end = self.expr(s.end)
            one = self.const(1, kind)
            limit = {"<": f"{end} - {one}", ">": f"{end} + {one}"}.get(s.cond, end)
            step = "" if isinstance(s.step, Const) and s.step.value == 1 else f" STEP {self.expr(s.step)}"
            label = self.label()
            self.loops.append(("FOR", label))
            body = self.block(s.body, depth + 1)
            self.loops.pop()
            return [pad + f"FOR {var} = {self.expr(s.start)} TO {limit}{step}", *body, *self.tail(label, body, pad),
                    pad + "NEXT"]
        label = self.label()
        self.loops.append(("DO", label))
        body = self.block(s.body, depth + 1)
        self.loops.pop()
        op = {"==": "=", "!=": "<>"}.get(s.cond, s.cond)
        return [
            pad + f"{var} = {self.expr(s.start)}",
            pad + f"DO WHILE {var} {op} {self.expr(s.end)}",
            *body,
            *([f"{label}:"] if any(line.strip() == f"GOTO {label}" for line in body) else []),
            pad + f"    {var} = {var} + {self.expr(s.step)}",
            pad + "LOOP",
        ]

    def params(self) -> list[str]:
        case = self.case
        out = [f"{ident(a.name)}() AS {btype(a.elem)}" for a in case.arrays if a.where == "param"]
        for name, kind in case.params:
            if isinstance(kind, Ptr):
                raise NotBasic("a pointer parameter")
            out.append(f"{ident(name)} AS {btype(kind)}")
        return out

    def declare(self) -> str:
        return f"DECLARE FUNCTION {self.fname} ({', '.join(self.params())})"

    def function(self) -> list[str]:
        case = self.case
        for array in case.arrays:
            self.array_ref(array.name)
        out = [f"FUNCTION {self.fname} ({', '.join(self.params())})"]
        for name, kind in case.locals:
            if isinstance(kind, Ptr):
                raise NotBasic("a pointer")
            out.append(f"    DIM {ident(name)} AS {btype(kind)}")
        out += self.block(case.body, 1)
        out.append("END FUNCTION")
        return out


def storage(case: Case, e: Emitter) -> list[str]:
    out = []
    for array in case.arrays:
        if array.alias:
            continue
        dims = ", ".join(f"0 TO {one - 1}" for one in array.dims)
        out.append(f"DIM SHARED {e.gname(array)}({dims}) AS {btype(array.elem)}")
    return out


def fields_of(elem):
    return list(elem.fields) if isinstance(elem, Struct) else [(None, elem)]


def helper_name(prefix: str, elem, rank: int) -> str:
    return f"{prefix}{btype(elem)[:3]}{rank}"


def helpers(elem, rank: int) -> list[str]:
    """Fill and digest for arrays of `elem` of `rank`, as oracle.fill_value
    and oracle.digest define them, in LONG arithmetic that cannot overflow."""
    t = btype(elem)
    idx = ", ".join(f"k{d}" for d in range(rank))
    loops_open, loops_close = [], []
    extents = ", ".join(f"n{d} AS LONG" for d in range(rank))
    for d in range(rank):
        loops_open.append("    " * (d + 1) + f"FOR k{d} = 0 TO n{d} - 1")
        loops_close.insert(0, "    " * (d + 1) + "NEXT")
    linear = "k0"
    for d in range(1, rank):
        linear = f"({linear}) * n{d} + k{d}"
    pad = "    " * (rank + 1)
    fill = [f"SUB {helper_name('FIL', elem, rank)} (a() AS {t}, {extents}, seed AS LONG, lo AS LONG, span AS LONG, stp AS LONG)",
            f"    DIM {', '.join(f'k{d} AS LONG' for d in range(rank))}, k AS LONG, x AS LONG, x2 AS LONG, s AS LONG",
            *loops_open, pad + f"k = {linear}"]
    for f, (name, kind) in enumerate(fields_of(elem)):
        place = f"a({idx})" + (f".{ident(name)}" if name else "")
        fill += [
            pad + f"s = seed + {101 * f}",
            pad + "x = (k * 7919 + s) MOD 65521",
            pad + "IF stp THEN",
            pad + f"    {place} = lo + k * stp",
            pad + "ELSEIF span THEN",
            pad + f"    {place} = lo + x MOD span",
            pad + "ELSE",
        ]
        if isinstance(kind, Float):
            fill.append(pad + f"    {place} = x - 32760")
        elif kind == I32:
            fill += [pad + "    x2 = (k * 7919 + s + 17) MOD 65521", pad + f"    {place} = (x - 32760) * 65536 + x2"]
        else:
            fill += [pad + "    IF x > 32767 THEN x = x - 65536", pad + f"    {place} = x"]
        fill.append(pad + "END IF")
    fill += [*loops_close, "END SUB"]
    dig = [f"FUNCTION {helper_name('DIG', elem, rank)}& (a() AS {t}, {extents})",
           f"    DIM {', '.join(f'k{d} AS LONG' for d in range(rank))}, d AS LONG, v AS LONG",
           *loops_open]
    for name, kind in fields_of(elem):
        place = f"a({idx})" + (f".{ident(name)}" if name else "")
        dig.append(pad + f"v = {place}")
        dig.append(pad + "d = (d * 31 + (v AND &HFFFF&)) MOD 65521")
        if kind != I16:
            dig += [pad + "v = (v AND &H7FFF0000) \\ &H10000 - 32768 * (v < 0)",
                    pad + "d = (d * 31 + v) MOD 65521"]
    dig += [*loops_close, f"    {helper_name('DIG', elem, rank)}& = d", "END FUNCTION"]
    return fill + dig


def driver(cases: list[Case], numbers: dict, plans: dict) -> str:
    """One module: every case's FUNCTION, called on its valid inputs,
    PRINTing the report stream."""
    types, head, body, subs, used = [], [], [], [], {}
    for case in cases:
        e = Emitter(case, numbers[case.name])
        for array in case.arrays:
            if isinstance(array.elem, Struct) and array.elem.name not in {t[0] for t in types}:
                fields = [f"    {ident(n)} AS {btype(k)}" for n, k in array.elem.fields]
                types.append((array.elem.name, [f"TYPE {btype(array.elem)}", *fields, "END TYPE"]))
            used[(array.elem, len(array.dims))] = True
        head.append(e.declare())
        body += storage(case, e)
    for (elem, rank) in used:
        t = btype(elem)
        extents = ", ".join(f"n{d} AS LONG" for d in range(rank))
        head.append(f"DECLARE SUB {helper_name('FIL', elem, rank)} (a() AS {t}, {extents}, seed AS LONG, lo AS LONG, span AS LONG, stp AS LONG)")
        head.append(f"DECLARE FUNCTION {helper_name('DIG', elem, rank)}& (a() AS {t}, {extents})")
        subs += helpers(elem, rank)
    main = ["DIM r AS LONG"]
    for case in cases:
        e = Emitter(case, numbers[case.name])
        for at in plans[case.name]:
            inp = case.inputs[at]
            fills = dict(inp.fills)
            for array in case.arrays:
                if array.alias:
                    continue
                f = fills[array.name]
                extents = ", ".join(f"{one}&" for one in array.dims)
                main.append(f"{helper_name('FIL', array.elem, len(array.dims))} {e.gname(array)}(), {extents}, {f.seed}, {f.lo}, {f.span}, {f.step}")
            for name, at, value in inp.pokes:
                array = case.array(name)
                index, rest = [], at
                for extent in reversed(array.dims):
                    index.insert(0, rest % extent)
                    rest //= extent
                main.append(f"{e.gname(array)}({', '.join(map(str, index))}) = {e.const(value, array.elem)}")
            args = [f"{e.gname(backing(case, a))}()" for a in case.arrays if a.where == "param"]
            args += [f"({e.const(value, kind)})" for (_, kind), value in zip(case.params, inp.args)]
            main.append(f"r = {e.fname}({', '.join(args)})")
            main.append("PRINT r")
            for name in written_arrays(case):
                array = case.array(name)
                extents = ", ".join(f"{one}&" for one in array.dims)
                main.append(f"r = {helper_name('DIG', array.elem, len(array.dims))}&({e.gname(backing(case, array))}(), {extents})")
                main.append("PRINT r")
    functions = []
    for case in cases:
        functions += Emitter(case, numbers[case.name]).function()
    lines = [*(line for _, t in types for line in t), *head, *body, *main, "END", *functions, *subs]
    return "\r\n".join(wrapped(line) for line in lines) + "\r\n"


LINE = 200  # BC refuses a line past 255 characters


def wrapped(line: str) -> str:
    """A long line continued with ` _`, after a comma where there is one."""
    out = []
    while len(line) > LINE:
        cut = line.rfind(", ", 0, LINE)
        cut = cut + 1 if cut > 0 else line.rfind(" ", 0, LINE)
        if cut <= 0:
            break
        out.append(line[:cut] + " _")
        line = "    " + line[cut:].lstrip()
    return "\r\n".join([*out, line])


def expressible(case: Case) -> str | None:
    try:
        Emitter(case, 1).function()
        return None
    except NotBasic as why:
        return str(why)
