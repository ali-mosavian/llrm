"""Moving a tool one directory down broke its way to the repository root: the arm64 tools looked for `crates/` under `tools/`."""
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import arm64_compare  # noqa: E402
import arm64_fuzz  # noqa: E402


def test_python_tools_find_the_repository_root():
    for tool in (arm64_compare, arm64_fuzz):
        assert (tool.ROOT / "Cargo.toml").is_file(), tool.__name__
        assert tool.CRATE.is_dir(), tool.__name__


def test_shell_tools_find_the_repository_root():
    for script in sorted(HERE.parent.glob("*.sh")):
        found = re.search(r'root=\$\(cd "\$\(dirname "\$0"\)((?:/\.\.)+)"', script.read_text())
        if found:
            assert (script.parent / found.group(1).lstrip("/") / "Cargo.toml").is_file(), script.name
