#!/usr/bin/env python3
"""Convert the recovered QBasic grammar (`qbasbnf.prs`) into lalrpop.

The `Statements:` and `Functions:` sections and every internal nonterminal
become rules that build a generic syntax tree. EMIT, MARK and the other
p-code directives are dropped. The `EXTERNAL` nonterminals, and the few
internal ones listed in `OVERRIDDEN`, are written by hand in `qb_hand.part`:
that is where the parser's context-sensitive choices become deferred nodes.

    prs2lalrpop.py ../qbfront/grammar/qbasbnf.prs > src/qb.lalrpop
    (also writes src/qb_kw.rs)
"""

import os
import re
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).parent

# Internal nonterminals the hand-written part defines instead.
OVERRIDDEN = {"printItem", "printList", "printUsingItem", "optFilenum", "coordStep", "coord2Step"}
# Statements the hand-written part defines instead (all productions of the keyword).
OVERRIDDEN_STATEMENTS = {
    "tkDATA", "tkREM", "tkIF", "tkELSEIF", "tkELSE", "tkVIEW", "tkPAINT", "tkRUN", "tkRESUME", "tkOPTION", "tkON", "tkKEY",
    "tkDECLARE", "tkFUNCTION",
}
# One production of a keyword that has several: (keyword, index).
OVERRIDDEN_PRODUCTIONS = {("tkLINE", 0)}
# Directives that consume nothing.
ZERO_WIDTH = re.compile(r"^(EMIT|MARK)\(|^(EMITFFFF|ACTIONid\w+|ErrIfNot1st|EndPrint|EndPrintExp)$")


def strip_comments(text):
    return re.sub(r"/\*.*?\*/", " ", text, flags=re.S)


def sections(text):
    names = ["TOKENS", "Statements", "Functions", "NonTerminals"]
    marks = sorted((text.index(name + ":"), name) for name in names)
    return {name: text[at + len(name) + 1 : (marks[i + 1][0] if i + 1 < len(marks) else len(text))]
            for i, (at, name) in enumerate(marks)}


def tokens_of(section):
    return re.findall(r'\b(tk\w+)\s*\(\s*"', section)


LEX = re.compile(r"""
    (?P<action><[^<>;]*>)           # <CgCall(...)> after a production
  | (?P<dir>(?:EMIT|MARK)\([^)]*\))
  | (?P<id>[A-Za-z_]\w*)
  | (?P<punct>[()\[\]{}|;:])
""", re.X)


def lex(text):
    text = re.sub(r"<[^<>;]*>", " ", text)  # post-actions
    return [m.group(0) for m in LEX.finditer(text) if m.lastgroup != "action"]


class Parser:
    """Productions: `tkX body ;` in statements, `Name: body ;` in nonterminals."""

    def __init__(self, tokens):
        self.tokens = tokens
        self.at = 0

    def peek(self):
        return self.tokens[self.at] if self.at < len(self.tokens) else None

    def take(self):
        token = self.tokens[self.at]
        self.at += 1
        return token

    def alternatives(self, closing):
        return [self.sequence(closing)]

    def sequence(self, closing):
        """A sequence of items. `a (b) | (c) | (d)` is a, then one of b, c, d:
        the recovered grammar's `|` joins the items on either side of it."""
        items = []
        while self.peek() is not None and self.peek() not in (";", ")", "]", "}"):
            alts = [[self.item()]]
            while self.peek() == "|":
                self.take()
                alts.append([self.item()])
            items.append(alts[0][0] if len(alts) == 1 else ("group", alts))
        return items

    def item(self):
        token = self.take()
        if token == "(":
            alts = self.alternatives(")")
            assert self.take() == ")"
            return ("group", alts)
        if token == "[":
            alts = self.alternatives("]")
            assert self.take() == "]"
            return ("opt", alts)
        if token == "{":
            alts = self.alternatives("}")
            assert self.take() == "}"
            return ("rep", alts)
        if ZERO_WIDTH.match(token):
            return ("zero",)
        return ("sym", token)


def productions(section, header):
    """Yield (head, alternatives) per `;`-terminated production."""
    parser = Parser(lex(section))
    out = []
    while parser.peek() is not None:
        if header:
            head = parser.take()
            assert parser.take() == ":", head
        else:
            head = parser.take()
        if parser.peek() == "EXTERNAL":
            while parser.take() != ";":
                pass
            out.append((head, None))
            continue
        alts = parser.alternatives(None)
        assert parser.take() == ";", (head, parser.peek())
        out.append((head, alts))
    return out


class Emitter:
    def __init__(self, terminals, internal, external):
        self.terminals = terminals
        self.internal = internal
        self.external = external
        self.helpers = []
        self.count = 0

    def nonterminal(self, name):
        return f"N_{name}"

    def fresh(self, owner):
        self.count += 1
        return f"G{self.count}_{owner}"

    def element(self, item, owner):
        """(lalrpop symbol, is_terminal) of an item; None when it is zero-width."""
        kind = item[0]
        if kind == "zero":
            return None
        if kind == "sym":
            name = item[1]
            if name in self.terminals:
                return (f'"{name}"', True)
            assert name in self.internal or name in self.external, f"unknown symbol {name} in {owner}"
            return (self.nonterminal(name), False)
        name = self.fresh(owner)
        alts = item[1]
        if kind == "group":
            self.helper(name, alts, owner)
        elif kind == "opt":
            self.helper(name, alts + [[]], owner)
        else:
            inner = self.fresh(owner)
            self.helper(inner, alts, owner)
            self.helpers.append(
                f"{name}: Tree = {{\n    => Tree::List(vec![]),\n    <r:{name}> <x:{inner}> => Tree::push(r, x),\n}};\n"
            )
        return (name, False)

    def alternative(self, seq, owner, label=None):
        parts = []
        for item in seq:
            element = self.element(item, owner)
            if element:
                parts.append(element)
        binds = " ".join(f"<e{i}:{sym}>" for i, (sym, _) in enumerate(parts))
        values = ", ".join(f"Tree::tok(e{i})" if term else f"e{i}" for i, (_, term) in enumerate(parts))
        if label:
            return f"    {binds} => Tree::node(\"{label}\", vec![{values}]),"
        return f"    {binds} => Tree::List(vec![{values}]),"

    def helper(self, name, alts, owner):
        body = "\n".join(self.alternative(seq, owner) for seq in alts)
        self.helpers.append(f"{name}: Tree = {{\n{body}\n}};\n")


CAPTURES = {"label": "N_LabLn", "identifier": "Ident", "string": "StrLit"}
TAILS = {"function_signature": "FunctionTail", "letter_ranges": "LetterRanges"}


def extension_rules(path):
    """The dialect additions of `dialect-extensions.toml`, one alternative each."""
    # Extension keywords share one terminal, so patterns of the same shape
    # are one alternative: the keyword's spelling says which, later.
    shapes = {}
    for extension in tomllib.loads(path.read_text())["extension"]:
        parts = []
        for element in extension["pattern"].split(", "):
            kind, _, value = element.partition(":")
            parts.append({"token": f'"{value}"', "keyword": '"ext"', "capture": CAPTURES.get(value)}[kind])
        if "tail" in extension:
            parts.append(TAILS[extension["tail"]])
        shapes.setdefault(tuple(parts), []).append(extension["action"])
    alternatives = []
    for parts, actions in shapes.items():
        binds = " ".join(f"<e{i}:{p}>" for i, p in enumerate(parts))
        values = ", ".join(
            "Tree::at_ext(e%d)" % i if p == '"ext"' else (f"Tree::tok(e{i})" if p.startswith('"') else f"e{i}")
            for i, p in enumerate(parts)
        )
        alternatives.append(f'    {binds} => Tree::node("{"|".join(actions)}", vec![{values}]),')
    return "ExtStatement: Tree = {\n" + "\n".join(alternatives) + "\n};\n"


def main():
    prs = Path(sys.argv[1]).read_text()
    text = strip_comments(prs)
    parts = sections(text)
    names = tokens_of(parts["TOKENS"])
    statements = productions(parts["Statements"], header=False)
    functions = productions(parts["Functions"], header=False)
    nonterminals = productions(parts["NonTerminals"], header=True)

    external = {head for head, alts in nonterminals if alts is None}
    internal = {head for head, alts in nonterminals if alts is not None and head not in OVERRIDDEN}
    hand = Path(HERE / "src" / "qb_hand.part").read_text()

    em = Emitter(set(names), internal, external | OVERRIDDEN)
    rules = []
    for head, alts in nonterminals:
        if alts is None or head in OVERRIDDEN:
            continue
        body = "\n".join(em.alternative(seq, head) for seq in alts)
        rules.append(f"N_{head}: Tree = {{\n{body}\n}};\n")

    def keyword_rules(prods, rule, skip):
        lines = []
        seen = {}
        for head, alts in prods:
            index = seen[head] = seen.get(head, -1) + 1
            if head in skip or (head, index) in OVERRIDDEN_PRODUCTIONS:
                continue
            for seq in alts:
                lines.append(em.alternative([("sym", head)] + seq, head, label=head[2:]))
        return f"{rule}: Tree = {{\n" + "\n".join(lines) + "\n};\n"

    only = os.environ.get("QB_ONLY")  # bisecting: keep just these statements
    skip_statements = OVERRIDDEN_STATEMENTS | ({h for h, _ in statements} - set(only.split(",")) if only else set())
    skip_functions = {h for h, _ in functions} - set(only.split(",")) if only else set()
    rules.append(keyword_rules(statements, "AutoStatement", skip_statements))
    rules.append(keyword_rules(functions, "AutoFunction", skip_functions))

    text_out = "\n".join(rules) + "\n".join(em.helpers)
    defined = set(re.findall(r"^N_(\w+)\s*:", hand + text_out, re.M))
    referenced = set(re.findall(r"\bN_(\w+)", hand + text_out))
    assert referenced <= defined, f"undefined: {sorted(referenced - defined)}"

    rules_text = extension_rules(HERE.parent / "qbfront" / "grammar" / "dialect-extensions.toml")
    function_only = [h for h, _ in functions if h not in {s for s, _ in statements}]
    spellings = dict(re.findall(r'\b(tk\w+)\s*\(\s*"([^"]*)"', parts["TOKENS"]))
    words = [n for n in names if n not in {"tkELSE", "tkELSEIF"} and re.fullmatch(r"[A-Za-z]+[$]?", spellings[n])]
    rules_text += "Keyword: Tree = {\n" + "".join(f'    <t:"{h}"> => Tree::tok(t),\n' for h in words) + "};\n"
    rules_text += "FnKeyword: Tree = {\n" + "".join(f'    <t:"{h}"> => Tree::tok(t),\n' for h in dict.fromkeys(function_only)) + "};\n"
    text_out += "\n" + rules_text

    extern = "\n".join(f'        "{n}" => Tok::Kw(Kw::{n[2:]}),' for n in names)
    header = (HERE / "src" / "qb_head.part").read_text().replace("@TERMINALS@", extern)
    sys.stdout.write(header + "\n" + hand + "\n// ---- generated from qbasbnf.prs ----\n\n" + "\n".join(rules) + "\n" + "\n".join(em.helpers) + "\n// ---- generated from dialect-extensions.toml ----\n\n" + rules_text)

    kw = "//! Generated by prs2lalrpop.py from the token list of `qbasbnf.prs`.\n\n"
    kw += "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n#[allow(non_camel_case_types, clippy::upper_case_acronyms)]\npub enum Kw {\n"
    kw += "".join(f"    {n[2:]},\n" for n in names) + "}\n\n"
    kw += "pub const NAMES: &[(&str, Kw)] = &[\n" + "".join(f'    ("{n}", Kw::{n[2:]}),\n' for n in names) + "];\n"
    (HERE / "src" / "qb_kw.rs").write_text(kw)


main()
