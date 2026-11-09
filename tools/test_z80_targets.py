"""The z80 runner skips the programs whose `// targets:` line leaves the z80 out."""

import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUN = ROOT / "crates/target/llrm-z80/tests/programs/run.py"


def runner():
    spec = importlib.util.spec_from_file_location("z80_run", RUN)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_a_program_for_other_targets_is_not_run_on_the_z80(tmp_path):
    """Before: stackover (`// targets: x86-m16 x86-m32`) ran unbounded recursion on ucsim to the timeout, 30 min in a sweep."""
    for name, header in (("plain", ""), ("x86only", "// targets: x86-m16 x86-m32\n"), ("dosonly", "// targets: dos\n"), ("tagged", "// targets: z80-sdcc\n")):
        (tmp_path / f"{name}.c").write_text(header + "int main(void) { return 0; }\n")
        (tmp_path / f"{name}.out").write_text("")
    assert [p.stem for p in runner().selected(tmp_path)] == ["plain", "tagged"]
    assert [p.stem for p in runner().selected(tmp_path, ["x86only", "plain"])] == ["plain"]
