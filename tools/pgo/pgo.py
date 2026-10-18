#!/usr/bin/env python3
"""A profile-guided build of llrm-c (the shipped, `dist` profile): 15-18% fewer instructions to compile, byte-identical output.

    pgo.py build --out DIR [--profile-dir DIR] [--siblings DIR] [--exclude NAME ...]
                   builds llrm-c with the profile in --profile-dir (made first when missing or stale), into DIR/llrm-c
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

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import corpora  # noqa: E402

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
MAX_COMMITS = 50
MAX_AGE_DAYS = 14
SOURCE_PATHS = ("crates", "src", "Cargo.toml", "Cargo.lock")
WARNING = re.compile(r"no profile data available for function (\S+)")
HEADER = "#define far\n#define huge\n#define near\n#define pascal\n#define cdecl\n#define __far\n#define _far\n#define __near\n#define _near\n"


def sh(command: list, check: bool = True, **kw) -> subprocess.CompletedProcess:
    return subprocess.run([str(c) for c in command], capture_output=True, text=True, check=check, **kw)


def corpus(root: Path = ROOT, exclude: frozenset[str] = frozenset(), qcport: Path | None = None) -> list[tuple[str, list[str], Path, bool]]:
    """(name, compiler arguments before `-o`, source, whether it is wrapped) per training compile: tools/pgo/corpus.toml's globs, at its levels, less `exclude`
    (names without .c). QCport's modules come from `qcport` (its src directory) when given: they are not in this repository."""
    config = tomllib.loads((HERE / "corpus.toml").read_text())
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


def train(binary: Path, jobs: list[tuple[str, list[str], Path, bool]]) -> tuple[int, list[str]]:
    """(compiles that worked, one line per compile that failed): the caller fails on any, a corpus that does not compile trains less than it says."""
    work = Path(tempfile.mkdtemp(prefix="pgo-train-"))

    def one(job: tuple[int, tuple[str, list[str], Path, bool]]) -> str:
        number, (name, arguments, source, wrapped) = job
        if wrapped:  # QCport's modules read as C with the segmented keywords emptied
            wrapped = work / f"{number}-{source.name}"
            wrapped.write_text(HEADER + f'#include "{source}"\n')
            source = wrapped
        done = sh([binary, *arguments, "-o", "/dev/null", source], check=False)
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


def cargo(root: Path, target: Path, flags: str) -> subprocess.CompletedProcess:
    env = {**os.environ, "CARGO_TARGET_DIR": str(target), "RUSTFLAGS": flags}
    return subprocess.run(["cargo", "build", "--profile", "dist", "--bin", "llrm-c"], cwd=root, env=env, capture_output=True, text=True)


def make_profile(root: Path, profile_dir: Path, exclude: frozenset[str], qcport: Path | None) -> None:
    """Instrumented build, the corpus through it, merged into profile_dir/profile.profdata."""
    raw, target = work_dir(profile_dir) / "raw", work_dir(profile_dir) / "instrumented"
    shutil.rmtree(raw, ignore_errors=True)
    raw.mkdir(parents=True)
    built = cargo(root, target, f"-Cprofile-generate={raw}")
    if built.returncode:
        raise SystemExit("pgo: the instrumented build failed:\n" + built.stderr[-2000:])
    ok, failed = train(target / "dist" / "llrm-c", corpus(root, exclude, qcport))
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


def build(root: Path, out: Path, profile_dir: Path, siblings: Path | None, exclude: frozenset[str], qcport: Path | None) -> list[str]:
    """DIR/llrm-c built with the profile (made first when none or stale); the functions the profile has no data for."""
    if stale(profile_dir, root) != 0:
        make_profile(root, profile_dir, exclude, qcport)
    target = work_dir(profile_dir) / "final"
    built = cargo(root, target, f"-Cprofile-use={profile_dir / 'profile.profdata'} -Cllvm-args=-pgo-warn-missing-function")
    if built.returncode:
        raise SystemExit("pgo: the profile-use build failed:\n" + built.stderr[-2000:])
    out.mkdir(parents=True, exist_ok=True)
    shutil.copy2(target / "dist" / "llrm-c", out / "llrm-c")
    for one in sorted(siblings.iterdir()) if siblings else []:  # the tools identical.py looks for beside a compiler
        if one.is_file() and os.access(one, os.X_OK) and one.name != "llrm-c" and not (out / one.name).exists():
            (out / one.name).symlink_to(one.resolve())
    return sorted(set(WARNING.findall(built.stderr)))


def summary(missing: list[str], profile_dir: Path) -> str:
    info = json.loads((profile_dir / "profile.json").read_text())
    size = (profile_dir / "profile.profdata").stat().st_size / 1e6
    head = f"PGO: profile of {info['commit'][:12]} ({size:.1f} MB, {(time.time() - info['created']) / 86400:.1f} days old, {info['compiles']} training compiles); {len(missing)} functions have no profile data (the training never ran them, or they changed since)"
    return "\n".join([head, *(f"  {name}" for name in missing[:20]), *([f"  ... and {len(missing) - 20} more"] if len(missing) > 20 else [])])


def main() -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ("build", "stale", "train"):
        one = sub.add_parser(name)
        one.add_argument("--exclude", nargs="*", default=[])
        if name == "build":
            one.add_argument("--out", type=Path, required=True)
            one.add_argument("--siblings", type=Path)
        if name == "train":
            one.add_argument("binary", type=Path)
        else:
            one.add_argument("--profile-dir", type=Path, default=Path(os.environ.get("PGO_DIR", ROOT / "target" / "pgo")))
    args = parser.parse_args()
    for name in ("profile_dir", "out", "siblings", "binary"):  # the builds run in the repository: a relative path is read from there
        if getattr(args, name, None):
            setattr(args, name, getattr(args, name).resolve())
    qcport = corpora.optional("qcport")
    exclude = frozenset(args.exclude)
    if args.command == "stale":
        return stale(args.profile_dir)
    if args.command == "train":
        ok, failed = train(args.binary, corpus(ROOT, exclude, qcport))
        print(f"trained on {ok} compiles, {len(failed)} failed", *failed, sep="\n  ")
        return 1 if failed else 0
    missing = build(ROOT, args.out, args.profile_dir, args.siblings, exclude, qcport)
    text = summary(missing, args.profile_dir)
    print(text)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a").write(text + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
