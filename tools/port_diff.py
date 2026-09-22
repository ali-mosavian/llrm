"""Diff the Rust port's stage dumps against Python's, first divergence first.

    uv run python tools/port_diff.py fixtures/c/parity/scalar.cgs [--opt]
    uv run python tools/port_diff.py --corpus [--opt]
    uv run python tools/port_diff.py --qb [bench/parity/algebra.bas ...]
    uv run python tools/port_diff.py --bc [fixtures/omf/hotlop-p-g2.obj ...]

Python writes the reference dump (`python -m qbopt.cfront --dump`); the Rust
port writes the same tree (`llrm-c --dump`). With `--qb`, Python's is
`tools/qbstages.py` and Rust's `llrm-qb --dump`; with no sources it runs
`bench/parity/*.bas` and `fixtures/qb/port`, every source and flag set the QB
tests compile (`tools/qb_port_corpus.py`). A source's `.flags` sidecar holds
the options both compilers get. With `--bc`, Python's is `tools/stages.py
--dump` and Rust's `llrm-omf --dump`; with no objects it runs every
`fixtures/omf/*.obj`, and the BC-only emission stage (`asm`) is not compared
until it is ported. Stages are
compared in the order Python wrote them, so the first mismatch is the first
stage the port gets wrong. `frozenset` elements are sorted on both sides: their order is Python's
hash order, not a fact of the compiler.
"""

import os
import sys
import shutil
import argparse
import subprocess
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ("fixtures/c/*.cgs", "fixtures/c/mir/*.cgs", "fixtures/c/parity/*.cgs")
QBFRONT = ROOT / "frontends/qb/target/release/qbfront"
BC_CORPUS = "fixtures/omf/*.obj"
# BC stage forms Rust does not write yet: the BC-only emission.
BC_UNPORTED = ("asm",)
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


def _bc_stage(name: str) -> tuple[int, str]:
    """`s<N>-<form>-<stage>.txt` as (N, form): N passes 99 unpadded, so names do not sort."""
    number, form, _ = name.split("-", 2)
    return int(number[1:]), form


def _stages(python: Path, qb: bool = False, bc: bool = False) -> list[str]:
    """Python's dump files in pipeline order, then write order; the object last.

    Python writes the whole-unit `mir` after every procedure's phases, so
    write order alone would rank the raise after register allocation. A QB
    dump is flat and written in pipeline order: write order is the order.
    A BC dump numbers its stages.
    """
    files = [one for one in python.rglob("*") if one.is_file() and one.name not in (OBJECT, REFUSAL)]
    if bc:
        files = [one for one in files if _bc_stage(one.name)[1] not in BC_UNPORTED]
        files.sort(key=lambda one: _bc_stage(one.name))
    elif qb:
        files.sort(key=lambda one: (one.stat().st_mtime_ns, one.name))
    else:
        files.sort(key=lambda one: (PIPELINE.index(one.relative_to(python).parts[0]), one.stat().st_mtime_ns))
    staged = [str(one.relative_to(python)) for one in files]
    return staged + [one for one in (OBJECT, REFUSAL) if (python / one).exists()]


def compare(python: Path, rust: Path, qb: bool = False, bc: bool = False) -> tuple[int, int, Divergence | None]:
    """How many of Python's stages Rust reproduces, and the first it does not."""
    stages = _stages(python, qb, bc)
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
    """`bench/parity/*.bas`, then every source the QB tests compile."""
    found = sorted(ROOT.glob("bench/parity/*.bas"))
    return found + sorted(one for one in (ROOT / "fixtures/qb/port").glob("*/*") if one.suffix.lower() == ".bas")


def qb_flags(source: Path) -> list[str]:
    """The compiler options in the source's `.flags` sidecar; none without one."""
    sidecar = source.with_suffix(".flags")
    return sidecar.read_text().split() if sidecar.is_file() else []


def oracle_env() -> dict[str, str]:
    """This checkout's qbopt first: a script's own folder does not make the checkout importable."""
    path = os.pathsep.join(filter(None, (str(ROOT), os.environ.get("PYTHONPATH"))))
    return {**os.environ, "PYTHONHASHSEED": "0", "PYTHONPATH": path}


def _fresh(work: Path) -> tuple[Path, Path]:
    """Empty python and rust dump folders: a file left by an earlier run is not this run's stage."""
    made = work / "python", work / "rust"
    for one in made:
        shutil.rmtree(one, ignore_errors=True)
        one.mkdir(parents=True)
    return made


def run_qb(source: Path, work: Path, rust: Path) -> Result:
    python_dir, rust_dir = _fresh(work)
    env = oracle_env()
    if QBFRONT.is_file():
        env.setdefault("QBOPT_QBFRONT", str(QBFRONT))
    python = [sys.executable, "-m"]
    flags = qb_flags(source)
    made = subprocess.run(
        [sys.executable, str(ROOT / "tools/qbstages.py"), str(source), "--output", str(python_dir), *flags],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
    )
    if not made.returncode:
        made = subprocess.run(
            [*python, "qbopt.frontend.qb", str(source), "-o", str(python_dir / OBJECT), *flags],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
        )
    if made.returncode:
        (python_dir / REFUSAL).write_text(_message(made.stderr, ": ") + "\n")
    done = subprocess.run(
        [str(rust), str(source), "--dump", str(rust_dir), "-o", str(rust_dir / OBJECT), *flags],
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
    python_dir, rust_dir = _fresh(work)
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


def _said(done: subprocess.CompletedProcess, prefix: str) -> str:
    """Why a BC run refused: stages.py prints "nothing to raise" rather than raising."""
    if done.stderr.strip():
        return _message(done.stderr, prefix)
    lines = done.stdout.strip().splitlines()
    return lines[-1].strip() if lines else ""


def run_bc(source: Path, work: Path, rust: Path) -> Result:
    python_dir, rust_dir = _fresh(work)
    made = subprocess.run(
        [sys.executable, str(ROOT / "tools/stages.py"), str(source), "--dump", str(python_dir)],
        cwd=ROOT,
        env=oracle_env(),
        capture_output=True,
        text=True,
    )
    if made.returncode:
        (python_dir / REFUSAL).write_text(_said(made, ": ") + "\n")
    done = subprocess.run([str(rust), str(source), "--dump", str(rust_dir)], cwd=ROOT, capture_output=True, text=True)
    if done.returncode:
        (rust_dir / REFUSAL).write_text(_said(done, "llrm-omf: ") + "\n")
    matched, total, first = compare(python_dir, rust_dir, bc=True)
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
    parser.add_argument("--bc", action="store_true", help="BC objects through llrm-omf; no objects: fixtures/omf")
    parser.add_argument("--jobs", type=int, default=1, help="sources compared at once")
    parser.add_argument("--work", type=Path, default=ROOT / "build" / "port_diff")
    args = parser.parse_args(argv)
    sources = list(args.sources)
    if args.corpus:
        sources += sorted(path for pattern in CORPUS for path in ROOT.glob(pattern))
    if args.qb and not sources:
        sources = qb_corpus()
    if args.bc and not sources:
        sources = sorted(ROOT.glob(BC_CORPUS))
    if not sources:
        parser.error("no sources")
    rust = _rust_binary("llrm-qb" if args.qb else "llrm-omf" if args.bc else "llrm-c")

    def one(source: Path) -> Result:
        work = _workdir(args.work, source)
        if args.qb:
            return run_qb(source.resolve(), work, rust)
        if args.bc:
            return run_bc(source.resolve(), work, rust)
        return run(source.resolve(), work, rust, args.opt)

    with ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        results = list(pool.map(one, sources))
    failed = 0
    for source, result in zip(sources, results, strict=True):
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
