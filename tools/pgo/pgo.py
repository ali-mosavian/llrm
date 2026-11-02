#!/usr/bin/env python3
"""A profile-guided build of llrm-llc, the backend llrm-cc runs (the shipped, `dist` profile): 15-18% fewer instructions to compile, byte-identical output.

    pgo.py build --out DIR [--profile-dir DIR] [--siblings DIR] [--exclude NAME ...]
                   builds llrm-llc with the profile in --profile-dir (made first when missing or stale), into DIR/llrm-llc
    pgo.py stale --profile-dir DIR     exit 0 fresh, 1 stale, 2 none: the policy in docs/working/pgo.md
    pgo.py train BINARY [--exclude NAME ...]    runs the training corpus through an instrumented BINARY

The profile is made by an instrumented build compiling the corpus (tools/pgo/corpus.toml), merged with the llvm-profdata that matches
rustc's LLVM (the rustup component `llvm-tools`), and used by a second build. It is tied to the source it was made from: a function
changed since gets no profile (the build lists them; the job summary shows the count), so a profile is regenerated past MAX_COMMITS
commits to the compiler or MAX_AGE_DAYS, and reused before that. `--exclude` leaves programs out of the training, to measure the gain
on programs the profile never saw.
"""

from __future__ import annotations

import os
import re
import sys
import json
import time
import shutil
import argparse
import tempfile
import tomllib
import subprocess
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "tools"))
import llrmbin  # noqa: E402
MAX_COMMITS = 50
MAX_AGE_DAYS = 14
SOURCE_PATHS = ("crates", "src", "Cargo.toml", "Cargo.lock")
WARNING = re.compile(r"no profile data available for function (\S+)")
HEADER = "#define far\n#define huge\n#define near\n#define pascal\n#define cdecl\n#define __far\n#define _far\n#define __near\n#define _near\n"


def sh(command: list, check: bool = True, **kw) -> subprocess.CompletedProcess:
    return subprocess.run([str(c) for c in command], capture_output=True, text=True, check=check, **kw)


def corpus(root: Path = ROOT, exclude: frozenset[str] = frozenset(), qcport: Path | None = None, driver: bool = False) -> list[tuple[str, list[str], Path, bool]]:
    """(name, compiler arguments before `-o`, source, whether it is wrapped) per training compile: tools/pgo/corpus.toml's globs, at its levels, less `exclude`
    (names without .c). QCport's modules come from `qcport` (its src directory) when given: they are not in this repository."""
    config = tomllib.loads((HERE / "corpus.toml").read_text())
    if driver:  # trained through llrm-cc: not the programs it cannot build yet
        exclude = exclude | frozenset(config["llrm-cc"]["unbuilt"])
    jobs = []
    for pattern in config["bench"]["globs"]:
        for source in sorted(root.glob(pattern)):
            if source.stem in exclude:
                continue
            include = ["-I", str(root / "bench"), "-I", str(source.parent)]
            for level in config["levels"]:
                jobs.append((source.stem, ["-m32", "-mabi=sysv", level, "-march=i486", *include], source, False))
                jobs.append((source.stem, ["-m16", level, "-march=i486", *include], source, False))
    for source in sorted((root / "tests/run/c").glob("*.c")):
        if source.stem in exclude:
            continue
        header = next((line for line in source.read_text().splitlines()[:3] if line.startswith("// flags:")), None)
        for flags in [v.split() for v in header.removeprefix("// flags:").split("|")] if header else [["-O2"]]:
            jobs.append((source.stem, flags, source, False))
    if qcport:
        for source in sorted((qcport / "render").glob("*.c")):
            if any(source.stem.startswith(name) for name in exclude):
                continue
            include = [flag for d in ("", "render", "game", "host", "qgl") for flag in ("-I", str(qcport / d))]
            for level in config["levels"]:
                jobs.append((source.stem, ["-m32", "-mabi=sysv", level, "-march=i486", *include], source, True))
    return jobs


ROWS = {"-m16": "x86_16-wat", "-m32": "x86_32-wat"}


def through_driver(driver: Path, binary: Path, arguments: list[str], source: Path, out: str) -> tuple[list[str], dict[str, str]]:
    """The llrm-cc command (and environment) that compiles `source` with llrm-c's `arguments`, the backend being `binary`: the target a row, not a flag."""
    row = next((ROWS[one] for one in arguments if one in ROWS), ROWS["-m16"])
    return [str(driver), "-c", *(one for one in arguments if one not in ROWS), "-o", out, str(source)], {**os.environ, "LLRM_LLC": str(binary), "LLRM_CC_TARGET": row}


def train(binary: Path, jobs: list[tuple[str, list[str], Path, bool]], driver: Path | None = None) -> tuple[int, list[str]]:
    """(compiles that worked, one line per compile that failed): the caller fails on any, a corpus that does not compile trains less than it says.
    With `driver` (llrm-cc), `binary` is the backend (llrm-llc) it runs."""
    work = Path(tempfile.mkdtemp(prefix="pgo-train-"))

    def one(job: tuple[int, tuple[str, list[str], Path, bool]]) -> str:
        number, (name, arguments, source, wrapped) = job
        if wrapped:  # QCport's modules read as C with the segmented keywords emptied
            wrapped = work / f"{number}-{source.name}"
            wrapped.write_text(HEADER + f'#include "{source}"\n')
            source = wrapped
        # an object, not /dev/null: llrm-c encodes only for `.o`/`.obj`, so the profile of a compile has the encoder in it
        with llrmbin.with_object([str(binary), *arguments, "-o", llrmbin.OBJECT, str(source)]) as command:
            if driver:
                command, env = through_driver(driver, binary, arguments, source, command[command.index("-o") + 1])
                done = sh(command, check=False, env=env)
            else:
                done = sh(command, check=False)
        return "" if done.returncode == 0 else f"{name} {' '.join(arguments)}: " + " ".join((done.stderr or done.stdout).split())[:200]

    try:
        with ThreadPoolExecutor(max_workers=os.cpu_count() or 4) as pool:
            done = list(pool.map(one, enumerate(jobs)))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    return done.count(""), [line for line in done if line]


def profdata() -> Path:
    """The llvm-profdata of rustc's own LLVM: another version cannot read the raw profiles ("raw profile version mismatch")."""
    sysroot = Path(sh(["rustc", "--print", "sysroot"]).stdout.strip())
    host = re.search(r"^host: (\S+)", sh(["rustc", "-vV"]).stdout, re.M)[1]
    found = sysroot / "lib" / "rustlib" / host / "bin" / "llvm-profdata"
    if not found.is_file():
        raise SystemExit("pgo: no llvm-profdata for rustc's LLVM: run `rustup component add llvm-tools`")
    return found


def stale(profile_dir: Path, root: Path = ROOT, now: float | None = None) -> int:
    """0 fresh, 1 stale, 2 none. Stale: made more than MAX_COMMITS commits to the compiler ago, or MAX_AGE_DAYS ago, or from a commit this clone lacks."""
    meta = profile_dir / "profile.json"
    if not (profile_dir / "profile.profdata").is_file() or not meta.is_file():
        return 2
    info = json.loads(meta.read_text())
    if (now or time.time()) - info["created"] > MAX_AGE_DAYS * 86400:
        return 1
    count = sh(["git", "-C", root, "rev-list", "--count", f"{info['commit']}..HEAD", "--", *SOURCE_PATHS], check=False)
    return 1 if count.returncode or int(count.stdout) > MAX_COMMITS else 0


def work_dir(profile_dir: Path) -> Path:
    """Where the builds go, beside the profile directory: that one is cached (a few tens of MB), the targets are gigabytes."""
    return profile_dir.parent / (profile_dir.name + "-work")


def cargo(root: Path, target: Path, flags: str, binary: str = "llrm-llc") -> subprocess.CompletedProcess:
    env = {**os.environ, "CARGO_TARGET_DIR": str(target), "RUSTFLAGS": flags}
    return subprocess.run(["cargo", "build", "--profile", "dist", "--bin", binary], cwd=root, env=env, capture_output=True, text=True)


def make_profile(root: Path, profile_dir: Path, exclude: frozenset[str], qcport: Path | None, binary: str = "llrm-llc") -> None:
    """Instrumented build, the corpus through it, merged into profile_dir/profile.profdata."""
    raw, target = work_dir(profile_dir) / "raw", work_dir(profile_dir) / "instrumented"
    shutil.rmtree(raw, ignore_errors=True)
    raw.mkdir(parents=True)
    built = cargo(root, target, f"-Cprofile-generate={raw}", binary)
    if built.returncode:
        raise SystemExit("pgo: the instrumented build failed:\n" + built.stderr[-2000:])
    ok, failed = train(target / "dist" / binary, corpus(root, exclude, qcport, binary == "llrm-llc"), driver=llrmbin.bin_dir() / "llrm-cc" if binary == "llrm-llc" else None)
    print(f"pgo: trained on {ok} compiles ({len(failed)} failed), {len(list(raw.glob('*.profraw')))} raw profiles", flush=True)
    if failed:
        raise SystemExit("pgo: the training corpus does not compile:\n  " + "\n  ".join(failed[:20]))
    profile_dir.mkdir(parents=True, exist_ok=True)
    merged = sh([profdata(), "merge", "-o", profile_dir / "profile.profdata", *sorted(raw.glob("*.profraw"))], check=False)
    if merged.returncode:
        raise SystemExit("pgo: llvm-profdata merge failed:\n" + merged.stderr[-2000:])
    commit = sh(["git", "-C", root, "rev-parse", "HEAD"]).stdout.strip()
    (profile_dir / "profile.json").write_text(json.dumps({"commit": commit, "created": time.time(), "compiles": ok, "failed": failed}))
    shutil.rmtree(raw, ignore_errors=True)
    shutil.rmtree(target, ignore_errors=True)


def build(root: Path, out: Path, profile_dir: Path, siblings: Path | None, exclude: frozenset[str], qcport: Path | None, binary: str = "llrm-llc") -> list[str]:
    """DIR/BINARY built with the profile (made first when none or stale); the functions the profile has no data for."""
    if stale(profile_dir, root) != 0:
        make_profile(root, profile_dir, exclude, qcport, binary)
    target = work_dir(profile_dir) / "final"
    built = cargo(root, target, f"-Cprofile-use={profile_dir / 'profile.profdata'} -Cllvm-args=-pgo-warn-missing-function", binary)
    if built.returncode:
        raise SystemExit("pgo: the profile-use build failed:\n" + built.stderr[-2000:])
    out.mkdir(parents=True, exist_ok=True)
    shutil.copy2(target / "dist" / binary, out / binary)
    for one in sorted(siblings.iterdir()) if siblings else []:  # the tools identical.py looks for beside a compiler
        if one.is_file() and os.access(one, os.X_OK) and one.name != binary and not (out / one.name).exists():
            (out / one.name).symlink_to(one.resolve())
    return sorted(set(WARNING.findall(built.stderr)))


def summary(missing: list[str], profile_dir: Path) -> str:
    info = json.loads((profile_dir / "profile.json").read_text())
    size = (profile_dir / "profile.profdata").stat().st_size / 1e6
    head = f"PGO: profile of {info['commit'][:12]} ({size:.1f} MB, {(time.time() - info['created']) / 86400:.1f} days old, {info['compiles']} training compiles); {len(missing)} functions have no profile data (the training never ran them, or they changed since)"
    return "\n".join([head, *(f"  {name}" for name in missing[:20]), *([f"  ... and {len(missing) - 20} more"] if len(missing) > 20 else [])])


def parse(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ("build", "stale", "train"):
        one = sub.add_parser(name)
        one.add_argument("--exclude", nargs="*", default=[])
        if name == "build":
            one.add_argument("--out", type=Path, required=True)
            one.add_argument("--siblings", type=Path)
            one.add_argument("--binary", choices=("llrm-llc",), default="llrm-llc", help="the compiler built with the profile; llrm-llc is trained through llrm-cc")
        if name == "train":
            one.add_argument("binary", type=Path)
        else:
            one.add_argument("--profile-dir", type=Path, default=Path(os.environ.get("PGO_DIR", ROOT / "target" / "pgo")))
    args = parser.parse_args(argv)
    for name in ("profile_dir", "out", "siblings", "binary"):  # the builds run in the repository: a relative path is read from there
        if isinstance(getattr(args, name, None), Path):  # `build --binary` is a compiler's name, not a path
            setattr(args, name, getattr(args, name).resolve())
    return args


def main() -> int:
    args = parse()
    qcport = Path(os.environ["QCPORT"]) if os.environ.get("QCPORT") else None
    exclude = frozenset(args.exclude)
    if args.command == "stale":
        return stale(args.profile_dir)
    if args.command == "train":
        ok, failed = train(args.binary, corpus(ROOT, exclude, qcport, args.binary.name == "llrm-llc"), driver=args.binary.with_name("llrm-cc") if args.binary.name == "llrm-llc" else None)
        print(f"trained on {ok} compiles, {len(failed)} failed", *failed, sep="\n  ")
        return 1 if failed else 0
    missing = build(ROOT, args.out, args.profile_dir, args.siblings, exclude, qcport, args.binary)
    text = summary(missing, args.profile_dir)
    print(text)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a").write(text + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
