#!/usr/bin/env python3
"""Check a lalrpop grammar with bison, which builds true LALR(1) tables.

lalrpop 0.23 builds tables with its lane-table algorithm whatever `#[LALR]`
says, so "no conflicts" from lalrpop means LR(1). This converts the grammar
(macros expanded, actions dropped) and runs bison on it.

    lalrpop2bison.py src/nib.lalrpop
"""

import itertools
import re
import subprocess
import sys
import tempfile
from pathlib import Path


class Source:
    def __init__(self, text):
        out, i, n = [], 0, len(text)
        while i < n:  # drop comments, but not the `//` inside a string
            if text[i] == '"':
                j = i + 1
                while text[j] != '"':
                    j += 2 if text[j] == "\\" else 1
                out.append(text[i : j + 1])
                i = j + 1
            elif text.startswith("//", i):
                while i < n and text[i] != "\n":
                    i += 1
            else:
                out.append(text[i])
                i += 1
        text = "".join(out)
        self.text, self.at = text, 0

    def ws(self):
        while self.at < len(self.text) and self.text[self.at].isspace():
            self.at += 1

    def peek(self, s=None):
        self.ws()
        if s is None:
            return self.text[self.at : self.at + 1]
        return self.text.startswith(s, self.at)

    def take(self, s):
        self.ws()
        assert self.text.startswith(s, self.at), (s, self.text[self.at : self.at + 60])
        self.at += len(s)

    def ident(self):
        self.ws()
        m = re.compile(r"[A-Za-z_]\w*").match(self.text, self.at)
        if not m:
            return None
        self.at = m.end()
        return m.group(0)

    def skip_balanced(self, stop):
        """Skip Rust code up to a depth-0 character in `stop`; leave it unread."""
        depth = 0
        while self.at < len(self.text):
            c = self.text[self.at]
            if depth == 0 and c in stop:
                return
            if c == '"':
                self.at += 1
                while self.text[self.at] != '"':
                    self.at += 2 if self.text[self.at] == "\\" else 1
            elif c == "'" and self.text[self.at + 2 : self.at + 3] == "'":
                self.at += 2
            elif c in "([{":
                depth += 1
            elif c in ")]}":
                if depth == 0 and c in stop:
                    return
                depth -= 1
            elif depth == 0 and c in stop:
                return
            self.at += 1


def parse_symbol(src, params):
    src.ws()
    c = src.peek()
    if c == "<":  # <x:Sym> <Sym> <mut x:Sym>
        src.take("<")
        save = src.at
        m = re.compile(r"(?:mut\s+)?\w+\s*:(?!:)").match(src.text, src.at)
        if m:
            src.at = m.end()
        sym = parse_symbol(src, params)
        src.take(">")
        return sym
    if c == "(":
        src.take("(")
        alts = parse_alts(src, params, ")")
        src.take(")")
        sym = ("group", alts)
    elif c == '"':
        end = src.text.index('"', src.at + 1)
        sym = ("t", src.text[src.at + 1 : end])
        src.at = end + 1
    elif c == "@":
        src.at += 2
        return ("eps",)
    else:
        name = src.ident()
        assert name, src.text[src.at : src.at + 60]
        args = []
        if src.text[src.at : src.at + 1] == "<":
            src.take("<")
            while True:
                args.append(parse_symbol(src, params))
                if src.peek(","):
                    src.take(",")
                    continue
                break
            src.take(">")
        sym = ("p", name) if name in params and not args else ("n", name, tuple(args))
    while src.text[src.at : src.at + 1] in ("?", "*", "+"):
        sym = ({"?": "opt", "*": "star", "+": "plus"}[src.text[src.at]], sym)
        src.at += 1
    return sym


def parse_alts(src, params, closing):
    """Alternatives up to `closing` ('}' or ')' or ';')."""
    alts = []
    while True:
        seq = []
        while True:
            src.ws()
            c = src.peek()
            if c in (",", closing, ";") or src.peek("=>") or c == "":
                break
            seq.append(parse_symbol(src, params))
        if src.peek("=>"):
            src.take("=>")
            src.skip_balanced(",;" + closing if closing != ";" else ";")
        alts.append([s for s in seq if s != ("eps",)])
        if src.peek(","):
            src.take(",")
            src.ws()
            if src.peek() == closing:
                break
            continue
        break
    return alts


def parse_grammar(text):
    src = Source(text)
    rules, start = {}, []
    # skip header: use ...; #[...]; grammar; extern { ... }
    while True:
        src.ws()
        if src.peek("use "):
            src.skip_balanced(";")
            src.take(";")
        elif src.peek("#["):
            src.skip_balanced("]")
            src.take("]")
        elif src.peek("grammar"):
            src.skip_balanced(";")
            src.take(";")
        elif src.peek("extern"):
            src.take("extern")
            src.ws()
            src.skip_balanced("{")
            src.take("{")
            src.skip_balanced("}")
            src.take("}")
        else:
            break
    while True:
        src.ws()
        if src.at >= len(src.text):
            break
        public = src.peek("pub ")
        if public:
            src.take("pub")
        name = src.ident()
        params = []
        if src.peek("<"):
            src.take("<")
            while True:
                params.append(src.ident())
                if src.peek(","):
                    src.take(",")
                    continue
                break
            src.take(">")
        src.take(":")
        # type: up to '=' not followed by '>'
        while not (src.text[src.at] == "=" and src.text[src.at + 1] != ">"):
            src.at += 1
        src.take("=")
        if src.peek("{"):
            src.take("{")
            alts = parse_alts(src, set(params), "}")
            src.take("}")
            src.ws()
            if src.peek(";"):
                src.take(";")
        else:
            alts = parse_alts(src, set(params), ";")
            src.take(";")
        rules[name] = (params, alts)
        if public:
            start.append(name)
    return rules, start


def inline(alt):
    """lalrpop inlines `x?` and the empty case of `x*` into the production
    that uses them, so a decision on the first token of x is not a conflict."""
    for i, sym in enumerate(alt):
        if sym[0] == "opt":
            return inline(alt[:i] + [sym[1]] + alt[i + 1 :]) + inline(alt[:i] + alt[i + 1 :])
        if sym[0] == "star":
            return inline(alt[:i] + [("plus", sym[1])] + alt[i + 1 :]) + inline(alt[:i] + alt[i + 1 :])
    return [alt]


class Emitter:
    def __init__(self, rules):
        self.rules, self.out, self.done, self.terms = rules, [], {}, {}
        self.count = itertools.count()

    def term(self, s):
        if s not in self.terms:
            self.terms[s] = "T%d_%s" % (len(self.terms), re.sub(r"\W", "", s)[:12])
        return self.terms[s]

    def subst(self, sym, env):
        if sym[0] == "p":
            return env[sym[1]]
        if sym[0] == "n":
            return ("n", sym[1], tuple(self.subst(a, env) for a in sym[2]))
        if sym[0] == "group":
            return ("group", [[self.subst(s, env) for s in alt] for alt in sym[1]])
        if sym[0] in ("opt", "star", "plus"):
            return (sym[0], self.subst(sym[1], env))
        return sym

    def name(self, sym):
        """Bison symbol for sym, emitting its rules the first time."""
        if sym[0] == "t" or (sym[0] == "n" and sym[1] not in self.rules):
            return self.term(sym[1])  # a bare name no rule defines is an extern terminal
        key = repr(sym)
        if key in self.done:
            return self.done[key]
        n = "n%d_%s" % (next(self.count), sym[1] if sym[0] == "n" else sym[0])
        self.done[key] = n
        if sym[0] == "n":
            params, alts = self.rules[sym[1]]
            env = dict(zip(params, sym[2]))
            body = [[self.subst(s, env) for s in alt] for alt in alts]
        elif sym[0] == "group":
            body = sym[1]
        elif sym[0] == "opt":
            body = [[], [sym[1]]]
        elif sym[0] == "star":
            body = [[], [("raw", n), sym[1]]]
        elif sym[0] == "plus":
            body = [[sym[1]], [("raw", n), sym[1]]]
        else:
            raise ValueError(sym)
        lines = []
        for alt in itertools.chain.from_iterable(inline(a) for a in body):
            lines.append(" ".join(s[1] if s[0] == "raw" else self.name(s) for s in alt))
        self.out.append("%s: %s ;" % (n, "\n  | ".join(lines)))
        return n


def main():
    rules, start = parse_grammar(Path(sys.argv[1]).read_text())
    em = Emitter(rules)
    starts = [em.name(("n", s, ())) for s in start]
    body = "\n".join(em.out)
    tokens = "\n".join("%%token %s" % t for t in em.terms.values())
    y = "%%define lr.type lalr\n%%start %s\n%s\n%%%%\n%s\n" % (starts[0], tokens, body)
    with tempfile.TemporaryDirectory() as d:
        Path(d, "g.y").write_text(y)
        r = subprocess.run(["bison", "-Wnone", "-Wconflicts-sr", "-Wconflicts-rr"] + (["-Wcounterexamples"] if "-v" in sys.argv else []) + [ "-o", d + "/g.c", d + "/g.y"], capture_output=True, text=True)
        print(Path(sys.argv[1]).name, "terminals", len(em.terms), "rules", len(em.out))
        print((r.stderr.strip() or "no conflicts")[:2000])


main()
