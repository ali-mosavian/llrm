"""tools/callconv/c.sh: a module llrm-cc refuses leaves its cells UNMEASURED; BCC's object is never linked in its place."""
from __future__ import annotations

import os
import subprocess
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "callconv" / "c.sh"
STUB = """#!/bin/bash
# Compiles a module (writes the -o file) when its name matches $STUB_ACCEPT, else refuses.
for a in "$@"; do [[ $prev == -o ]] && out=$a; [[ $a == *.c ]] && src=$a; prev=$a; done
[[ $src =~ $STUB_ACCEPT ]] || { echo "refused $src" >&2; exit 1; }
echo "llrm $src" > "$out"
"""
QUIET = "#!/bin/bash\nexit 0\n"


def run(tmp_path: Path, accept: str) -> tuple[subprocess.CompletedProcess, Path]:
    bins = tmp_path / "bin"
    bins.mkdir()
    for name, text in (("llrm-cc", STUB), ("jwasm", QUIET), ("jwlink", QUIET), ("dosbox-x", QUIET)):
        (bins / name).write_text(text)
        (bins / name).chmod(0o755)
    work = tmp_path / "work"
    env = {**os.environ, "LLRM_BIN": str(bins), "DOSRUN": str(bins / "dosbox-x"), "BCLIB": str(tmp_path), "STUB_ACCEPT": accept}
    return subprocess.run([str(SCRIPT), str(work)], capture_output=True, text=True, env=env), work


def test_a_refusing_llrm_cc_leaves_every_llrm_cell_unmeasured_not_bcc_against_bcc(tmp_path):
    """llrm-cc, given no target, refused all 16 modules and c.sh linked BCC's own objects as 'LL': the matrix passed with 0 llrm code in it."""
    done, work = run(tmp_path, "^$")
    assert done.returncode != 0 and "llrm compiled 0/16" in done.stderr
    for k in ("CF", "CN", "PF", "PN"):
        for pair in ("LB", "BL", "LL"):
            assert (work / f"{k}{pair}.UNMEASURED").is_file(), f"{k}{pair}"
            assert not (work / f"{k}{pair}.lnk").exists(), f"{k}{pair} linked with a module llrm-cc refused"
        assert (work / f"{k}BB.lnk").is_file() and not (work / f"{k}BB.UNMEASURED").exists()
    assert (work / "MODULES.tsv").read_text().count("REFUSED") == 16


def test_a_cell_is_measured_when_every_llrm_module_it_names_compiled(tmp_path):
    """Callers and aggers compile: LB links llrm's objects, BL and LL (which need the callees too) are unmeasured; the exit status still says a module was refused."""
    done, work = run(tmp_path, r"(caller|agger)\.c$")
    assert done.returncode != 0 and "llrm compiled 8/16" in done.stderr
    for k in ("CF", "PF"):
        assert f"{k.lower()}caller.llrm.obj" in (work / f"{k}LB.lnk").read_text()
        for pair in ("BL", "LL"):
            assert (work / f"{k}{pair}.UNMEASURED").is_file() and not (work / f"{k}{pair}.lnk").exists()


def test_llrm_callers_are_armed_without_the_si_di_check_and_bcc_references_are_not_relaxed(tmp_path):
    """-DARMED on llrm callers alone made every llrm caller that copies a struct with `rep movsd` report "lost 0003" (SI+DI) for a callee that kept them: llrm's own code legally uses SI/DI between arm() and verify()."""
    seen = tmp_path / "flags"
    stub = STUB.replace('echo "llrm $src" > "$out"', 'echo "llrm $src" > "$out"; echo "$src $*" >> "$SEEN"')
    bins = tmp_path / "bin"
    bins.mkdir()
    for name, text in (("llrm-cc", stub), ("jwasm", QUIET), ("jwlink", QUIET), ("dosbox-x", QUIET)):
        (bins / name).write_text(text)
        (bins / name).chmod(0o755)
    env = {**os.environ, "LLRM_BIN": str(bins), "DOSRUN": str(bins / "dosbox-x"), "BCLIB": str(tmp_path), "STUB_ACCEPT": ".", "SEEN": str(seen)}
    subprocess.run([str(SCRIPT), str(tmp_path / "work")], capture_output=True, text=True, env=env)
    lines = seen.read_text().splitlines()
    for line in lines:
        armed = line.split()[0].endswith(("caller.c", "agger.c"))
        assert ("-DARMED -DARMED_NO_SIDI" in line) == armed, line
    assert len(lines) == 16
    reference = (SCRIPT.parent / "reference.sh").read_text()
    assert "-DARMED -r-" in reference and "NO_SIDI" not in reference


def test_the_probe_has_a_verify_that_skips_si_and_di_and_keeps_sp_bp_ds_df():
    """verify_nosidi must clear exactly bits 0 and 1 of the lost mask before it reports: SP (bit 3), BP, DS and DF still fail."""
    probe = (SCRIPT.parents[2] / "tests/fixtures/callconv/c/probe.asm").read_text()
    nosidi = probe[probe.index("_verify_nosidi proc far"):probe.index("_verify_nosidi endp")]
    assert "and cx, 0FFFCh" in nosidi and nosidi.index("and cx, 0FFFCh") < nosidi.index("jcxz")
    for check in ("cmp bp, savedbp", "cmp dx, savedsp", "test ax, 400h"):
        assert check in nosidi, check
