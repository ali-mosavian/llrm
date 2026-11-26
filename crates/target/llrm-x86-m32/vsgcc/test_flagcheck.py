"""vsgcc flag parity: a flag on one side only is refused, not measured (arm64 shipped 24 of 72 cells off by 4..16 B that way)."""
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import flagcheck as f  # noqa: E402
import external_flags  # noqa: E402

BASE = [*external_flags.flags("x86", ("-fno-pic",)), "-m32", "-march=i486", "-Dfar=", "-O1"]
GCC = CLANG = BASE
# What llrm-cc's clang ran for an m32 -O1 cell (logged through a clang wrapper, 2026-11-09, x_fir).
OURS = ("-target i486-unknown-linux-gnu -fhosted -fshort-wchar -mlong-double-64 -fno-pic -fno-stack-protector -fno-asynchronous-unwind-tables "
        "-std=gnu99 -Xclang -fdefault-calling-conv=regcall -fdeclspec -Wno-unknown-attributes -nostdinc -isystem /h -idirafter /i "
        "-mlong-double-80 -O1 -march=i486 -funroll-loops -S -emit-llvm -Xclang -disable-llvm-passes -o /tmp/x.ll x.c")
EXT = [*GCC, "-mlong-double-80"]


def test_the_real_columns_agree():
    assert f.check_external("gcc", "O1", GCC) == ""
    assert f.check_external("clang", "O1", CLANG) == ""


def test_a_flag_on_the_external_only_is_refused():
    assert "ffast-math" in f.check_external("gcc", "O1", GCC + ["-ffast-math"])
    assert "fno-inline" in f.check_external("gcc", "O1", GCC + ["-fno-inline"])


def test_a_wrong_level_or_march_is_refused():
    assert f.check_external("gcc", "O2", GCC)
    assert f.check_external("gcc", "O1", [t for t in GCC if t != "-march=i486"])


def test_ours_and_an_external_agree_at_o1_where_only_documented_settings_differ():
    assert f.check_ours("O1", OURS, EXT) == ""


def test_ours_with_a_flag_the_external_lacks_is_refused():
    assert "ffast-math" in f.check_ours("O1", OURS + " -ffast-math", EXT)
    assert "funroll-loops" in f.check_ours("O2", OURS, EXT), "-funroll-loops is documented at -O1 only"


def test_a_program_using_wchar_t_is_refused_since_the_columns_disagree_on_its_size():
    """-fshort-wchar: 2 bytes in ours, 4 in gcc and clang: the same source is two programs."""
    assert f.uses_wide_chars("wchar_t w[2] = {1, 2};")
    assert f.uses_wide_chars('const wchar_t *s = L"ab";')
    assert not f.uses_wide_chars('report(bench_open_read("NOSUCH.FIL"));')
    assert not f.uses_wide_chars("int FILL = 1; long BULL(void);")


def test_a_column_aligning_its_calls_unlike_ours_is_refused(tmp_path):
    """The table built the bars at a 16-byte stack beside ours at 4: they stored arguments into a reserved frame where ours
    pushed, and hundreds of cells read as lost to the compiler."""
    (tmp_path / "p.c").write_text(f.PROBE)
    flags = ["gcc", "-m32", "-march=i486", "-fno-pic", "-O2", "-S", "-masm=intel", "-o", "-", str(tmp_path / "p.c")]
    sixteen = subprocess.run(flags, capture_output=True, text=True, check=True).stdout
    four = subprocess.run([*flags[:1], "-mpreferred-stack-boundary=2", *flags[1:]], capture_output=True, text=True, check=True).stdout
    ours = "_f proc near\n    pushd 1\n    call _g\n    pop ecx\n    ret\n_f endp\n"
    assert f.check_stack(ours, four) == ""
    assert "4 bytes in ours, 16" in f.check_stack(ours, sixteen)


def test_build_sh_refuses_either_external_whose_calls_align_the_stack_unlike_ours():
    """The check beside the table, not only in the module: a build with a mismatched bar must not produce one."""
    text = (Path(__file__).parent / "build.sh").read_text()
    for bar in ("gcc", "clang"):
        assert f'flagcheck.py" stack stack.$P.llrm.s stack.$P.{bar}.s || fail' in text


def _llrm_lines():
    return [line.strip() for line in (Path(__file__).parent / "build.sh").read_text().splitlines() if "$LLRM --target=x86_32-wat" in line and " -c -o " in line]


def test_every_llrm_compile_of_build_sh_shares_one_flag_set_with_the_parity_check():
    """flagparity.sh compares ours as `--target=x86_32-wat -mabi=sysv -mlong-double-80 -march=i486`; build.sh's llrm lines (the
    table's) must be that set at its defaults, or the verdict is about another command."""
    import re

    lines = _llrm_lines()
    assert len(lines) >= 5, "premise: build.sh compiles llrm at four levels, the ELF build and the probe"
    for line in lines:
        assert re.search(r"--target=x86_32-wat \$\{LLRM_FLAGS:-\} (\$\(HF llrm \$?\w+\) )?-mabi=\$ABI \$LLRM_LD (-\$?\w+ )?-march=i486", line), line
    text = (Path(__file__).parent / "build.sh").read_text()
    assert "ABI=${VSGCC_ABI:-sysv}" in text and "LLRM_LD=${VSGCC_LLRM_LD--mlong-double-80}" in text
    parity = (Path(__file__).parent / "flagparity.sh").read_text()
    assert "--target=x86_32-wat -mabi=sysv -mlong-double-80 -$o -march=i486" in parity
