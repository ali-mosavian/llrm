"""bcc against cfront, every function: instruction counts and the causes of the gap.

    uv run python tools/bcccmp.py --bcc BUILD/base --ours BUILD/fullopt --src SRC --out cmp.json MOD...

bcc's objects are disassembled; cfront's asm is read as printed. bcc's static
functions have no public, so a function begins where bcc put the first line of
its definition (LINNUM), checked against publics and Borland's debug scopes.
"""

import re
import json
import struct
import argparse
from pathlib import Path
from collections import Counter

from iced_x86 import Decoder
from iced_x86 import Formatter
from iced_x86 import FormatterSyntax

from qbopt.objectfile import omf

FORMAT = Formatter(FormatterSyntax.MASM)
LABEL = re.compile(r"(L\d+_\d+):$")


def ours(text: str) -> dict[str, list[str]]:
    """Each procedure's lines, labels kept."""
    out, name, lines = {}, None, []
    for line in text.splitlines():
        if m := re.match(r"_(\w+) proc", line):
            name, lines = m.group(1), []
        elif name and line.startswith(f"_{name} endp"):
            out[name] = lines
            name = None
        elif name and line.strip():
            lines.append(line.strip())
    return out


def definitions(source: str, names) -> dict[str, int]:
    """The line each function's definition starts on."""
    found = {}
    for name in names:
        for m in re.finditer(rf"^[A-Za-z_][^;{{}}\n]*\b{name}\s*\(", source, re.M):
            rest = source[m.end() :]
            body, end = rest.find("{"), rest.find(";")
            if body != -1 and (end == -1 or body < end):
                found[name] = source.count("\n", 0, m.start()) + 1
                break
    return found


def linnums(records, segment: int) -> list[tuple[int, int]]:
    pairs = []
    for one in records:
        if one.type & 0xFE != omf.LINNUM:
            continue
        _group, at = omf._index(one.body, 0)
        base, at = omf._index(one.body, at)
        if base != segment:
            continue
        step, offset = (6, "<I") if one.type & 1 else (4, "<H")
        for i in range(at, len(one.body) - step + 1, step):
            pairs.append((struct.unpack_from("<H", one.body, i)[0], struct.unpack_from(offset, one.body, i + 2)[0]))
    return pairs


def scopes(records) -> dict[int, int]:
    """Borland's outermost debug scopes, begin to end: each function with a local or parameter."""
    depth, out, begin = 0, {}, 0
    for one in records:
        if one.type != omf.COMENT or one.body[1] not in (0xE5, 0xE7):
            continue
        if one.body[1] == 0xE5:
            if depth == 0:
                begin = struct.unpack_from("<H", one.body, 3)[0]
            depth += 1
        else:
            depth -= 1
            if depth == 0:
                out[begin] = struct.unpack_from("<H", one.body, 2)[0]
    return out


def starts(pairs: list[tuple[int, int]], lines: dict[str, int]) -> list[tuple[int, str]]:
    """Where each function begins: the lowest offset of any line between its definition and the next."""
    order = sorted(lines.items(), key=lambda one: one[1])
    begun = []
    for i, (name, line) in enumerate(order):
        end = order[i + 1][1] if i + 1 < len(order) else 1 << 30
        offsets = [offset for number, offset in pairs if line <= number < end]
        if offsets:
            begun.append((min(offsets), name))
    return sorted(begun)


def extents(image: bytes, begun: list[tuple[int, str]], ends: dict[int, int]) -> dict[str, tuple[int, int]]:
    """Each function's bytes. A switch table sits in the code segment after its last instruction."""
    out = {}
    for (lo, name), (hi, _) in zip(begun, [*begun[1:], (len(image), "")]):
        hi = min(hi, ends.get(lo, hi))
        code = " ".join(FORMAT.format(one) for one in Decoder(16, image[lo:hi], ip=lo))
        tables = [
            int(t[:-1], 16) if t.endswith("h") else int(t) for t in re.findall(r"cs:\[\w+\+([0-9A-F]+h?)\]", code)
        ]
        out[name] = (lo, min([hi, *(t for t in tables if lo < t < hi)]))
    return out


def instructions(lines: list[str], listing: bool, labels: bool = False) -> list[str]:
    """Instruction texts, without notes, and labels only if asked; inline `db` runs decoded like object bytes."""
    out, blob = [], bytearray()

    def flush():
        out.extend(FORMAT.format(one) for one in Decoder(16, bytes(blob)))
        blob.clear()

    for line in lines:
        text = line.split(" ", 1)[1] if listing else line
        if text.startswith("db "):
            blob.extend(int(byte.rstrip("h"), 16) for byte in text[3:].split(","))
            continue
        flush()
        if labels or not LABEL.match(text):
            out.append(text.split(" ; ")[0])
    flush()
    # -f87 writes FWAIT before x87 instructions: a no-op on a 387, not an instruction choice.
    return [one for one in out if one != "wait"]


def bcc(path: Path, source: str, names) -> tuple[dict[str, list[str]], list[str]]:
    """Each function's listing, calls and externals named from the fixups."""
    records = omf.read(path)
    externs = omf.externals(records)
    publics = omf.public_definitions(records)
    lines = definitions(source, names)
    problems = [f"{path.stem}.{name}: no definition" for name in names if name not in lines]
    ends = scopes(records)
    out = {}
    for index, segment in enumerate(omf.segments(records)):
        if not segment or not segment[0].endswith("_TEXT"):
            continue
        image = omf.segment_image(records, index, segment[1])
        fixed = {one.offset: one for one in omf.fixups(records) if one.seg == index}
        begun = starts(linnums(records, index), lines)
        named = dict(begun)
        problems += [
            f"{path.stem}.{name}: starts {offset:04x}, public {publics[name][1]:04x}"
            for offset, name in begun
            if name in publics and publics[name][1] != offset
        ]
        problems += [f"{path.stem}: scope at {offset:04x} starts no function" for offset in set(ends) - set(named)]
        for name, (lo, hi) in extents(image, begun, ends).items():
            listing = []
            for insn in Decoder(16, image[lo:hi], ip=lo):
                text = FORMAT.format(insn)
                fix = next((fixed[at] for at in range(insn.ip + 1, insn.ip + insn.len) if at in fixed), None)
                if fix is not None and fix.target == "external" and 0 < fix.index < len(externs):
                    text = (
                        text.replace("0:0", externs[fix.index]) if "0:0" in text else f"{text} ; {externs[fix.index]}"
                    )
                elif (m := re.fullmatch(r"call ([0-9A-F]+)h", text)) and int(m.group(1), 16) in named:
                    text += f" ; {named[int(m.group(1), 16)]}"
                listing.append(f"{insn.ip:04x} {text}")
            out[name] = listing
    return out, problems


# Each cause as a pattern over cfront's printed instructions, and what one site costs beyond bcc's form.
CAUSES = (
    ("far pointer split through shr", r"mov (e\w\w), (e\w\w)\nshr \1, 16\n", 2),
    ("halves joined through the stack", r"push \w\w\npush \w\w\npop e\w\w\n", 1),
    ("inline float to int", r"fnstcw [^\n]+\nfnstcw [^\n]+\nor [^\n]+\nfldcw [^\n]+\nfistp [^\n]+\nfldcw [^\n]+\n", 5),
    ("int constant to x87 through a slot", r"mov word ptr \[bp-\w+\], -?\d+\nfi\w+ word ptr \[bp-\w+\]\n", 1),
    (
        "slot load, op, store back",
        r"mov (\w\w), word ptr (\[bp-\w+\])\n(add|sub|and|or|xor|shl|shr) \1, [^\n]+\nmov word ptr \2, \1\n",
        2,
    ),
    ("slot load then test or compare", r"mov (\w\w), (word ptr )?\[bp[-+]\w+\]\n(or \1, \1|cmp \1, -?\w+)\n", 1),
    ("constant stored to slot and register", r"mov word ptr \[bp-\w+\], -?\d+\n(mov \w\w, -?\d+|xor (\w\w), \2)\n", 1),
    ("mov sp,bp; pop bp", r"mov sp, bp\npop bp\n", 1),
    # The loop header's label sits between the constant and its compare.
    ("constant compare at loop entry", r"(xor (\w\w), \2|mov (\w\w), -?\d+)\n(L\d+_\d+:\n)+cmp (\2|\3), -?\d+\nj", 2),
    ("jcc over a jmp", r"\nj(?!mp)\w+ \S+\njmp \S+\n", 1),
)


def jumps(lines: list[str]) -> Counter:
    """Jumps layout could remove: to the next label, or to a block that only jumps."""
    found = Counter()
    first = {}
    for i, line in enumerate(lines):
        if m := LABEL.match(line):
            rest = next((one for one in lines[i + 1 :] if not LABEL.match(one)), "")
            first[m.group(1)] = rest
    for i, line in enumerate(lines):
        if not (m := re.match(r"(j\w+) (L\d+_\d+)$", line)):
            continue
        if first.get(m.group(2), "").startswith("jmp "):
            found["jump to a jmp"] += 1
        following = []
        for one in lines[i + 1 :]:
            if not (label := LABEL.match(one)):
                break
            following.append(label.group(1))
        if m.group(1) == "jmp" and m.group(2) in following:
            found["jmp to the next label"] += 1
    return found


def counted(lines: list[str]) -> Counter:
    """Each cause's sites in one procedure's printed lines."""
    # Labels stay in: a pattern across one spans two blocks, which nothing local can fuse.
    printed = "\n" + "\n".join(instructions(lines, listing=False, labels=True)) + "\n"
    found = Counter({cause: len(re.findall(pattern, printed)) for cause, pattern, _cost in CAUSES})
    found.update(jumps(lines))
    return found


def compare(bcc_dir: Path, ours_dir: Path, src_dir: Path, modules: list[str]) -> dict:
    functions, problems, causes = [], [], Counter()
    for module in modules:
        mine = ours((ours_dir / f"{module}.asm").read_text())
        theirs, trouble = bcc(bcc_dir / f"{module}.obj", (src_dir / f"{module}.c").read_text(), mine)
        problems += trouble
        for name, lines in mine.items():
            if name not in theirs:
                problems.append(f"{module}.{name}: no bcc region")
                continue
            causes.update(counted(lines))
            functions.append(
                {
                    "module": module,
                    "name": name,
                    "bcc": theirs[name],
                    "ours": lines,
                    "counts": [len(instructions(theirs[name], listing=True)), len(instructions(lines, listing=False))],
                }
            )
    costs = {cause: cost for cause, _pattern, cost in CAUSES}
    return {
        "totals": [sum(one["counts"][side] for one in functions) for side in (0, 1)],
        "functions": functions,
        "causes": {cause: [n, n * costs.get(cause, 1)] for cause, n in causes.items()},
        "problems": problems,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--bcc", type=Path, required=True)
    parser.add_argument("--ours", type=Path, required=True)
    parser.add_argument("--src", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("modules", nargs="+")
    args = parser.parse_args()
    report = compare(args.bcc, args.ours, args.src, args.modules)
    args.out.write_text(json.dumps(report))
    print(f"{len(report['functions'])} functions: bcc {report['totals'][0]}, cfront {report['totals'][1]}")
    for cause, (sites, cost) in sorted(report["causes"].items(), key=lambda one: -one[1][1]):
        print(f"{sites:>6} sites {cost:>6} instructions  {cause}")
    print("\n".join(report["problems"]))


if __name__ == "__main__":
    main()
