"""
A case as Nib, for llrm-nib; the HIR executor (llrm-run) checks the oracle.

Nib wraps every integer operation, promotes operands narrower than i16 as C
does, and forbids mixing signed with unsigned. Array parameters are views
(`&[T]`), which carry their length. Foreign calls sit in `unsafe:`.
"""

from __future__ import annotations

from spec import (
    Int, Float, Struct, Ptr, Case, Array, Const, Var, Bin, Neg, Cast, Cmp, Not, Logic, Load, AddrOf, PtrAdd,
    Deref, Len, CallE, Assign, For, While, DoWhile, If, Break, Continue, Return, CallS, OPAQUE, I16, I32,
    written_arrays, walk, stmt_exprs, exprs,
)


class NotNib(Exception):
    pass


RESERVED = {"fn", "let", "mut", "var", "if", "else", "while", "for", "in", "loop", "break", "continue", "return",
            "match", "struct", "type", "and", "or", "not", "true", "false", "with", "unsafe", "print", "is", "as",
            "enum", "import", "pub", "const", "self", "main"}


def ntype(kind) -> str:
    if isinstance(kind, Int):
        return kind.name
    if isinstance(kind, Struct):
        return kind.name
    raise NotNib(f"no Nib type for {getattr(kind, 'name', kind)}")


def ident(name: str) -> str:
    return name + "_" if name in RESERVED else name


class Emitter:
    def __init__(self, case: Case, prefix: str = ""):
        self.case = case
        self.types = case.types()
        self.prefix = prefix
        self.loops: list[str | None] = []  # innermost last: what continue runs first
        self.ranged = self._ranged()

    def gname(self, array: Array) -> str:
        return f"{self.prefix}{self.case.symbol}_{array.name}"

    def type_of(self, e):
        from oracle import Machine

        return Machine(self.case, "nib").type_of(e)

    def _ranged(self) -> set[str]:
        """Counters a Nib range can carry: `for v in a..b`, step 1, never
        touched or read outside that loop."""
        out = set()
        stmts = walk(self.case.body)
        for s in stmts:
            if not (isinstance(s, For) and s.cond == "<" and isinstance(s.step, Const) and s.step.value == 1):
                continue
            inside = {id(one) for one in walk(s.body)}
            others = [one for one in stmts if one is not s and id(one) not in inside]
            used = any(isinstance(e, Var) and e.name == s.var for one in others for e in stmt_exprs(one))
            assigned = any(isinstance(one, Assign) and one.place == Var(s.var) for one in stmts)
            another = sum(1 for one in stmts if isinstance(one, For) and one.var == s.var) > 1
            if not (used or assigned or another):
                out.add(s.var)
        return out

    def const(self, value: int, kind) -> str:
        if isinstance(kind, Float):
            raise NotNib("a float")
        return f"{kind.name}({value})" if value < 0 else str(value)

    def expr(self, e, operand: bool = True) -> str:
        if isinstance(e, Const):
            return self.const(e.value, e.type)
        if isinstance(e, Var):
            if isinstance(self.types[e.name], Ptr):
                raise NotNib("a raw pointer walk")
            return ident(e.name)
        if isinstance(e, Bin):
            kind = self.type_of(e.a)
            ntype(kind)
            a, b = self.expr(e.a), self.expr(e.b)
            if e.op in ("min", "max"):
                test = "<" if e.op == "min" else ">"
                text = f"({a} {test} {b} ? {a} : {b})"
            else:
                text = f"({a} {'//' if e.op == '/' else e.op} {b})"
            if kind.bits < 16 and (operand or e.op not in ("min", "max")):
                return f"{kind.name}{text}"
            return text
        if isinstance(e, Neg):
            kind = self.type_of(e.a)
            return f"{kind.name}(-{self.expr(e.a)})" if kind.bits < 16 else f"(-{self.expr(e.a)})"
        if isinstance(e, Cast):
            ntype(self.type_of(e.a))
            return f"{ntype(e.type)}({self.expr(e.a, False)})"
        if isinstance(e, Cmp):
            return f"({self.expr(e.a)} {e.op} {self.expr(e.b)})"
        if isinstance(e, Not):
            return f"(!{self.expr(e.a)})"
        if isinstance(e, Logic):
            return f"({self.expr(e.a)} {'&&' if e.op == 'and' else '||'} {self.expr(e.b)})"
        if isinstance(e, Load):
            text = f"{self.array_ref(e.array)}[{', '.join(self.expr(one, False) for one in e.index)}]"
            return text + (f".{ident(e.field)}" if e.field else "")
        if isinstance(e, (AddrOf, PtrAdd, Deref)):
            raise NotNib("a raw pointer walk")
        if isinstance(e, Len):
            array = self.case.array(e.array)
            field = "len" if len(array.dims) == 1 else f"dim[{e.dim}]"
            return f"i16({self.array_ref(e.array)}.{field})"
        if isinstance(e, CallE):
            return f"{e.name}({', '.join(self.expr(one, False) for one in e.args)})"
        raise NotNib(f"no Nib for {e}")

    def array_ref(self, name: str) -> str:
        array = self.case.array(name)
        ntype(array.elem)
        if array.bytes > 30000:
            raise NotNib("a module array this large does not fit DGROUP")
        return self.gname(array) if array.where == "global" else ident(name)

    def unsafe(self, s) -> bool:
        return any(isinstance(e, CallE) for e in stmt_exprs(s)) or isinstance(s, CallS)

    def block(self, stmts, depth: int) -> list[str]:
        out = []
        pad = "    " * depth
        for s in stmts:
            lines = self.stmt(s, depth)
            if self.unsafe(s) and not isinstance(s, (For, While, DoWhile, If)):
                lines = [pad + "unsafe:", *("    " + one for one in lines)]
            out += lines
        if not out:
            raise NotNib("an empty block")
        return out

    def stmt(self, s, depth: int) -> list[str]:
        pad = "    " * depth
        if isinstance(s, Assign):
            place, value = self.expr(s.place, False), s.value
            if isinstance(value, Bin) and value.a == s.place and value.op in ("+", "-", "*", "&", "|", "^"):
                return [pad + f"{place} {value.op}= {self.expr(value.b, False)}"]
            return [pad + f"{place} = {self.expr(value, False)}"]
        if isinstance(s, For):
            var = ident(s.var)
            if s.var in self.ranged:
                self.loops.append(None)
                body = self.block(s.body, depth + 1)
                self.loops.pop()
                return [pad + f"for {var} in {self.expr(s.start, False)}..{self.expr(s.end, False)}:", *body]
            step = self.step_text(s)
            self.loops.append(("step", step))
            body = self.block(s.body, depth + 1)
            self.loops.pop()
            return [pad + f"{var} = {self.expr(s.start, False)}",
                    pad + f"while {var} {s.cond} {self.expr(s.end, False)}:", *body, pad + "    " + step]
        if isinstance(s, While):
            self.loops.append(None)
            body = self.block(s.body, depth + 1)
            self.loops.pop()
            return [pad + f"while {self.expr(s.cond, False)}:", *body]
        if isinstance(s, DoWhile):
            cond = self.expr(s.cond)
            self.loops.append(("do", cond))
            body = self.block(s.body, depth + 1)
            self.loops.pop()
            return [pad + "loop:", *body, pad + f"    if !{cond}:", pad + "        break"]
        if isinstance(s, If):
            out = [pad + f"if {self.expr(s.cond, False)}:", *self.block(s.then, depth + 1)]
            if s.orelse:
                out += [pad + "else:", *self.block(s.orelse, depth + 1)]
            return out
        if isinstance(s, Break):
            return [pad + "break"]
        if isinstance(s, Continue):
            first = self.loops[-1]
            if first is None:
                return [pad + "continue"]
            kind, text = first
            if kind == "step":
                return [pad + text, pad + "continue"]
            return [pad + f"if !{text}:", pad + "    break", pad + "continue"]
        if isinstance(s, Return):
            return [pad + f"return {self.expr(s.value, False)}"]
        if isinstance(s, CallS):
            return [pad + f"{s.name}({', '.join(self.expr(one, False) for one in s.args)})"]
        raise NotNib(f"no Nib for {s}")

    def step_text(self, s: For) -> str:
        var = ident(s.var)
        if isinstance(s.step, Const) and s.step.value < 0:
            return f"{var} -= {-s.step.value}"
        return f"{var} += {self.expr(s.step, False)}"

    def params(self) -> list[str]:
        case = self.case
        written = set(written_arrays(case))
        out = []
        for a in case.arrays:
            if a.where != "param":
                continue
            rank = "" if len(a.dims) == 1 else f", {len(a.dims)}"
            out.append(f"{ident(a.name)}: &{'mut ' if a.name in written else ''}[{ntype(a.elem)}{rank}]")
        for name, kind in case.params:
            if isinstance(kind, Ptr):
                raise NotNib("a pointer parameter")
            out.append(f"{ident(name)}: {ntype(kind)}")
        return out

    def function(self) -> list[str]:
        case = self.case
        for array in case.arrays:
            self.array_ref(array.name)
        if sum(a.bytes for a in case.arrays if a.where == "local") > 2048:
            raise NotNib("local arrays past half of Nib's 4K stack")
        out = [f"fn {case.symbol}({', '.join(self.params())}) -> {ntype(case.ret)}:"]
        for array in case.arrays:
            if array.where == "local":
                out.append(f"    let mut {ident(array.name)}: {_declared(array)} = {_zeros(array)}")
        for name, kind in case.locals:
            if isinstance(kind, Ptr):
                raise NotNib("a raw pointer walk")
            if name in self.ranged:
                continue
            out.append(f"    let mut {ident(name)}: {ntype(kind)} = 0")
        for array in case.arrays:
            if array.where == "local":
                elem = ntype(array.elem)
                out += ["    unsafe:",
                        f"        let d: *far mut {elem} = &mut {ident(array.name)}",
                        f"        let s: *far {elem} = &{self.gname(array)}",
                        f"        lcopy(d.cast[u8](), s.cast[u8](), {array.bytes})"]
        out += self.block(case.body, 1)
        return out


def _declared(array: Array) -> str:
    return f"{ntype(array.elem)}[{', '.join(map(str, array.dims))}]"


def _zero_elem(elem) -> str:
    if isinstance(elem, Struct):
        return f"{elem.name}({', '.join(f'{n}=0' for n, _ in elem.fields)})"
    return "0"


def _zeros(array: Array) -> str:
    text = f"[{_zero_elem(array.elem)}] * {array.dims[-1]}"
    for extent in reversed(array.dims[:-1]):
        text = f"[{text}] * {extent}"
    return text


def fields_of(elem):
    return list(elem.fields) if isinstance(elem, Struct) else [(None, elem)]


def helper_name(prefix: str, elem, rank: int) -> str:
    return f"{prefix}_{ntype(elem).lower()}_{rank}"


def helpers(elem, rank: int) -> list[str]:
    """Fill and digest, as oracle.fill_value and oracle.digest define them."""
    t = ntype(elem)
    view = f"[{t}]" if rank == 1 else f"[{t}, {rank}]"
    index = ", ".join(f"k{d}" for d in range(rank))
    fill = [f"fn {helper_name('fill', elem, rank)}(a: &mut {view}, seed: i32, lo: i32, span: i32, step: i32) -> void:",
            "    let mut k: i32 = 0"]
    dig = [f"fn {helper_name('dig', elem, rank)}(a: &{view}) -> i32:", "    let mut d: i32 = 0"]
    heads = []
    for d in range(rank):
        extent = "a.len" if rank == 1 else f"a.dim[{d}]"
        heads.append("    " * (d + 1) + f"for k{d} in 0..i32({extent}):")
    pad = "    " * (rank + 1)
    fill += heads
    for f, (name, kind) in enumerate(fields_of(elem)):
        place = f"a[{index}]" + (f".{ident(name)}" if name else "")
        k = kind.name
        fill += [
            pad + f"let s{f}: i32 = seed + {101 * f}",
            pad + f"let x{f}: i32 = (k * 7919 + s{f}) % 65521",
            pad + "if step != 0:",
            pad + f"    {place} = {k}(lo + k * step)",
            pad + "else:",
            pad + "    if span != 0:",
            pad + f"        {place} = {k}(lo + x{f} % span)",
            pad + "    else:",
        ]
        if kind.bits == 32:
            fill.append(pad + f"        {place} = {k}((x{f} - 32760) * 65536 + (k * 7919 + s{f} + 17) % 65521)")
        elif kind.bits == 16:
            fill.append(pad + f"        {place} = {k}(x{f})")
        else:
            fill.append(pad + f"        {place} = {k}(x{f} & 255)")
    fill.append(pad + "k += 1")
    dig += heads
    for f, (name, kind) in enumerate(fields_of(elem)):
        place = f"a[{index}]" + (f".{ident(name)}" if name else "")
        if kind.bits == 32:
            dig += [pad + f"let w{f}: u32 = u32({place})",
                    pad + f"d = (d * 31 + i32(w{f} & 65535)) % 65521",
                    pad + f"d = (d * 31 + i32(w{f} >> 16)) % 65521"]
        else:
            mask = 255 if kind.bits == 8 else 65535
            wide = "u16" if kind.signed else ""
            value = f"i32({wide}({place}))" if wide and kind.bits == 16 else f"i32({place})"
            dig.append(pad + f"d = (d * 31 + ({value} & {mask})) % 65521")
    dig.append("    return d")
    return fill + dig


def driver(cases: list[Case], plans: dict) -> str:
    out, structs, used = [], {}, {}
    externs = [
        '@extern("cdecl16")\nfn keep(x: i16) -> i16', '@extern("cdecl16")\nfn keep32(x: i32) -> i32',
        '@extern("cdecl16")\nfn touch() -> void', '@extern("cdecl16")\nfn tick() -> void',
        '@extern("cdecl16")\nfn tick_count() -> u16',
        '@extern("cdecl16")\nfn lcopy(d: *far mut u8, s: *far u8, n: u16) -> void',
    ]
    for case in cases:
        for array in case.arrays:
            if isinstance(array.elem, Struct):
                structs[array.elem.name] = array.elem
            used[(array.elem, len(array.dims))] = True
    for struct in structs.values():
        out.append('@repr("c16", pack=1)')
        out.append(f"struct {struct.name}:")
        out += [f"    mut {ident(n)}: {ntype(k)}" for n, k in struct.fields]
    out += externs
    for case in cases:
        e = Emitter(case)
        for array in case.arrays:
            out.append(f"var {e.gname(array)}: {_declared(array)} = {_zeros(array)}")
    for case in cases:
        out += Emitter(case).function()
    for elem, rank in used:
        out += helpers(elem, rank)
    main = []
    for case in cases:
        e = Emitter(case)
        main += [f"fn run_{case.symbol}() -> void:", "    let mut r: i32 = 0"]
        ticks = any(isinstance(one, CallS) and one.name == "tick" for one in walk(case.body))
        for at in plans[case.name]:
            inp = case.inputs[at]
            fills = dict(inp.fills)
            for array in case.arrays:
                f = fills[array.name]
                main.append(f"    {helper_name('fill', array.elem, len(array.dims))}(&mut {e.gname(array)}, {f.seed}, {f.lo}, {f.span}, {f.step})")
            for name, at, value in inp.pokes:
                array = case.array(name)
                index, rest = [], at
                for extent in reversed(array.dims):
                    index.insert(0, rest % extent)
                    rest //= extent
                main.append(f"    {e.gname(array)}[{', '.join(map(str, index))}] = {e.const(value, array.elem)}")
            args = [f"&{'mut ' if a.name in written_arrays(case) else ''}{e.gname(a)}" for a in case.arrays if a.where == "param"]
            args += [e.const(value, kind) for (_, kind), value in zip(case.params, inp.args)]
            if ticks:
                main += ["    unsafe:", "        tick_count()"]
            main.append(f"    r = i32({case.symbol}({', '.join(args)}))")
            main.append("    print(r)")
            for name in written_arrays(case):
                array = case.array(name)
                main.append(f"    print({helper_name('dig', array.elem, len(array.dims))}(&{e.gname(array)}))")
            if ticks:
                main += ["    unsafe:", "        print(i32(tick_count()))"]
    main.append("fn main() -> i16:")
    main += [f"    run_{case.symbol}()" for case in cases]
    main.append("    return 0")
    return "\n".join(out + main) + "\n"


def expressible(case: Case) -> str | None:
    try:
        Emitter(case).function()
        return None
    except NotNib as why:
        return str(why)
