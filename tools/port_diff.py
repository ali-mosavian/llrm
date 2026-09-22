"""Diff the Rust port's stage dumps against Python's, first divergence first.

    uv run python tools/port_diff.py fixtures/c/parity/scalar.cgs [--opt]
    uv run python tools/port_diff.py --corpus [--opt]
    uv run python tools/port_diff.py --qb [bench/parity/algebra.bas ...]

Python writes the reference dump (`python -m qbopt.cfront --dump`); the Rust
port writes the same tree (`llrm-c --dump`). With `--qb`, Python's is
`tools/qbstages.py` and Rust's `llrm-qb --dump`; with no sources it runs
`bench/parity/*.bas` and every `.bas` tests/test_hir.py names. Stages are
compared in the order Python wrote them, so the first mismatch is the first
stage the port gets wrong. `frozenset` elements are sorted on both sides: their order is Python's
hash order, not a fact of the compiler.
"""

import os
import re
import sys
import argparse
import subprocess
from pathlib import Path
from dataclasses import dataclass

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ("fixtures/c/*.cgs", "fixtures/c/mir/*.cgs", "fixtures/c/parity/*.cgs")
QBFRONT = ROOT / "frontends/qb/target/release/qbfront"
OBJECT = "out.obj"
REFUSAL = "refusal"


@dataclass(frozen=True)
class Divergence:
    stage: str
    line: int
    python: str
    rust: str


@dataclass(frozen=True)
class Result:
    source: Path
    matched: int
    total: int
    first: Divergence | None
    rust_error: str


def _elements(text: str) -> list[str]:
    """Top-level comma-separated elements, respecting brackets and quotes."""
    out, depth, start, quote, at = [], 0, 0, "", 0
    while at < len(text):
        char = text[at]
        if quote:
            if char == "\\":
                at += 1
            elif char == quote:
                quote = ""
        elif char in "'\"":
            quote = char
        elif char in "([{":
            depth += 1
        elif char in ")]}":
            depth -= 1
        elif char == "," and depth == 0:
            out.append(text[start:at].strip())
            start = at + 1
        at += 1
    if text[start:].strip():
        out.append(text[start:].strip())
    return out


def normalized(text: str) -> str:
    """The text with every `frozenset({...})` element list sorted."""
    marker = "frozenset({"
    out, at = [], 0
    while (found := text.find(marker, at)) >= 0:
        out.append(text[at:found])
        start = found + len(marker)
        depth, end, quote = 1, start, ""
        while depth:
            char = text[end]
            if quote:
                if char == "\\":
                    end += 1
                elif char == quote:
                    quote = ""
            elif char in "'\"":
                quote = char
            elif char in "([{":
                depth += 1
            elif char in ")]}":
                depth -= 1
            end += 1
        inner = text[start : end - 1]
        out.append(marker + ", ".join(sorted(normalized(one) for one in _elements(inner))) + "}")
        at = end
    out.append(text[at:])
    return "".join(out)


PIPELINE = ("stream", "hir", "mir", "passes", "phases", "lir", "asm")


def _stages(python: Path, qb: bool = False) -> list[str]:
    """Python's dump files in pipeline order, then write order; the object last.

    Python writes the whole-unit `mir` after every procedure's phases, so
    write order alone would rank the raise after register allocation. A QB
    dump is flat and written in pipeline order: write order is the order.
    """
    files = [one for one in python.rglob("*") if one.is_file() and one.name not in (OBJECT, REFUSAL)]
    if qb:
        files.sort(key=lambda one: (one.stat().st_mtime_ns, one.name))
    else:
        files.sort(key=lambda one: (PIPELINE.index(one.relative_to(python).parts[0]), one.stat().st_mtime_ns))
    staged = [str(one.relative_to(python)) for one in files]
    return staged + [one for one in (OBJECT, REFUSAL) if (python / one).exists()]


def compare(python: Path, rust: Path, qb: bool = False) -> tuple[int, int, Divergence | None]:
    """How many of Python's stages Rust reproduces, and the first it does not."""
    stages = _stages(python, qb)
    matched, first = 0, None
    for stage in stages:
        want, got = python / stage, rust / stage
        if stage == OBJECT:
            same = got.exists() and want.read_bytes() == got.read_bytes()
            if not same and first is None:
                first = Divergence(
                    stage, 0, f"{want.stat().st_size} bytes", "missing" if not got.exists() else "differs"
                )
        else:
            left = normalized(want.read_text()).splitlines()
            right = normalized(got.read_text()).splitlines() if got.exists() else None
            same = left == right
            if not same and first is None:
                if right is None:
                    first = Divergence(stage, 0, "", "missing")
                else:
                    line = next(
                        (n for n, (a, b) in enumerate(zip(left, right, strict=False)) if a != b),
                        min(len(left), len(right)),
                    )
                    first = Divergence(
                        stage,
                        line + 1,
                        left[line] if line < len(left) else "<end>",
                        right[line] if line < len(right) else "<end>",
                    )
        matched += same
    return matched, len(stages), first


def _message(stderr: str, prefix: str) -> str:
    """A refusal's message: Python's `Class: message` or Rust's `llrm-c: message`."""
    last = stderr.strip().splitlines()[-1] if stderr.strip() else ""
    return last.partition(prefix)[2] if prefix in last else last


def _rust_binary(name: str = "llrm-c") -> Path:
    subprocess.run(["cargo", "build", "--quiet", "--bin", name], cwd=ROOT, check=True)
    return ROOT / "target" / "debug" / name


def qb_corpus() -> list[Path]:
    """`bench/parity/*.bas`, then every `.bas` tests/test_hir.py names that exists."""
    named = re.findall(r'"((?:frontends|bench)/[^"]+\.bas)"', (ROOT / "tests/test_hir.py").read_text())
    found = sorted(ROOT.glob("bench/parity/*.bas"))
    found += [ROOT / one for one in dict.fromkeys(named) if (ROOT / one).is_file() and ROOT / one not in found]
    return found


def oracle_env() -> dict[str, str]:
    """This checkout's qbopt first: a script's own folder does not make the checkout importable."""
    path = os.pathsep.join(filter(None, (str(ROOT), os.environ.get("PYTHONPATH"))))
    return {**os.environ, "PYTHONHASHSEED": "0", "PYTHONPATH": path}


def run_qb(source: Path, work: Path, rust: Path) -> Result:
    python_dir, rust_dir = work / "python", work / "rust"
    for one in (python_dir, rust_dir):
        one.mkdir(parents=True, exist_ok=True)
    env = oracle_env()
    if QBFRONT.is_file():
        env.setdefault("QBOPT_QBFRONT", str(QBFRONT))
    python = [sys.executable, "-m"]
    made = subprocess.run(
        [sys.executable, str(ROOT / "tools/qbstages.py"), str(source), "--output", str(python_dir)],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
    )
    if not made.returncode:
        made = subprocess.run(
            [*python, "qbopt.frontend.qb", str(source), "-o", str(python_dir / OBJECT)],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
        )
    if made.returncode:
        (python_dir / REFUSAL).write_text(_message(made.stderr, ": ") + "\n")
    done = subprocess.run(
        [str(rust), str(source), "--dump", str(rust_dir), "-o", str(rust_dir / OBJECT)],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
    )
    if done.returncode:
        (rust_dir / REFUSAL).write_text(_message(done.stderr, "llrm-qb: ") + "\n")
    matched, total, first = compare(python_dir, rust_dir, qb=True)
    return Result(source, matched, total, first, done.stderr.strip() if done.returncode else "")


def run(source: Path, work: Path, rust: Path, opt: bool) -> Result:
    python_dir, rust_dir = work / "python", work / "rust"
    for one in (python_dir, rust_dir):
        one.mkdir(parents=True, exist_ok=True)
    flags = ["--opt"] if opt else []
    env = oracle_env()
    made = subprocess.run(
        [
            sys.executable,
            "-m",
            "qbopt.cfront",
            str(source),
            "-o",
            str(python_dir / OBJECT),
            "--dump",
            str(python_dir),
            *flags,
        ],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
    )
    if made.returncode:
        (python_dir / REFUSAL).write_text(_message(made.stderr, ": ") + "\n")
    done = subprocess.run(
        [str(rust), str(source), "-o", str(rust_dir / OBJECT), "--dump", str(rust_dir), *flags],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if done.returncode:
        (rust_dir / REFUSAL).write_text(_message(done.stderr, "llrm-c: ") + "\n")
    matched, total, first = compare(python_dir, rust_dir)
    return Result(source, matched, total, first, done.stderr.strip() if done.returncode else "")


def _display(source: Path) -> Path:
    """The source as the report names it: relative to the repository where it is inside it."""
    resolved = source.resolve()
    return resolved.relative_to(ROOT) if resolved.is_relative_to(ROOT) else source


def _workdir(work: Path, source: Path) -> Path:
    """Where one source's two dumps go, keyed by its whole path: stems repeat across fixture folders."""
    return work / _display(source).with_suffix("")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("sources", nargs="*", type=Path)
    parser.add_argument("--corpus", action="store_true", help="every C fixture")
    parser.add_argument("--opt", action="store_true")
    parser.add_argument("--qb", action="store_true", help="QB sources through llrm-qb; no sources: the QB corpus")
    parser.add_argument("--work", type=Path, default=ROOT / "build" / "port_diff")
    args = parser.parse_args(argv)
    sources = list(args.sources)
    if args.corpus:
        sources += sorted(path for pattern in CORPUS for path in ROOT.glob(pattern))
    if args.qb and not sources:
        sources = qb_corpus()
    if not sources:
        parser.error("no sources")
    rust = _rust_binary("llrm-qb" if args.qb else "llrm-c")
    failed = 0
    for source in sources:
        work = _workdir(args.work, source)
        if args.qb:
            result = run_qb(source.resolve(), work, rust)
        else:
            result = run(source.resolve(), work, rust, args.opt)
        name = _display(source)
        if result.first is None:
            print(f"ok    {name}: {result.matched}/{result.total}")
            continue
        failed += 1
        first = result.first
        print(f"FAIL  {name}: {result.matched}/{result.total}, first {first.stage}:{first.line}")
        if first.python or first.rust:
            print(f"        python: {first.python[:200]}")
            print(f"        rust:   {first.rust[:200]}")
        if result.rust_error:
            print(f"        {result.rust_error.splitlines()[-1][:200]}")
    print(f"{len(sources) - failed}/{len(sources)} ok")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
