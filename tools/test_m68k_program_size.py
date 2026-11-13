"""The whole-program size counts the library members a program references, no others."""
import os
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import m68k_exec_sweep as sweep  # noqa: E402
import m68k_program_size as size  # noqa: E402
import o0_sweep  # noqa: E402

MAP = """
Archive member included to satisfy reference by file (symbol)

lib.a(rt_68000_O2_int_ctzsi2.o)
                              p.o (__ctzsi2)

Linker script and memory map

.text           0x0000000080000094       0x40
 .text          0x0000000080000094       0x24 crt.o
 .text          0x00000000800000b8       0x1c p.o
 .text          0x00000000800000d4       0x1c lib.a(rt_68000_O2_int_ctzsi2.o)
                0x00000000800000d4                __ctzsi2
"""


def test_the_map_gives_text_bytes_per_input_and_per_archive_member():
    sizes = size.text_bytes(MAP)
    assert sizes["p.o"] == 0x1C
    assert size.members_of(sizes) == {"rt_68000_O2_int_ctzsi2.o": 0x1C}
    assert size.short("rt_68000_O2_int_ctzsi2.o") == "ctzsi2"


GCC = os.environ.get("GCC_M68K")


@pytest.mark.skipif(not GCC or not Path(o0_sweep.compiler()).exists(), reason="needs GCC_M68K and llrm-cc")
def test_a_routine_the_program_does_not_reference_is_not_linked(tmp_path):
    """prog.sh linked all six count routines into both columns: bitcount_u64's call form showed 0 library bytes, so the inline form looked 204 B worse than it was (whole program, 842 vs 900)."""
    source = tmp_path / "ctz.c"
    source.write_text("int main(void) { volatile unsigned x = 8; return __builtin_ctz(x) != 3; }\n")
    cc = o0_sweep.compiler()
    got = size.measure(source, "68000", "O2", "ours", cc, GCC, sweep.cache_dir(cc), tmp_path)
    linked = {size.short(m) for m in got["members"]}
    assert "ctzsi2" in linked, linked
    assert linked.isdisjoint({"ctzdi2", "clzdi2", "popcountsi2", "popcountdi2"}), linked
    assert got["whole"] == got["program"] + got["library"]


@pytest.mark.skipif(not GCC or not Path(o0_sweep.compiler()).exists(), reason="needs GCC_M68K and llrm-cc")
def test_both_columns_link_by_the_same_rule(tmp_path):
    """Two columns, one rule: each links only the members its own code references. popcount is a libcall in the gcc column and inline in ours, so the gcc column
    carries __popcountsi2 and ours does not; neither carries a routine its code does not call."""
    source = tmp_path / "pop.c"
    source.write_text("int main(void) { volatile unsigned x = 7; return __builtin_popcount(x) != 3; }\n")
    cc = o0_sweep.compiler()
    cache = sweep.cache_dir(cc)
    ours = size.measure(source, "68020", "Os", "ours", cc, GCC, cache, tmp_path)
    gcc = size.measure(source, "68020", "Os", "gcc", cc, GCC, cache, tmp_path)
    linked = {name: {size.short(m) for m in got["members"]} for name, got in (("ours", ours), ("gcc", gcc))}
    assert linked["gcc"] == {"popcountsi2"}, linked
    assert linked["ours"] == set(), linked
    for got in (ours, gcc):
        assert got["whole"] == got["program"] + got["library"]
    assert gcc["library"] > 0 and ours["library"] == 0


def test_the_archive_cache_key_follows_the_runtime_sources(tmp_path, monkeypatch):
    """The cache was keyed by the compiler binary alone: an edited compiler-rt source kept using the archive built from the old one, so a runtime change measured as no change."""
    runtime = tmp_path / "rt"
    (runtime / "lib/int").mkdir(parents=True)
    (runtime / "lib/int/clzsi2.c").write_text("int a;\n")
    monkeypatch.setattr(sweep, "RUNTIME", runtime)
    monkeypatch.setenv("M68K_EXEC_CACHE", str(tmp_path / "cache"))
    cc = str(tmp_path / "bin/llrm-cc")
    before = sweep.cache_dir(cc)
    (runtime / "lib/int/clzsi2.c").write_text("int b;\n")
    assert sweep.cache_dir(cc) != before


def test_gcc_gets_the_programs_no_inline_in_its_own_spelling():
    """The gcc column was built without the program's flags: frameless.c's gcc cell inlined what ours and the pin keep calls."""
    source = HERE.parent / "tests/run/c/frameless.c"
    flags = o0_sweep.flags_of(source.read_text(), "m68k-68000")
    command = size.gcc_command("m68k-linux-gnu-", "68000", "Os", Path("x.s"), source, flags)
    assert "-fno-inline" in command and "-fno-inline-functions" not in command, command
    assert "-std=gnu17" in command, "gcc 15 reads C23: old_style_extra_args had no gcc cell"
