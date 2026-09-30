"""
Correctness, fast: a program's MIR as the pipeline got it and as it left it,
run in llrm-mir's interpreter.

The program's externals are replaced by stubs written here in MIR: the report
call folds its value into a hash, the opaque calls do what spec.OPAQUE says,
and anything else the stubs do not know makes the run refuse, never guess.
`@__loops_entry` calls the program's main and returns the hash.
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

from build import BIN

MAIN = {"c": "_main", "bas": "__main", "nib": "main"}
REPORT = {"_report", "B$PEI4", "N$PI4"}
SILENT = {"N$PN", "B$CEND"}
FUEL = 400_000_000


class Unrunnable(Exception):
    """The MIR calls something no stub models."""


def fold(values: list[int]) -> int:
    h = 0
    for one in values:
        h = (h * 31 + one) & 0xFFFFFFFF
    return h


def _split(args: str) -> list[str]:
    out, depth, current = [], 0, ""
    for ch in args:
        if ch in "([{<":
            depth += 1
        elif ch in ")]}>":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(current.strip())
            current = ""
        else:
            current += ch
    if current.strip():
        out.append(current.strip())
    return out


DECLARE = re.compile(r"^declare\s+(?P<pre>(?:\w+\s+)*?)(?P<ret>void|i\d+|ptr(?: addrspace\(\d\))?)\s+@(?P<name>\"[^\"]+\"|[\w.$]+)\((?P<rest>.*)$")


def _declared(line: str):
    """(name, return type, argument types, what follows) of a declare line."""
    match = DECLARE.match(line)
    if not match:
        return None
    rest, depth = match.group("rest"), 1
    for at, ch in enumerate(rest):
        depth += {"(": 1, ")": -1}.get(ch, 0)
        if depth == 0:
            return match.group("name"), match.group("ret"), _split(rest[:at]), rest[at + 1 :]
    return None


def _zero(kind: str) -> str:
    return "void" if kind == "void" else f"{kind} {'null' if kind.startswith('ptr') else 0}"


def _stub(name: str, ret: str, args: list[str], post: str) -> str:
    """A definition doing what the external does."""
    plain = name.strip('"').removeprefix("llrm.qb.")
    bare = plain.lstrip("_")
    types = [re.sub(r"\s+(noundef|nocapture|readonly|writeonly|signext|zeroext|noalias)\b", "", one).strip() for one in args]
    params = ", ".join(f"{kind} %a{at}" for at, kind in enumerate(types))
    space = re.search(r"addrspace\(\d\)", post)
    head = f"define {ret} @{name}({params}) {space.group(0) if space else ''} {{"
    back = f"ret {_zero(ret)}" if ret != "void" else "ret void"
    if plain in REPORT or bare == "report":
        wide = types[0]
        widen = f"%w = sext {wide} %a0 to i32" if wide != "i32" else "%w = add i32 %a0, 0"
        body = [widen, "%h = load i32, ptr @__loops_hash", "%m = mul i32 %h, 31", "%s = add i32 %m, %w",
                "store i32 %s, ptr @__loops_hash", back]
    elif plain in SILENT or bare in ("touch",):
        body = [back]
    elif bare in ("keep", "keep32"):
        body = [f"ret {types[0]} %a0"]
    elif bare == "tick":
        body = ["%t = load i16, ptr @__loops_ticks", "%u = add i16 %t, 1", "store i16 %u, ptr @__loops_ticks", back]
    elif bare == "tick_count":
        body = ["%t = load i16, ptr @__loops_ticks", "store i16 0, ptr @__loops_ticks", f"ret {ret} %t"]
    elif plain == "B$UBND":
        # Microsoft ARRAY.INC's descriptor: rank at 8, then per dimension,
        # last first (column-major), a word count and a word lower bound
        return "\n".join([
            head,
            f"  %rp = getelementptr i8, {types[0]} %a0, i16 8", f"  %r8 = load i8, {types[0]} %rp",
            "  %rank = zext i8 %r8 to i16", "  %rec = sub i16 %rank, %a1", "  %off = mul i16 %rec, 4",
            "  %co = add i16 %off, 14", f"  %cp = getelementptr i8, {types[0]} %a0, i16 %co",
            f"  %count = load i16, {types[0]} %cp", "  %lo = add i16 %off, 16",
            f"  %lp = getelementptr i8, {types[0]} %a0, i16 %lo", f"  %low = load i16, {types[0]} %lp",
            "  %sum = add i16 %low, %count", "  %up = sub i16 %sum, 1", "  ret i16 %up", "}",
        ])
    elif bare == "lcopy":
        return "\n".join([
            head,
            "b0:", "  br label %b1",
            "b1:", "  %k = phi i16 [ 0, %b0 ], [ %k1, %b2 ]", "  %c = icmp ult i16 %k, %a2", "  br i1 %c, label %b2, label %b3",
            "b2:", f"  %ps = getelementptr i8, {types[1]} %a1, i16 %k", f"  %x = load i8, {types[1]} %ps",
            f"  %pd = getelementptr i8, {types[0]} %a0, i16 %k", f"  store i8 %x, {types[0]} %pd",
            "  %k1 = add i16 %k, 1", "  br label %b1",
            "b3:", f"  {back}", "}",
        ])
    else:
        raise Unrunnable(f"no stub for the external @{name}")
    return "\n".join([head, *("  " + one for one in body), "}"])


def harness(text: str, lang: str) -> str:
    out = []
    for line in text.splitlines():
        # BASIC's END never returns; here it returns to the entry.
        if line.strip() == "unreachable" and out and "@llrm.qb.B$CEND()" in out[-1]:
            out.append("  ret void")
            continue
        found = _declared(line)
        if found and not found[0].startswith("llvm."):
            out.append(_stub(*found))
        else:
            out.append(line)
    main = MAIN[lang]
    signature = re.search(rf"^define .*?(i\d+|void) @{re.escape(main)}\(", text, re.M)
    if not signature:
        raise Unrunnable(f"no @{main}")
    kind = signature.group(1)
    call = f"call addrspace(1) {kind} @{main}()"
    out += [
        "@__loops_hash = global i32 0",
        "@__loops_ticks = global i16 0",
        "define i32 @__loops_entry() addrspace(1) {",
        f"  {'%r = ' if kind != 'void' else ''}{call}",
        "  %h = load i32, ptr @__loops_hash",
        "  ret i32 %h",
        "}",
    ]
    return "\n".join(out) + "\n"


def stages(directory: Path) -> tuple[Path, Path]:
    """The pipeline's input and its last MIR."""
    files = sorted(directory.glob("[0-9][0-9]-*.ll"))
    if not files:
        raise Unrunnable(f"no MIR stages in {directory}")
    return files[0], files[-1]


def run(path: Path, lang: str, work: Path) -> int:
    text = harness(path.read_text(), lang)
    target = work / f"{path.stem}.run.ll"
    target.write_text(text)
    done = subprocess.run(
        [str(BIN / "llrm-mir"), "--run", "--entry", "__loops_entry", "--fuel", str(FUEL), str(target)],
        capture_output=True, text=True, timeout=600,
    )
    if done.returncode != 0:
        raise RuntimeError(f"{target}: {done.stderr.strip()[:500]}")
    return int(done.stdout.strip()) & 0xFFFFFFFF


def called(path: Path, functions: list[str]) -> list[str]:
    """Of `functions`, those the program no longer calls by name: inlined,
    so their own code would go unchecked."""
    text = path.read_text()
    return [one for one in functions if not re.search(rf"call [^\n]*@\"?{re.escape(one)}\"?\(", text)]
