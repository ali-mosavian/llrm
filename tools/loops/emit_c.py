"""
A case as C, for llrm-c, Open Watcom and gcc-ia16 (16-bit int), and for the
host's clang, which validates the oracle (`host=True`: fixed-width types).

Arithmetic narrower than int, and u16's, is cast back to its type where it
is an operand, so the host (32-bit int) and the target mean the same; signed
overflow at int width is left out by the oracle, so needs no cast.
"""

from __future__ import annotations

from spec import (
    Int, Float, Struct, Ptr, Case, Array, Const, Var, Bin, Neg, Cast, Cmp, Not, Logic, Load, AddrOf, PtrAdd,
    Deref, Len, CallE, Assign, For, While, DoWhile, If, Break, Continue, Return, CallS, Fill, OPAQUE,
    written_arrays, walk, I16, F80, backing,
)


class NotC(Exception):
    pass


def ctype(kind) -> str:
    if isinstance(kind, Int):
        return kind.name
    if isinstance(kind, Float):
        return {32: "float", 64: "double", 80: "long double"}[kind.bits]
    if isinstance(kind, Struct):
        return f"struct {kind.name}"
    raise NotC(f"no C type for {kind}")


def qualifier(kind: str) -> str:
    return {"near": "", "far": "FAR ", "huge": "HUGE "}[kind]


PRELUDE = {
    False: """\
typedef signed char i8; typedef unsigned char u8; typedef int i16; typedef unsigned u16;
typedef long i32; typedef unsigned long u32;
#define FAR __far
#define HUGE __huge
""",
    True: """\
#include <stdint.h>
#include <stdio.h>
#include <string.h>
typedef int8_t i8; typedef uint8_t u8; typedef int16_t i16; typedef uint16_t u16;
typedef int32_t i32; typedef uint32_t u32;
#define FAR
#define HUGE
""",
}


class Emitter:
    def __init__(self, case: Case, host: bool = False, prefix: str = ""):
        self.case = case
        self.host = host
        self.types = case.types()
        self.prefix = prefix

    def gname(self, array: Array) -> str:
        return f"{self.prefix}{self.case.symbol}_{array.name}"

    # expressions

    def type_of(self, e):
        from oracle import Machine

        return Machine(self.case, "c").type_of(e)

    def const(self, value: int, kind) -> str:
        if isinstance(kind, Float):
            suffix = {32: "f", 64: "", 80: "L"}[kind.bits]
            return f"({value}.0{suffix})"
        text = str(value)
        if kind.bits == 32:
            text += "L" if kind.signed else "UL"
        elif not kind.signed and kind.bits == 16:
            text += "u"
        return f"({text})" if value < 0 else text

    def expr(self, e, operand: bool = True) -> str:
        """`operand`: the value feeds another operation, so narrow arithmetic
        must be cast back to its type first."""
        if isinstance(e, Const):
            return self.const(e.value, e.type)
        if isinstance(e, Var):
            return e.name
        if isinstance(e, Bin):
            kind = self.type_of(e.a)
            a, b = self.expr(e.a), self.expr(e.b)
            if e.op in ("min", "max"):
                test = "<" if e.op == "min" else ">"
                return f"({a} {test} {b} ? {a} : {b})"
            if isinstance(kind, Int) and kind.bits == 16 and not kind.signed and e.op == "*":
                a = f"(unsigned){a}"
            text = f"{a} {e.op} {b}"
            narrow = isinstance(kind, Int) and (kind.bits < 16 or (kind.bits == 16 and not kind.signed))
            if narrow and (operand or self.host):
                return f"(({kind.name})({text}))"
            return f"({text})"
        if isinstance(e, Neg):
            kind = self.type_of(e.a)
            narrow = isinstance(kind, Int) and (kind.bits < 16 or not kind.signed)
            return f"(({kind.name})(-{self.expr(e.a)}))" if narrow else f"(-{self.expr(e.a)})"
        if isinstance(e, Cast):
            source = self.type_of(e.a)
            if F80 in (source, e.type) and source != e.type:
                # wccq converts long double only to and from double
                return f"(({ctype(e.type)})(double){self.expr(e.a)})"
            return f"(({ctype(e.type)}){self.expr(e.a)})"
        if isinstance(e, Cmp):
            return f"({self.expr(e.a)} {e.op} {self.expr(e.b)})"
        if isinstance(e, Not):
            return f"(!{self.expr(e.a)})"
        if isinstance(e, Logic):
            return f"({self.expr(e.a)} {'&&' if e.op == 'and' else '||'} {self.expr(e.b)})"
        if isinstance(e, Load):
            text = self.array_ref(e.array) + "".join(f"[{self.expr(one, False)}]" for one in e.index)
            return text + (f".{e.field}" if e.field else "")
        if isinstance(e, AddrOf):
            return f"&{self.array_ref(e.array)}[{self.expr(e.index, False)}]"
        if isinstance(e, PtrAdd):
            by = e.by
            if isinstance(by, Neg):
                return f"({self.expr(e.ptr)} - {self.expr(by.a)})"
            return f"({self.expr(e.ptr)} + {self.expr(by)})"
        if isinstance(e, Deref):
            at = e.at
            text = f"*{self.expr(e.ptr)}" if isinstance(at, Const) and at.value == 0 and not e.field else None
            if text:
                return f"({text})"
            text = f"{self.expr(e.ptr)}[{self.expr(at, False)}]"
            return text + (f".{e.field}" if e.field else "")
        if isinstance(e, Len):
            array = self.case.array(e.array)
            if array.where == "param":
                raise NotC("a parameter array's length")
            ref = self.array_ref(e.array)
            inner = "[0]" * (len(array.dims) - 1 - e.dim)
            outer = "[0]" * e.dim
            return f"((i16)(sizeof({ref}{outer}) / sizeof({ref}{outer}[0])))" if not inner else f"((i16)(sizeof({ref}{outer}) / sizeof({ref}{outer}[0])))"
        if isinstance(e, CallE):
            return f"{e.name}({', '.join(self.expr(one, False) for one in e.args)})"
        raise NotC(f"no C for {e}")

    def array_ref(self, name: str) -> str:
        array = self.case.array(name)
        return self.gname(array) if array.where == "global" else name

    # statements

    def place(self, p) -> str:
        return self.expr(p, False)

    def assign(self, s: Assign) -> str:
        place = self.place(s.place)
        value = s.value
        if isinstance(value, Bin) and value.a == s.place and value.op in "+-*&|^" and len(value.op) == 1:
            if value.op in "+-" and isinstance(value.b, Const) and value.b.value == 1:
                return f"{place}{value.op * 2};"
            return f"{place} {value.op}= {self.expr(value.b, False)};"
        return f"{place} = {self.expr(value, False)};"

    def step(self, var: str, step) -> str:
        if isinstance(step, Const):
            if step.value == 1:
                return f"{var}++"
            if step.value == -1:
                return f"{var}--"
            if step.value < 0:
                return f"{var} -= {-step.value}"
        return f"{var} += {self.expr(step, False)}"

    def block(self, stmts, depth: int) -> list[str]:
        out = []
        pad = "    " * depth
        for s in stmts:
            if isinstance(s, Assign):
                out.append(pad + self.assign(s))
            elif isinstance(s, For):
                head = f"for ({s.var} = {self.expr(s.start, False)}; {s.var} {s.cond} {self.expr(s.end, False)}; {self.step(s.var, s.step)})"
                out += [pad + head + " {", *self.block(s.body, depth + 1), pad + "}"]
            elif isinstance(s, While):
                out += [pad + f"while ({self.expr(s.cond, False)}) {{", *self.block(s.body, depth + 1), pad + "}"]
            elif isinstance(s, DoWhile):
                out += [pad + "do {", *self.block(s.body, depth + 1), pad + f"}} while ({self.expr(s.cond, False)});"]
            elif isinstance(s, If):
                out += [pad + f"if ({self.expr(s.cond, False)}) {{", *self.block(s.then, depth + 1)]
                if s.orelse:
                    out += [pad + "} else {", *self.block(s.orelse, depth + 1)]
                out.append(pad + "}")
            elif isinstance(s, Break):
                out.append(pad + "break;")
            elif isinstance(s, Continue):
                out.append(pad + "continue;")
            elif isinstance(s, Return):
                out.append(pad + f"return {self.expr(s.value, False)};")
            elif isinstance(s, CallS):
                out.append(pad + f"{s.name}({', '.join(self.expr(one, False) for one in s.args)});")
            else:
                raise NotC(f"no C for {s}")
        return out

    def decl(self, name: str, kind) -> str:
        if isinstance(kind, Ptr):
            return f"{ctype(kind.elem)} {qualifier(kind.kind)}*{name}"
        return f"{ctype(kind)} {name}"

    def array_param(self, array: Array) -> str:
        q = qualifier(array.ptr)
        if len(array.dims) == 1:
            return f"{ctype(array.elem)} {q}*{array.name}"
        rest = "".join(f"[{one}]" for one in array.dims[1:])
        return f"{ctype(array.elem)} ({q}*{array.name}){rest}"

    def signature(self) -> str:
        case = self.case
        params = [self.array_param(one) for one in case.arrays if one.where == "param"]
        params += [self.decl(name, kind) for name, kind in case.params]
        return f"{case.ret.name} {case.symbol}({', '.join(params) or 'void'})"

    def function(self) -> list[str]:
        case = self.case
        if sum(a.bytes for a in case.arrays if a.where == "local") > 12000:
            raise NotC("local arrays past three quarters of the corpus's 16K stack")
        out = [self.signature(), "{"]
        for array in case.arrays:
            if array.where == "local":
                dims = "".join(f"[{one}]" for one in array.dims)
                out.append(f"    {ctype(array.elem)} {array.name}{dims};")
        for name, kind in case.locals:
            out.append(f"    {self.decl(name, kind)};")
        for array in case.arrays:
            if array.where == "local":
                out.append(f"    lcopy((void FAR *){array.name}, (void FAR *){self.gname(array)}, {array.bytes}u);")
        out += self.block(case.body, 1)
        out.append("}")
        return out


def structs(cases: list[Case]) -> list[str]:
    seen, out = set(), []
    for case in cases:
        for array in case.arrays:
            if isinstance(array.elem, Struct) and array.elem.name not in seen:
                seen.add(array.elem.name)
                fields = " ".join(f"{ctype(kind)} {name};" for name, kind in array.elem.fields)
                out.append(f"struct {array.elem.name} {{ {fields} }};")
    return ["#pragma pack(1)", *out] if out else []


def storage(case: Case, emitter: Emitter) -> list[str]:
    """The case's arrays as the driver owns them: every one but a local's frame copy."""
    out = []
    for array in case.arrays:
        if array.alias:
            continue
        q = qualifier(array.ptr) if array.where != "local" else qualifier("far" if array.bytes > 30000 else "near")
        dims = "".join(f"[{one}]" for one in array.dims)
        out.append(f"{ctype(array.elem)} {q}{emitter.gname(array)}{dims};")
    return out


# --- the driver ------------------------------------------------------------

FILLX = "static u32 fillx(u32 k, u32 seed) { return (k * 7919UL + seed) % 65521UL; }"
DIG = "static u32 dig;\nstatic void fold(u32 v) { dig = (dig * 31UL + v) % 65521UL; }"


def _full(kind) -> str:
    """oracle.fill_value's whole-range value of element k, as C."""
    if isinstance(kind, Float):
        return "(i32)x - 32760L"
    if kind.bits == 32:
        return "((i32)x - 32760L) * 65536L + (i32)fillx(k, seed + 17)"
    if kind.bits == 16:
        return f"({kind.name})(u16)x"
    return f"({kind.name})(u8)(x & 0xFF)"


def helpers(case: Case, e: "Emitter") -> list[str]:
    """Per array a fill and a digest, each for its own element type:
    straight code, the same arithmetic the oracle does."""
    out = []
    for array in case.arrays:
        if array.alias:
            continue
        g = e.gname(array)
        q = "HUGE " if array.bytes > 65535 else "FAR "
        elem = ctype(array.elem)
        out.append(f"static void fill_{g}(u16 seed0, i32 lo, i32 span, i32 step)\n{{")
        out.append(f"    {elem} {q}*p = ({elem} {q}*){g}; u32 k, x, seed;")
        out.append(f"    for (k = 0; k < {array.count}UL; k++) {{")
        for f, (name, kind) in enumerate(_named_fields(array.elem)):
            place = "p[k]" + (f".{name}" if name else "")
            # wccq converts long double only to and from double
            t = ctype(kind) + (")(double" if kind == F80 else "")
            out.append(f"        seed = seed0 + {101 * f}u; x = fillx(k, seed);")
            out.append(f"        if (step) {place} = ({t})(lo + (i32)k * step);")
            out.append(f"        else if (span) {place} = ({t})(lo + (i32)(x % (u32)span));")
            out.append(f"        else {place} = ({t})({_full(kind)});")
        out += ["    }", "}"]
        if any(backing(case, case.array(w)) is array or backing(case, case.array(w)) == array for w in written_arrays(case)):
            out.append(f"static i32 dig_{g}(void)\n{{")
            out.append(f"    {elem} {q}*p = ({elem} {q}*){g}; u32 k, v;")
            out.append(f"    dig = 0;\n    for (k = 0; k < {array.count}UL; k++) {{")
            for name, kind in _named_fields(array.elem):
                place = "p[k]" + (f".{name}" if name else "")
                if isinstance(kind, Float):
                    value = f"(u32)(i32)(double){place}"
                elif kind.bits == 32:
                    value = f"(u32){place}"
                else:
                    value = f"(u32)(u{kind.bits}){place}"
                out.append(f"        v = {value}; fold(v & 0xFFFFUL);")
                if isinstance(kind, Float) or kind.bits == 32:
                    out.append("        fold(v >> 16);")
            out += ["    }", "    return (i32)dig;", "}"]
    return out


def _named_fields(elem):
    return list(elem.fields) if isinstance(elem, Struct) else [(None, elem)]


def kind_code(kind) -> int:
    if isinstance(kind, Float):
        return {32: 40, 64: 41, 80: 42}[kind.bits]
    return kind.bits + (0 if kind.signed else 1)


def fields_of(elem) -> list[tuple[int, object]]:
    """(byte offset, scalar type) of each field, in order."""
    if isinstance(elem, Struct):
        out, at = [], 0
        for _, kind in elem.fields:
            out.append((at, kind))
            at += kind.size
        return out
    return [(0, elem)]


def driver(cases: list[Case], plans: dict, host: bool) -> str:
    """One C program: every case, then main calling each on its valid inputs.
    `plans[name]` lists the input indices the oracle left valid in C."""
    out = [PRELUDE[host]]
    if host:
        out.append("static void report(i32 v) { printf(\"%ld\\n\", (long)v); }")
        out.append("static i16 keep(i16 x) { return x; } static i32 keep32(i32 x) { return x; }")
        out.append("static void touch(void) {} static u16 ticks; static void tick(void) { ticks++; }")
        out.append("static u16 tick_count(void) { u16 t = ticks; ticks = 0; return t; }")
        out.append("static void lcopy(void *d, void *s, u16 n) { memcpy(d, s, n); }")
    else:
        out.append("extern void report(i32 v);")
        out.append("extern i16 keep(i16 x); extern i32 keep32(i32 x); extern void touch(void); extern void tick(void);")
        out.append("extern u16 tick_count(void); extern void lcopy(void FAR *d, void FAR *s, u16 n);")
    if any(a.elem == F80 or (isinstance(a.elem, Struct) and F80 in dict(a.elem.fields).values()) for c in cases for a in c.arrays):
        out.append("#define F80")
    out += structs(cases)
    for case in cases:
        e = Emitter(case, host)
        out += storage(case, e)
        out += e.function()
    out += [FILLX, DIG]
    for case in cases:
        e = Emitter(case, host)
        out += helpers(case, e)
        out.append(f"static void run_{case.symbol}(void)\n{{")
        out.append("    i32 r;")
        ticks = any(isinstance(one, CallS) and one.name == "tick" for one in walk(case.body))
        for at in plans[case.name]:
            inp = case.inputs[at]
            fills = dict(inp.fills)
            for array in case.arrays:
                if array.alias:
                    continue
                f = fills[array.name]
                out.append(f"    fill_{e.gname(array)}({f.seed}u, {f.lo}L, {f.span}L, {f.step}L);")
            for name, at, value in inp.pokes:
                array = case.array(name)
                out.append(f"    (({ctype(array.elem)} HUGE *){e.gname(array)})[{at}] = {e.const(value, array.elem)};")
            args = [e.gname(backing(case, one)) for one in case.arrays if one.where == "param"]
            args += [e.const(value, kind) for (_, kind), value in zip(case.params, inp.args)]
            if ticks:
                out.append("    tick_count();")
            out.append(f"    r = (i32){case.symbol}({', '.join(args)});")
            out.append("    report(r);")
            for name in written_arrays(case):
                out.append(f"    report(dig_{e.gname(backing(case, case.array(name)))}());")
            if ticks:
                out.append("    report((i32)tick_count());")
        out.append("}")
    out.append("int main(void)\n{")
    out += [f"    run_{case.symbol}();" for case in cases]
    out.append("    return 0;\n}")
    return "\n".join(out) + "\n"


def expressible(case: Case) -> str | None:
    try:
        e = Emitter(case)
        e.function()
        return None
    except NotC as why:
        return str(why)


LIBRARY = {
    "llrm": PRELUDE[False],
    "ow": PRELUDE[False],
    # gcc-ia16 has __far but no __huge; msp430 has neither
    "gcc": PRELUDE[False].replace("#define HUGE __huge", "#define HUGE __far"),
    "llvm": PRELUDE[True].replace("#include <stdio.h>\n", "").replace("#include <string.h>\n", "")
    .replace("#include <stdint.h>\n", "typedef signed char int8_t; typedef unsigned char uint8_t;\n"
             "typedef short int16_t; typedef unsigned short uint16_t; typedef long int32_t; typedef unsigned long uint32_t;\n"),
}


def library(cases: list[Case], flavor: str) -> str:
    """The cases alone, for a reference compiler: no driver."""
    out = [LIBRARY[flavor]]
    out.append("extern i16 keep(i16 x); extern i32 keep32(i32 x); extern void touch(void); extern void tick(void);")
    out.append("extern void lcopy(void FAR *d, void FAR *s, u16 n);")
    if any(a.elem == F80 or (isinstance(a.elem, Struct) and F80 in dict(a.elem.fields).values()) for c in cases for a in c.arrays):
        out.append("#define F80")
    out += structs(cases)
    for case in cases:
        e = Emitter(case)
        out += storage(case, e)
        out += e.function()
    return "\n".join(out) + "\n"
