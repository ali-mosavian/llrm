"""Small Markdown to HTML converter for this repository's documents.

`convert(text)` returns a `Result`: the HTML, the headings with their ids, the
links found, the figure errors, and `unrecognised`, the constructs it could not
convert (the build fails on any).
"""
from __future__ import annotations

import re
from collections.abc import Callable
from dataclasses import dataclass, field
from html import escape as _esc

from . import figures

FENCE = re.compile(r"^ {0,3}(`{3,}|~{3,})\s*(.*)$")
ATX = re.compile(r"^ {0,3}(#{1,6})\s+(.*?)(?:\s+#+)?\s*$")
HR = re.compile(r"^ {0,3}([-*_])(?:\s*\1){2,}\s*$")
ITEM = re.compile(r"^( *)([-*+]|\d{1,9}[.)])( +|$)(.*)$")
DELIM = re.compile(r"^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$")


@dataclass
class Result:
    html: str = ""
    headings: list[tuple[int, str, str]] = field(default_factory=list)  # level, text, id
    links: list[tuple[int, str]] = field(default_factory=list)  # line, href
    figure_errors: list[tuple[int, str]] = field(default_factory=list)
    unrecognised: list[tuple[int, str]] = field(default_factory=list)
    figures: int = 0


def slugify(text: str) -> str:
    t = re.sub(r"!?\[([^\]]*)\]\([^)]*\)", r"\1", text)
    t = re.sub(r"[`*]", "", t).lower()
    t = re.sub(r"[^\w\s-]", "", t)
    return re.sub(r"\s", "-", t.strip())


def esc(s: str) -> str:
    return _esc(s, quote=True)


class Converter:
    def __init__(self, link_hook: Callable[[str], str] | None = None) -> None:
        self.hook = link_hook or (lambda h: h)
        self.res = Result()
        self.slugs: dict[str, int] = {}

    # ------------------------------------------------------------ inline
    def inline(self, text: str, line: int) -> str:
        holds: list[str] = []

        def hold(h: str) -> str:
            holds.append(h)
            return f"\x01{len(holds) - 1}\x02"

        # code spans
        out, i = [], 0
        while i < len(text):
            if text[i] == "`":
                j = i
                while j < len(text) and text[j] == "`":
                    j += 1
                run = text[i:j]
                k = text.find(run, j)
                while k != -1 and (text[k + len(run) : k + len(run) + 1] == "`" or text[k - 1 : k] == "`"):
                    k = text.find(run, k + 1)
                if k == -1:
                    out.append(run)
                    i = j
                    continue
                code = text[j:k].replace("\n", " ")
                if code.startswith(" ") and code.endswith(" ") and code.strip():
                    code = code[1:-1]
                out.append(hold(f"<code>{esc(code)}</code>"))
                i = k + len(run)
            elif text[i] == "\\" and i + 1 < len(text) and text[i + 1] in "\\`*_{}[]()#+-.!|<>~":
                out.append(hold(esc(text[i + 1])))
                i += 2
            else:
                out.append(text[i])
                i += 1
        text = self._rest("".join(out), line, hold)
        while "\x01" in text:
            text = re.sub(r"\x01(\d+)\x02", lambda m: holds[int(m.group(1))], text)
        return text

    def _rest(self, text: str, line: int, hold) -> str:
        text = self._links(text, line, hold)
        text = re.sub(r"<(https?://[^\s>]+)>", lambda m: hold(self._anchor(m.group(1), esc(m.group(1)), line)), text)
        text = re.sub(r"<br\s*/?>", lambda m: hold("<br>"), text)
        text = re.sub(
            r"https?://[^\s<>\x01\x02]+",
            lambda m: self._bare(m.group(0), line, hold),
            text,
        )
        for m in re.finditer(r"</?[A-Za-z][^>\x01]*>", text):
            self.res.unrecognised.append((line, f"raw HTML {m.group(0)}"))
        if "~~" in text:
            self.res.unrecognised.append((line, "strikethrough"))
        text = esc(text)
        text = re.sub(r"\*\*(?=\S)(.+?)(?<=\S)\*\*", r"<strong>\1</strong>", text)
        text = re.sub(r"(?<![\w_])__(?=\S)(.+?)(?<=\S)__(?![\w_])", r"<strong>\1</strong>", text)
        text = re.sub(r"(?<![*\w])\*(?=[^\s*])(.+?)(?<=[^\s*])\*(?![*\w])", r"<em>\1</em>", text)
        return re.sub(r"(?<![\w_])_(?=[^\s_])(.+?)(?<=[^\s_])_(?![\w_])", r"<em>\1</em>", text)

    def _anchor(self, href: str, inner: str, line: int, title: str = "") -> str:
        self.res.links.append((line, href))
        t = f' title="{esc(title)}"' if title else ""
        return f'<a href="{esc(self.hook(href))}"{t}>{inner}</a>'

    def _bare(self, url: str, line: int, hold) -> str:
        trail = ""
        while url and (url[-1] in ".,;:!?'\"" or (url[-1] == ")" and url.count("(") < url.count(")"))):
            trail = url[-1] + trail
            url = url[:-1]
        return hold(self._anchor(url, esc(url), line)) + trail

    def _links(self, text: str, line: int, hold) -> str:
        out, i = [], 0
        while i < len(text):
            c = text[i]
            img = c == "!" and text[i + 1 : i + 2] == "["
            if c == "[" or img:
                s = i + (2 if img else 1)
                depth, j = 1, s
                while j < len(text) and depth:
                    depth += {"[": 1, "]": -1}.get(text[j], 0)
                    j += 1
                if depth == 0:
                    label = text[s : j - 1]
                    if text[j : j + 1] == "(":
                        m = self._dest(text, j)
                        if m:
                            url, title, end = m
                            if img:
                                out.append(hold(f'<img src="{esc(self.hook(url))}" alt="{esc(re.sub(chr(1) + r"\d+" + chr(2), "", label))}" loading="lazy">'))
                                self.res.links.append((line, url))
                            else:
                                out.append(hold(self._anchor(url, self._rest(label, line, hold), line, title)))
                            i = end
                            continue
                    elif text[j : j + 1] == "[" and "]" in text[j:]:
                        self.res.unrecognised.append((line, "reference-style link"))
            out.append(c)
            i += 1
        return "".join(out)

    @staticmethod
    def _dest(text: str, j: int):
        depth, k = 0, j
        while k < len(text):
            if text[k] == "(":
                depth += 1
            elif text[k] == ")":
                depth -= 1
                if depth == 0:
                    break
            k += 1
        else:
            return None
        body = text[j + 1 : k].strip()
        m = re.fullmatch(r'(\S+)(?:\s+"(.*)")?', body)
        if not m:
            return None
        url = m.group(1)
        if url.startswith("<") and url.endswith(">"):
            url = url[1:-1]
        return url, m.group(2) or "", k + 1

    # ------------------------------------------------------------ blocks
    def blocks(self, lines: list[tuple[int, str]], tight: bool = False) -> str:
        out: list[str] = []
        i, n = 0, len(lines)
        while i < n:
            no, ln = lines[i]
            if not ln.strip():
                i += 1
                continue
            m = FENCE.match(ln)
            if m and not (m.group(1)[0] == "`" and "`" in m.group(2)):
                i = self._fence(lines, i, m, out)
                continue
            if len(ln) - len(ln.lstrip()) >= 4:
                i = self._indented(lines, i, out)
                continue
            m = ATX.match(ln)
            if m:
                out.append(self._heading(len(m.group(1)), m.group(2), no))
                i += 1
            elif HR.match(ln) and not ITEM.match(ln):
                out.append("<hr>")
                i += 1
            elif ln.lstrip().startswith(">") and ln.lstrip() == ln.lstrip(" ") and len(ln) - len(ln.lstrip()) < 4:
                i = self._quote(lines, i, out)
            elif self._table_at(lines, i):
                i = self._table(lines, i, out)
            elif ITEM.match(ln) and len(ln) - len(ln.lstrip()) < 4:
                i = self._list(lines, i, out)
            else:
                i = self._para(lines, i, out, tight)
        return "\n".join(out)

    def _heading(self, level: int, text: str, no: int) -> str:
        base = slugify(text)
        k = self.slugs.get(base, 0)
        self.slugs[base] = k + 1
        sid = base if k == 0 else f"{base}-{k}"
        self.res.headings.append((level, re.sub(r"[`*]", "", text), sid))
        return f'<h{level} id="{esc(sid)}">{self.inline(text, no)}</h{level}>'

    def _fence(self, lines, i, m, out) -> int:
        no, ln = lines[i]
        fence, info = m.group(1), m.group(2).split()[0] if m.group(2).split() else ""
        indent = len(ln) - len(ln.lstrip())
        close = re.compile(rf"^ {{0,3}}{re.escape(fence[0])}{{{len(fence)},}}\s*$")
        body, j = [], i + 1
        stray = re.compile(rf"^ {{0,3}}{re.escape(fence[0])}{{{len(fence)},}}\s*\S")
        while j < len(lines) and not close.match(lines[j][1]):
            if stray.match(lines[j][1]):
                self.res.unrecognised.append((lines[j][0], "fence line with text inside a code block"))
            body.append(re.sub(rf"^ {{0,{indent}}}", "", lines[j][1]))
            j += 1
        if j >= len(lines):
            self.res.unrecognised.append((no, "code fence is never closed"))
        text = "\n".join(body)
        if info == "fig":
            self.res.figures += 1
            try:
                out.append(f'<figure class="figure">{figures.render(text)}</figure>')
            except figures.FigureError as e:
                self.res.figure_errors.append((no + e.line, e.message))
                out.append(f'<pre class="figerr">figure error, line {e.line}: {esc(e.message)}</pre>')
        else:
            cls = f' class="language-{esc(info)}"' if info else ""
            out.append(f"<pre><code{cls}>{esc(text)}\n</code></pre>")
        return j + 1

    def _indented(self, lines, i, out) -> int:
        body = []
        while i < len(lines) and (not lines[i][1].strip() or len(lines[i][1]) - len(lines[i][1].lstrip()) >= 4):
            body.append(lines[i][1][4:])
            i += 1
        while body and not body[-1].strip():
            body.pop()
        out.append(f"<pre><code>{esc(chr(10).join(body))}\n</code></pre>")
        return i

    def _quote(self, lines, i, out) -> int:
        inner = []
        while i < len(lines) and lines[i][1].lstrip().startswith(">"):
            no, ln = lines[i]
            inner.append((no, re.sub(r"^\s*> ?", "", ln)))
            i += 1
        out.append(f"<blockquote>\n{self.blocks(inner)}\n</blockquote>")
        return i

    @staticmethod
    def _cells(row: str) -> list[str]:
        row = row.strip()
        if row.startswith("|"):
            row = row[1:]
        cells, cur, k = [], "", 0
        while k < len(row):
            if row[k] == "\\" and k + 1 < len(row) and row[k + 1] == "|":
                cur += "|"
                k += 2
                continue
            if row[k] == "|":
                cells.append(cur)
                cur = ""
            else:
                cur += row[k]
            k += 1
        if cur.strip() or not row.endswith("|"):
            cells.append(cur)
        return [c.strip() for c in cells]

    def _table_at(self, lines, i) -> bool:
        return (
            i + 1 < len(lines)
            and "|" in lines[i][1]
            and DELIM.match(lines[i + 1][1]) is not None
            and "-" in lines[i + 1][1]
            and len(self._cells(lines[i][1])) == len(self._cells(lines[i + 1][1]))
        )

    def _table(self, lines, i, out) -> int:
        head = self._cells(lines[i][1])
        al = []
        for c in self._cells(lines[i + 1][1]):
            al.append("center" if c.startswith(":") and c.endswith(":") else "right" if c.endswith(":") else "left" if c.startswith(":") else "")
        rows, j = [], i + 2
        while j < len(lines) and lines[j][1].strip() and "|" in lines[j][1]:
            rows.append((lines[j][0], self._cells(lines[j][1])))
            j += 1

        def cell(tag, text, k, no):
            a = f' style="text-align:{al[k]}"' if al[k] else ""
            return f"<{tag}{a}>{self.inline(text, no)}</{tag}>"

        h = "".join(cell("th", c, k, lines[i][0]) for k, c in enumerate(head))
        body = []
        for no, cs in rows:
            if len(cs) > len(head):
                self.res.unrecognised.append((no, f"table row has {len(cs)} cells, header {len(head)}"))
            cs = (cs + [""] * len(head))[: len(head)]
            body.append("<tr>" + "".join(cell("td", c, k, no) for k, c in enumerate(cs)) + "</tr>")
        out.append(f'<div class="tbl"><table>\n<thead><tr>{h}</tr></thead>\n<tbody>\n' + "\n".join(body) + "\n</tbody></table></div>")
        return j

    def _list(self, lines, i, out) -> int:
        m0 = ITEM.match(lines[i][1])
        base = len(m0.group(1))
        ordered = m0.group(2)[0].isdigit()
        start = int(m0.group(2)[:-1]) if ordered else 1
        items: list[list[tuple[int, str]]] = []
        loose = False
        while i < len(lines):
            m = ITEM.match(lines[i][1])
            if not m or len(m.group(1)) != base or m.group(2)[0].isdigit() != ordered:
                break
            width = len(m.group(1)) + len(m.group(2)) + max(len(m.group(3)), 1)
            if len(m.group(3)) > 4:
                width = len(m.group(1)) + len(m.group(2)) + 1
            item = [(lines[i][0], m.group(4))]
            i += 1
            while i < len(lines):
                no, ln = lines[i]
                ind = len(ln) - len(ln.lstrip())
                if not ln.strip():
                    k = i
                    while k < len(lines) and not lines[k][1].strip():
                        k += 1
                    if k < len(lines) and (len(lines[k][1]) - len(lines[k][1].lstrip()) >= width):
                        item += [(lines[x][0], "") for x in range(i, k)]
                        loose = True
                        i = k
                        continue
                    if k < len(lines):
                        nm = ITEM.match(lines[k][1])
                        if nm and len(nm.group(1)) == base and nm.group(2)[0].isdigit() == ordered:
                            loose = True
                    break
                if ind >= width:
                    item.append((no, ln[width:]))
                    i += 1
                elif ITEM.match(ln) and ind <= base or FENCE.match(ln) or ATX.match(ln) or ln.lstrip().startswith(">") or HR.match(ln):
                    break
                elif item[-1][1] == "" or self._opens_block(item):
                    break
                else:
                    item.append((no, ln.strip()))
                    i += 1
            items.append(item)
            if i < len(lines) and not lines[i][1].strip():
                k = i
                while k < len(lines) and not lines[k][1].strip():
                    k += 1
                if k < len(lines):
                    nm = ITEM.match(lines[k][1])
                    if nm and len(nm.group(1)) == base and nm.group(2)[0].isdigit() == ordered:
                        i = k
                        continue
                break
        tag = "ol" if ordered else "ul"
        st = f' start="{start}"' if ordered and start != 1 else ""
        body = "\n".join(f"<li>{self.blocks(it, tight=not loose)}</li>" for it in items)
        out.append(f"<{tag}{st}>\n{body}\n</{tag}>")
        return i

    @staticmethod
    def _opens_block(item) -> bool:
        return any(FENCE.match(t) for _, t in item[-1:]) and False

    def _para(self, lines, i, out, tight) -> int:
        no0 = lines[i][0]
        buf = []
        while i < len(lines):
            no, ln = lines[i]
            if not ln.strip():
                break
            if buf and (
                (FENCE.match(ln) and not (ln.lstrip().startswith("`") and "`" in FENCE.match(ln).group(2)))
                or ATX.match(ln)
                or HR.match(ln)
                or ln.lstrip().startswith(">")
                or (ITEM.match(ln) and (ITEM.match(ln).group(2) in "-*+" or ITEM.match(ln).group(2) == "1."))
                or self._table_at(lines, i)
            ):
                break
            buf.append(ln)
            i += 1
        joined = "\n".join(s.rstrip("\n") if k == len(buf) - 1 else s for k, s in enumerate(buf))
        joined = re.sub(r"(?: {2,}|\\)\n", "\x03\n", joined.strip())
        html = self.inline(joined.replace("\x03", "\x03"), no0).replace("\x03", "<br>")
        out.append(html if tight else f"<p>{html}</p>")
        return i


def convert(text: str, link_hook: Callable[[str], str] | None = None) -> Result:
    c = Converter(link_hook)
    lines = [(n, s.rstrip("\r")) for n, s in enumerate(text.expandtabs(4).split("\n"), 1)]
    c.res.html = c.blocks(lines)
    return c.res
