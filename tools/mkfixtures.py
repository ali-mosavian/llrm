"""
Regenerate fixtures/omf from the suite, and record where each object came from.

The objects are committed; this is what made them. Generating at test time would
make the suite unrunnable for anyone without three DOS toolchains, and would
silently re-baseline itself whenever a toolchain differed. --check recompiles
into a temporary directory and diffs against what is committed, which is what
catches "someone's toolchain is not the toolchain".

The five objects that predate this script are kept, not regenerated: tests
assert exact offsets and an exact fixup count against them, and they are the
only anchors the repo has. Their provenance is lost, and the manifest says so.
"""

import sys
import shutil
import hashlib
import argparse
import subprocess
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parent))

from dosbox import launch
from configs import Config
from configs import CONFIGS
from dosbox import read_dos
from dosbox import dosbox_bin

ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / "suite"
FIXTURES = ROOT / "fixtures" / "omf"
MANIFEST = FIXTURES / "MANIFEST.tsv"

# /Zd is the only switch that adds a record type rather than changing code, so
# it is a variant of a few configurations rather than a column of its own.
VARIANTS = {"": "", "zd": "/Zd"}
VARIANT_CONFIGS = ("v-g3", "p-g2", "q-O")

INHERITED = {
    "jumptable.obj": "unrecorded -- predates this script, provenance lost",
    "pds-g2.obj": "unrecorded -- predates this script, provenance lost",
    "qb45.obj": "unrecorded -- predates this script, provenance lost",
    "vbdos-g2.obj": "unrecorded -- predates this script, provenance lost",
    "vbdos-g3.obj": "unrecorded -- predates this script, provenance lost",
}

COLUMNS = ("file", "sha256", "source", "config", "command", "bc_sha256", "dosbox", "made_by")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def dosbox_version() -> str:
    binary = dosbox_bin()
    if binary is None:
        return "unknown"
    out = subprocess.run([binary, "-version"], capture_output=True, check=False)
    for line in (out.stdout + out.stderr).decode("latin1").splitlines():
        if "DOSBox-X" in line:
            return line.strip()
    return "unknown"


def wanted(configs: list[str], programs: list[str]) -> list[tuple[str, str, Config, str, str]]:
    """(file stem, program, config, variant switches, variant suffix) for each object."""
    out = []
    for tag in configs:
        config = CONFIGS[tag]
        for variant, extra in VARIANTS.items():
            if variant and tag not in VARIANT_CONFIGS:
                continue
            suffix = f"-{variant}" if variant else ""
            for program in programs:
                out.append((f"{program}-{tag}{suffix}", program, config, extra, suffix))
    return out


def dos_names(jobs: list[tuple[str, str, Config, str, str]]) -> dict[str, str]:
    """A DOS 8.3 name per job. Truncating the stem collides: procs-v-g3 and
    procs-v-g3-zd both begin PROCS_V_, and the second build silently overwrote
    the first, so the plain object carried LINNUM."""
    names = {stem: f"F{n:02d}" for n, (stem, *_) in enumerate(jobs)}
    assert len(set(names.values())) == len(jobs), "DOS names must be unique"
    return names


def build(
    into: Path, jobs: list[tuple[str, str, Config, str, str]], names: dict[str, str], timeout: int
) -> dict[str, str]:
    """Compile every job into `into`, and give back the command line each used."""
    commands = {}
    by_config: dict[str, list[tuple[str, str, str]]] = {}
    for stem, program, config, extra, _suffix in jobs:
        switches = f"{config.switches} {extra}".strip()
        by_config.setdefault(config.tag, []).append((stem, program, switches))

    for tag, batch in by_config.items():
        config = CONFIGS[tag]
        work = into / tag
        work.mkdir(parents=True)
        steps = []
        for stem, program, switches in batch:
            shutil.copy(SUITE / f"{program}.bas", work / f"{program.upper()}.BAS")
            commands[stem] = f"BC {switches} {program.upper()}.BAS, {names[stem]}.OBJ;"
            steps.append(f"{config.bc} {switches} {program.upper()}.BAS, {names[stem]}.OBJ; >> BC.OUT")
        run = launch(work, config.mount, steps, timeout=timeout, env={"LIB": r"V:\LIB"})
        if not run.finished:
            raise SystemExit(f"{tag}: BC did not finish\n{read_dos(work, 'BC.OUT')}")
    return commands


def collect(
    into: Path,
    jobs: list[tuple[str, str, Config, str, str]],
    names: dict[str, str],
    commands: dict[str, str],
    out: Path,
) -> list[str]:
    rows = []
    version = dosbox_version()
    for stem, program, config, _extra, _suffix in jobs:
        made = into / config.tag / f"{names[stem]}.OBJ"
        if not made.is_file():
            raise SystemExit(f"{stem}: no object -- {commands[stem]}")
        shutil.copy(made, out / f"{stem}.obj")
        rows.append(
            "\t".join(
                (
                    f"{stem}.obj",
                    digest(out / f"{stem}.obj"),
                    f"suite/{program}.bas",
                    config.tag,
                    commands[stem],
                    digest(config.mount / config.bc[3:].replace("\\", "/")),
                    version,
                    "tools/mkfixtures.py",
                )
            )
        )
    return rows


def write_manifest(rows: list[str]) -> None:
    inherited = [
        "\t".join((name, digest(FIXTURES / name), why, "unknown", "unknown", "unknown", "unknown", "unknown"))
        for name, why in sorted(INHERITED.items())
    ]
    MANIFEST.write_text("\n".join(["\t".join(COLUMNS), *inherited, *sorted(rows)]) + "\n")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="mkfixtures")
    ap.add_argument("--config", action="append", choices=list(CONFIGS))
    ap.add_argument("--prog", action="append")
    ap.add_argument("--check", action="store_true", help="rebuild into a temp dir and diff, changing nothing")
    ap.add_argument("--timeout", type=int, default=300)
    args = ap.parse_args(argv)

    configs = args.config or [tag for tag, config in CONFIGS.items() if config.available]
    programs = args.prog or sorted(p.stem for p in SUITE.glob("*.bas"))
    jobs = wanted(configs, programs)

    with TemporaryDirectory() as temporary:
        into = Path(temporary)
        names = dos_names(jobs)
        commands = build(into, jobs, names, args.timeout)
        target = into / "check" if args.check else FIXTURES
        target.mkdir(parents=True, exist_ok=True)
        rows = collect(into, jobs, names, commands, target)

        if not args.check:
            write_manifest(rows)
            print(f"{len(rows)} objects, {MANIFEST.relative_to(ROOT)} rewritten")
            return 0

        differ = [
            stem
            for stem, *_ in jobs
            if not (FIXTURES / f"{stem}.obj").is_file()
            or (FIXTURES / f"{stem}.obj").read_bytes() != (target / f"{stem}.obj").read_bytes()
        ]
        for stem in differ:
            print(f"differs: {stem}.obj")
        print(f"{len(jobs) - len(differ)} of {len(jobs)} match what is committed")
        return 1 if differ else 0


if __name__ == "__main__":
    sys.exit(main())
