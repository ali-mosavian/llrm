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
from collections.abc import Callable

sys.path.insert(0, str(Path(__file__).resolve().parent))

from configs import Config
from configs import CONFIGS
from dosbox import read_dos
from configs import DIVERGES
from cache import cached_launch
from configs import switches_for
from cache import toolchain_identity

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


# fixmul declares a routine nothing defines: the differential needs a base BC
# alone can link, and no library ever provides FIXMUL. tests/test_fix_multiply
# covers it, without a base, the way its own docstring explains.
NO_BASE = {"fixmul"}


def programs() -> list[str]:
    return sorted(p.stem for p in SUITE.glob("*.bas") if p.stem not in NO_BASE)


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
    cached_launch(
        work,
        cfg.mount,
        [f"{cfg.bc} {switches_for(cfg, n)} {n.upper()}.BAS, {n.upper()}.OBJ; >> BC.OUT" for n in names],
        identity=toolchain_identity(cfg),
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
    cached_launch(work, cfg.mount, steps, identity=toolchain_identity(cfg), timeout=timeout, env={"LIB": r"V:\LIB"})


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

    diverges = name in DIVERGES
    if base != golden and not diverges:
        return Verdict(name, "BASEDIFF", first_difference(golden, base))
    for who, out in (("baseline", base), ("rewritten", opt)):
        if not out or out[-1] != "DONE":
            return Verdict(name, "NODONE", f"the {who} run stopped early")
    # where the rewrite is meant to disagree with BC, the golden is the only
    # thing worth comparing against
    want = golden if diverges else base
    if opt != want:
        return Verdict(name, "DIFF", first_difference(want, opt))
    return Verdict(name, "PASS", f"{len(want) - 1} cases")


def through_qbopt(data: bytes) -> bytes:
    return rewrite(data, dry_run=False)[0]


def run(
    tag: str,
    only: str | None = None,
    *,
    dry_run: bool = False,
    timeout: int = 300,
    transform: Callable[[bytes], bytes] | None = None,
    work: Path | None = None,
) -> Result:
    cfg = CONFIGS[tag]
    if not cfg.available:
        raise SystemExit(f"no toolchain at {cfg.mount}; see docs/testing.md")

    names = [only] if only else programs()
    # BUILD/tag is this function's own default and is owned by the one
    # caller that never passes `work` -- a second caller sharing a tag but
    # wanting a different program set or transform must pass its own, or two
    # pytest-xdist workers racing on the same directory delete each other's
    # objects mid-run
    work = work or BUILD / tag
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)

    compile_all(cfg, work, names, timeout)
    for name in names:
        obj = work / f"{name.upper()}.OBJ"
        if obj.is_file():
            change = transform or (lambda data: rewrite(data, dry_run=dry_run)[0])
            (work / f"{name.upper()}Q.OBJ").write_bytes(change(obj.read_bytes()))
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
