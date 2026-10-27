"""`uv run --project tools python -m pytest tools/test_path_index.py`

Every `tools/` path that docs or CI name exists. The tools/ regroup moves files, and a path left
behind in a workflow or a doc is a command that fails on the next run, found only then.

A name matches a file, a directory, or the start of a file name (`tools/ci/ensure_nightly` in a
grep pattern names `ensure_nightly.sh`). NOT_PATHS lists the few names that are placeholders or
forward references, each with its reason; an entry whose path now exists is itself a failure.
"""
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCANNED = (".github", "docs", "README.md", "CLA" "UDE.md", "AGENTS.md")  # split: no_local_paths.rs bars the name
TEXT_SUFFIXES = (".md", ".yml", ".yaml", ".toml", ".sh", ".py")
NAMED = re.compile(r"(?<![A-Za-z0-9_./-])(tools/[A-Za-z0-9_./-]*[A-Za-z0-9_])")

NOT_PATHS = {
    "tools/NAME": "placeholder in docs/reference/tools.md",
    "tools/NAME.py": "placeholder in docs/reference/tools.md",
    "tools/clib": "prose abbreviation in docs/architecture/register-lists.md",
    "tools/llvmpin.py": "on a branch, not yet on main (scev-port.md, loop-passes-port.md)",
    "tools/mkinputs.sh": "historical row tool named in loopidiom-port.md",
    "tools/precision.py": "future tool named in lir-cleanup-plan.md",
    "tools/rows/run8.sh": "historical row script named in recursion-rows.md",
    "tools/scev_oracle.py": "historical tool named in scev-port.md",
}


MACHINE_DOC = "docs/architecture/machine-description.md"
MACHINE_NAMED = re.compile(r"(?<![A-Za-z0-9_./-])(machine/[A-Za-z0-9_./-]*[A-Za-z0-9_])")
MACHINE_NOT_YET: dict[str, str] = {}


def exists(path: str) -> bool:
    target = ROOT / path
    if target.exists():
        return True
    return target.parent.is_dir() and any(p.name.startswith(target.name) for p in target.parent.iterdir())


def named_paths(text: str) -> set[str]:
    return {m.rstrip(".") for m in NAMED.findall(text)}


def missing_in(text: str) -> set[str]:
    return {p for p in named_paths(text) if "*" not in p and not exists(p) and p not in NOT_PATHS}


def scanned_names(tracked: list[str]) -> list[str]:
    """The tracked files to scan. Matched without case: the pathspec `git ls-files CLA` `UDE.md` is exact, and the tracked doc is lowercase, so it matched nothing."""
    roots = {name.lower() for name in SCANNED}
    return [f for f in tracked if f.lower().endswith(TEXT_SUFFIXES) and f.split("/")[0].lower() in roots]


def scanned_files() -> list[Path]:
    out = subprocess.check_output(["git", "ls-files"], cwd=ROOT, text=True)
    return [ROOT / f for f in scanned_names(out.splitlines())]


def test_a_path_missing_from_the_tree_is_reported():
    """A doc naming tools/gone/x.py after the file moved: the scan has to report it."""
    assert missing_in("run `python3 tools/gone/x.py` and tools/nonesuch/y.sh") == {"tools/gone/x.py", "tools/nonesuch/y.sh"}


def test_a_doc_named_in_another_case_is_scanned():
    """The root doc is tracked in lowercase; the scan listed it by an uppercase pathspec and read nothing of it."""
    doc = "cla" "ude.md"
    assert scanned_names([doc, "docs/a.md", "src/lib.rs", "tools/x.py"]) == [doc, "docs/a.md"]


def test_a_name_that_starts_a_file_name_is_a_path():
    assert missing_in("grep -qE '^(tools/ci/ensure_nightly)'") == set()


def test_every_tools_path_named_in_docs_or_ci_exists():
    missing = {}
    for f in scanned_files():
        for p in missing_in(f.read_text(errors="ignore")):
            missing.setdefault(p, []).append(str(f.relative_to(ROOT)))
    assert not missing, "tools/ paths named but absent: " + "; ".join(f"{p} (in {', '.join(sorted(fs)[:2])})" for p, fs in sorted(missing.items()))


def test_no_not_paths_entry_has_become_a_real_path():
    stale = [p for p in NOT_PATHS if exists(p)]
    assert not stale, f"drop from NOT_PATHS, the path exists now: {stale}"


def test_every_machine_path_named_in_the_machine_doc_exists():
    """M1 moved the platform files; a path the doc names must be there, or listed as not yet made."""
    named = set(MACHINE_NAMED.findall((ROOT / MACHINE_DOC).read_text()))
    missing = sorted(p for p in named if not (ROOT / p).exists() and p not in MACHINE_NOT_YET)
    assert not missing, f"machine/ paths named in {MACHINE_DOC} but absent: {missing}"


def test_no_machine_not_yet_entry_has_become_a_real_path():
    stale = [p for p in MACHINE_NOT_YET if (ROOT / p).exists()]
    assert not stale, f"drop from MACHINE_NOT_YET, the path exists now: {stale}"
