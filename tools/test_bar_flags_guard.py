"""Every external-bar builder takes its flags from tools/external_flags.py.

Bars were compiled with each harness's own list (frame pointer, endbr64, canary, PIE, i686 at m32 against our i486, a distro gcc 13.4 for the
gcc-15 baseline) and a gcc fallback hid refused modules: the byte and clock bars read wrong in qbbars, the QB clock harness and the m68k column.
A script that carries a bar flag but neither reads external_flags nor is listed in DEBT is red; DEBT is the known set, each entry owned and dated:
fixing one removes it, and a stale entry (the script now reads external_flags) is red too.
"""

import os
import re

TOOLS = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(TOOLS)
BAR_FLAG = re.compile(r"-fno-stack-protector|-fcf-protection|-fomit-frame-pointer")
SCANNED = ("tools", "runtime/compiler-rt/tests")

# path -> owner and exit date. Each builds a bar with its own flags.
DEBT = {
    "tools/cc_cost.py": "unassigned, 2026-12-02",
    "tools/compile_map.py": "unassigned, 2026-12-02",
    "tools/scoreboard.py": "unassigned, 2026-12-02",
    "tools/softfloat_cells/rows_bytes.sh": "unassigned, 2026-12-02",
    "tools/wasm_corpus.sh": "unassigned, 2026-12-02",
    "tools/wasm_flag_check.py": "unassigned, 2026-12-02",
    "tools/z80_flag_check.py": "unassigned, 2026-12-02",
    "runtime/compiler-rt/tests/size_x86.sh": "unassigned, 2026-12-02",
}


def builders():
    for top in SCANNED:
        for here, _dirs, names in os.walk(os.path.join(ROOT, top)):
            for name in names:
                if name.endswith((".py", ".sh")) and not name.startswith("test_"):
                    path = os.path.join(here, name)
                    text = open(path, errors="replace").read()
                    if BAR_FLAG.search(text):
                        yield os.path.relpath(path, ROOT), "external_flags" in text


def test_every_bar_builder_reads_the_shared_flags_or_is_owed():
    loose = sorted(path for path, shared in builders() if not shared and path not in DEBT)
    assert not loose, f"build bars with their own flags, not tools/external_flags.py (fix, or list in DEBT with owner and date): {loose}"


def test_a_debt_entry_is_removed_when_the_script_is_fixed():
    found = dict(builders())
    stale = sorted(path for path in DEBT if found.get(path, False) or path not in found)
    assert not stale, f"fixed or gone, remove from DEBT: {stale}"
