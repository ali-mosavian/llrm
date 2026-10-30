"""The reducer keeps the lines the failure needs and nothing else, and refuses a file that does not fail."""
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import reduce as r  # noqa: E402


def test_ddmin_keeps_exactly_the_lines_the_predicate_needs():
    lines = [f"line {i}\n" for i in range(40)]
    need = {"line 7\n", "line 31\n"}
    small = r.reduce(lines, lambda c: need <= set(c))
    assert small == ["line 7\n", "line 31\n"]


def test_blocks_go_before_lines_and_a_dependent_pair_survives_together():
    text = ["define a {\n", "  x\n", "}\n", "\n", "define b {\n", "  y\n", "  z\n", "}\n", "\n", "define c {\n", "  w\n", "}\n"]
    keep = lambda c: any("y" in l for l in c) and any(l.startswith("define b") for l in c) and any(l.strip() == "}" for l in c)  # noqa: E731
    small = r.reduce(text, keep, "blocks")
    assert "".join(small).count("define") == 1 and "  y\n" in small and "  z\n" not in small


def test_a_file_the_command_does_not_fail_on_is_refused(tmp_path):
    f = tmp_path / "a.c"
    f.write_text("int x;\n")
    done = subprocess.run([sys.executable, str(Path(r.__file__)), str(f), "--cmd", "false"], capture_output=True, text=True)
    assert done.returncode == 2 and "not interesting" in done.stderr


def test_the_command_line_reduces_a_file_and_writes_it(tmp_path):
    f = tmp_path / "a.txt"
    f.write_text("".join(f"{i}\n" for i in range(30)) + "NEEDLE\n")
    done = subprocess.run([sys.executable, str(Path(r.__file__)), str(f), "--cmd", "grep -q NEEDLE {}"], capture_output=True, text=True)
    assert done.returncode == 0 and (tmp_path / "a.txt.reduced").read_text() == "NEEDLE\n", done.stdout + done.stderr
