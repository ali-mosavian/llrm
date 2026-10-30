"""regen-target.sh's symbol list for a record."""

import re
import subprocess
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "regen-target.sh"


def symbols(record, tmp_path):
    """The lines `field registers | sed ...` feeds into symbols.cpp, run as the script has them."""
    text = SCRIPT.read_text()
    define = next(line for line in text.splitlines() if line.startswith("field()"))
    stage = re.search(r'field registers \| sed "[^"]*"', text).group(0)
    rec = tmp_path / "dagisel.toml"
    rec.write_text(record)
    run = f'REC={rec}; NS="Ns::"; {define}\n{stage}'
    out = subprocess.run(["bash", "-c", run], capture_output=True, text=True, check=True).stdout
    return [line for line in out.splitlines() if line]  # the script's `grep .` drops blank lines


def test_a_record_without_registers_adds_no_bare_namespace(tmp_path):
    """`Ns::` alone reached ids.cpp as a symbol and it did not compile."""
    assert symbols('[target]\nns = "Ns::"\n', tmp_path) == []


def test_each_register_gets_the_namespace(tmp_path):
    assert symbols('[target]\nregisters = ["A", "B"]\n', tmp_path) == ["Ns::A", "Ns::B"]
