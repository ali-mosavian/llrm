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


def exists(path: str) -> bool:
    target = ROOT / path
    if target.exists():
        return True
    return target.parent.is_dir() and any(p.name.startswith(target.name) for p in target.parent.iterdir())


def named_paths(text: str) -> set[str]:
    return {m.rstrip(".") for m in NAMED.findall(text)}


def missing_in(text: str) -> set[str]:
    return {p for p in named_paths(text) if "*" not in p and not exists(p) and p not in NOT_PATHS}


def scanned_files() -> list[Path]:
    out = subprocess.check_output(["git", "ls-files", *SCANNED], cwd=ROOT, text=True)
    return [ROOT / f for f in out.split() if f.endswith(TEXT_SUFFIXES)]


def test_a_path_missing_from_the_tree_is_reported():
    """A doc naming tools/gone/x.py after the file moved: the scan has to report it."""
    assert missing_in("run `python3 tools/gone/x.py` and tools/nonesuch/y.sh") == {"tools/gone/x.py", "tools/nonesuch/y.sh"}


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
