"""
Regenerate tests/fixtures/omf from the suite, and record where each object came from.

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
from configs import switches_for

ROOT = Path(__file__).resolve().parents[2]
SUITE = ROOT / "tests/suite"
FIXTURES = ROOT / "tests/fixtures" / "omf"
MANIFEST = FIXTURES / "manifest.tsv"

# /Zd and /Zi add records rather than changing code, so each is a variant of
# a few configurations rather than a column of its own. /Zi is what makes BC
# write $$SYMBOLS and $$TYPES -- every variable's name, type and address,
# every procedure's signature, and each parameter and local with its own bp
# offset. It is on every configuration now, so what this variant produces is
# an anchor that carries them where the older committed objects do not.
VARIANTS = {"": "", "zd": "/Zd", "zi": "/Zi"}
VARIANT_CONFIGS = ("v-g3", "p-g2", "q-O")

INHERITED = {
    "jumptable.obj": "unrecorded -- predates this script, provenance lost",
    "pds-g2.obj": "unrecorded -- predates this script, provenance lost",
    "qb45.obj": "unrecorded -- predates this script, provenance lost",
    "vbdos-g2.obj": "unrecorded -- predates this script, provenance lost",
    "vbdos-g3.obj": "unrecorded -- predates this script, provenance lost",
}

COLUMNS = ("file", "sha256", "source", "config", "command", "bc_sha256", "dosbox", "made_by")


def artifact_name(stem: str) -> str:
    """Committed object-file name; configuration tags remain case-sensitive metadata."""
    return f"{stem.lower()}.obj"


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
        switches = f"{switches_for(config, program)} {extra}".strip()
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
        artifact = artifact_name(stem)
        shutil.copy(made, out / artifact)
        rows.append(
            "\t".join(
                (
                    artifact,
                    digest(out / artifact),
                    f"tests/suite/{program}.bas",
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
    """Merge this run's rows into the manifest, keeping every other one.

    A --prog or --config run builds a slice of the corpus, and writing only
    that slice erases the provenance of everything it did not build. The
    objects stay on disk; the record of which compiler made them, on which
    switches, does not. Adding tests/suite/fpdeep.bas silently dropped all fifteen
    fpemu rows this way before the merge was here.

    A rebuilt object replaces its old row -- the file name is the key, so the
    newest build of a name is what the manifest describes.
    """
    inherited = [
        "\t".join((name, digest(FIXTURES / name), why, "unknown", "unknown", "unknown", "unknown", "unknown"))
        for name, why in sorted(INHERITED.items())
    ]
    kept = {row.split("\t")[0]: row for row in _existing()}
    kept.update({row.split("\t")[0]: row for row in inherited + rows})
    on_disk = {path.name for path in FIXTURES.glob("*.obj")}
    MANIFEST.write_text(
        "\n".join(["\t".join(COLUMNS), *sorted(row for name, row in kept.items() if name in on_disk)]) + "\n"
    )


def _existing() -> list[str]:
    if not MANIFEST.exists():
        return []
    head, *rest = MANIFEST.read_text().splitlines()
    return [row for row in rest if row.strip()]


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
            if not (FIXTURES / artifact_name(stem)).is_file()
            or (FIXTURES / artifact_name(stem)).read_bytes()
            != (target / artifact_name(stem)).read_bytes()
        ]
        for stem in differ:
            print(f"differs: {artifact_name(stem)}")
        print(f"{len(jobs) - len(differ)} of {len(jobs)} match what is committed")
        return 1 if differ else 0


if __name__ == "__main__":
    sys.exit(main())
