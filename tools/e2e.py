"""
BC -> .OBJ -> qbopt -> .OBJ' -> LINK -> .EXE -> run -> compare, for one
configuration.

Two DOSBox launches, not two per program: one compiles the whole suite, the
host rewrites every object in milliseconds, and one links and runs both builds.
A launch costs a second or two of startup, so per-program launches would make
the matrix mostly overhead.

The differential is three-way. golden is what the program means, authored by
tools/mkgolden.py. base is BC's own object, linked and run. opt is the
rewritten one. base != golden is not the pass -- it is a compiler difference or
a wrong expectation, and it fails the run anyway, because an unexplained base
is not a base.
"""

import sys
import shutil
import argparse
from pathlib import Path
from typing import NamedTuple
from dataclasses import dataclass

sys.path.insert(0, str(Path(__file__).resolve().parent))

from dosbox import launch
from configs import Config
from configs import CONFIGS
from dosbox import read_dos

from qbopt.rewrite import rewrite

ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / "suite"
BUILD = ROOT / "build" / "e2e"


class Verdict(NamedTuple):
    program: str
    status: str
    detail: str

    @property
    def ok(self) -> bool:
        return self.status == "PASS"


@dataclass(frozen=True, slots=True)
class Result:
    tag: str
    verdicts: list[Verdict]

    @property
    def ok(self) -> bool:
        return all(v.ok for v in self.verdicts)


def programs() -> list[str]:
    return sorted(p.stem for p in SUITE.glob("*.bas"))


def lines(text: str) -> list[str]:
    return [ln.rstrip() for ln in text.replace("\r\n", "\n").split("\n") if ln.strip()]


def first_difference(want: list[str], got: list[str]) -> str:
    for i in range(max(len(want), len(got))):
        w = want[i] if i < len(want) else "<end>"
        g = got[i] if i < len(got) else "<end>"
        if w != g:
            return f"line {i + 1}: want {w!r}, got {g!r}"
    return ""


def compile_all(cfg: Config, work: Path, names: list[str], timeout: int) -> None:
    for name in names:
        shutil.copy(SUITE / f"{name}.bas", work / f"{name.upper()}.BAS")
    launch(
        work,
        cfg.mount,
        [f"{cfg.bc} {cfg.switches} {n.upper()}.BAS, {n.upper()}.OBJ; >> BC.OUT" for n in names],
        timeout=timeout,
        env={"LIB": r"V:\LIB"},
    )


def link_and_run(cfg: Config, work: Path, names: list[str], timeout: int) -> None:
    steps = []
    for n in names:
        u = n.upper()
        steps += [
            f"{cfg.link} {u}.OBJ, B_{u}.EXE,, {cfg.runtime}; >> LINK.OUT",
            f"{cfg.link} {u}Q.OBJ, O_{u}.EXE,, {cfg.runtime}; >> LINK.OUT",
            f"B_{u}.EXE > B_{u}.TXT",
            f"O_{u}.EXE > O_{u}.TXT",
        ]
    launch(work, cfg.mount, steps, timeout=timeout, env={"LIB": r"V:\LIB"})


def judge(work: Path, name: str) -> Verdict:
    u = name.upper()
    obj = work / f"{u}.OBJ"
    if not obj.is_file():
        errs = [ln for ln in lines(read_dos(work, "BC.OUT")) if "rror" in ln or "arning" in ln]
        return Verdict(name, "BCFAIL", "; ".join(errs[-3:]) or "no object produced")

    link = read_dos(work, "LINK.OUT").lower()
    # LINK emits an .EXE even with unresolved externals, patching the call site
    # to an int 3. "Did an exe appear" is not a link check.
    if "unresolved external" in link or "error l" in link:
        bad = [ln for ln in lines(read_dos(work, "LINK.OUT")) if "rror" in ln.lower() or "unresolved" in ln.lower()]
        return Verdict(name, "LINKFAIL", "; ".join(bad[:3]))

    base, opt = lines(read_dos(work, f"B_{u}.TXT")), lines(read_dos(work, f"O_{u}.TXT"))
    if not base:
        return Verdict(name, "RUNFAIL", "the baseline produced no output")
    golden = lines((SUITE / "golden" / f"{name}.txt").read_text())

    if base != golden:
        return Verdict(name, "BASEDIFF", first_difference(golden, base))
    for who, out in (("baseline", base), ("rewritten", opt)):
        if not out or out[-1] != "DONE":
            return Verdict(name, "NODONE", f"the {who} run stopped early")
    if opt != base:
        return Verdict(name, "DIFF", first_difference(base, opt))
    return Verdict(name, "PASS", f"{len(base) - 1} cases")


def run(tag: str, only: str | None = None, *, dry_run: bool = False, timeout: int = 300) -> Result:
    cfg = CONFIGS[tag]
    if not cfg.available:
        raise SystemExit(f"no toolchain at {cfg.mount}; see docs/testing.md")

    names = [only] if only else programs()
    work = BUILD / tag
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)

    compile_all(cfg, work, names, timeout)
    for name in names:
        obj = work / f"{name.upper()}.OBJ"
        if obj.is_file():
            out, _ = rewrite(obj.read_bytes(), dry_run=dry_run)
            (work / f"{name.upper()}Q.OBJ").write_bytes(out)
    link_and_run(cfg, work, names, timeout)

    return Result(tag, [judge(work, n) for n in names])


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="e2e")
    ap.add_argument("config", choices=list(CONFIGS))
    ap.add_argument("--prog")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--timeout", type=int, default=300)
    args = ap.parse_args(argv)

    result = run(args.config, args.prog, dry_run=args.dry_run, timeout=args.timeout)
    for v in result.verdicts:
        print(f"  {v.program:10} {v.status:9} {v.detail}")
    print(f"{args.config}: {'PASS' if result.ok else 'FAIL'}   (build/e2e/{args.config})")
    return 0 if result.ok else 1


if __name__ == "__main__":
    sys.exit(main())
