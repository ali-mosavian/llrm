"""The paths whose change can change the nib-lsp binary: its crate and the workspace crates it builds from.

    nib_lsp_paths.py          print the list
    nib_lsp_paths.py --write  put it in the `push: paths:` of .github/workflows/nib-lsp-release.yml
"""

import re
import sys
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/nib-lsp-release.yml"
# The release job builds `--no-default-features --bin nib-lsp`: the features are the ones the workflow builds with.
BUILD = ["-p", "llrm", "--no-default-features"]


def paths() -> list[str]:
    tree = subprocess.run(["cargo", "tree", *BUILD, "-e", "normal,build", "--target", "all", "--prefix", "none", "-f", "{p}"], cwd=ROOT, capture_output=True, text=True, check=True).stdout
    dirs = set()
    for line in tree.splitlines():
        if m := re.search(r"\((/[^)]*)\)", line):
            dirs.add(Path(m.group(1)).relative_to(ROOT))
    out = [("**" if d == Path(".") else f"{d}/**") for d in dirs]
    out = sorted({p for p in out if p != "**"} | {"Cargo.toml", "build.rs"})
    # The root crate's other binaries are not nib-lsp: later patterns override earlier ones.
    return out + ["src/**", "!src/bin/**", "src/bin/nib-lsp.rs"]


def listed(text: str) -> list[str]:
    """The `push: paths:` entries of the workflow."""
    block = re.search(r"  push:\n(?:    .*\n)*?    paths:\n((?:      - .*\n)+)", text)
    return re.findall(r'- "([^"]*)"', block.group(1)) if block else []


def main() -> int:
    want = paths()
    if "--write" in sys.argv:
        text = WORKFLOW.read_text()
        block = "".join(f'      - "{p}"\n' for p in want)
        text = re.sub(r"(    paths:\n)((?:      - .*\n)+)", lambda m: m.group(1) + block, text, count=1)
        WORKFLOW.write_text(text)
    else:
        print("\n".join(want))
    return 0


if __name__ == "__main__":
    sys.exit(main())
