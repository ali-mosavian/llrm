"""Moving a tool one directory down broke its way to the repository root: the arm64 tools looked for `crates/` under `tools/`."""
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent


def test_python_tools_find_the_repository_root():
    """Read from the source: arm64_fuzz runs main() on import."""
    for name in ("arm64_compare.py", "arm64_fuzz.py"):
        found = re.search(r"^ROOT = Path\(__file__\)\.resolve\(\)\.parents\[(\d)\]$", (HERE.parent / name).read_text(), re.M)
        assert found, name
        assert (HERE.parent.parents[int(found.group(1)) - 1] / "Cargo.toml").is_file(), name


def test_shell_tools_find_the_repository_root():
    for script in sorted(HERE.parent.glob("*.sh")):
        found = re.search(r'root=\$\(cd "\$\(dirname "\$0"\)((?:/\.\.)+)"', script.read_text())
        if found:
            assert (script.parent / found.group(1).lstrip("/") / "Cargo.toml").is_file(), script.name
