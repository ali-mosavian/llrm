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
    (runtime / "config/m68k").mkdir(parents=True)  # cache_dir hashes config/m68k/mulsi3.s too
    (runtime / "config/m68k/mulsi3.s").write_text("rts\n")
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


def test_a_program_that_does_not_link_is_a_skip_row_not_the_end_of_the_sweep(monkeypatch, capsys):
    """The Os matrix of 211 programs stopped at its first (accumulator_forms: undefined `memcpy`) and printed no row for the other 210."""
    import argparse

    def measure(source, *rest):
        if source.stem == "bad":
            raise SystemExit("m68k_program_size: ours bad.c does not link: undefined reference to `memcpy'")
        return {"program": 10, "library": 2, "whole": 12, "members": {}}

    monkeypatch.setattr(size, "measure", measure)
    args = argparse.Namespace(cpu="68000", level="Os")
    for name in ("bad", "good"):
        size.report(name, ["ours", "gcc"], args, "cc", "/", Path("."), Path("."))
    rows = capsys.readouterr().out.splitlines()
    assert rows[0].startswith("| bad | 68000 | Os | ours | SKIP |") and "memcpy" in rows[0], rows
    assert rows[1:] == [f"| good | 68000 | Os | {c} | 10 | 2 | 12 |  |" for c in ("ours", "gcc")], rows


def test_the_assembly_is_not_given_more_global_labels_than_the_compiler_wrote():
    """Every label global made ours 192 B larger on fcmp_all (68020 Os, program 5530 B against 5338 B of the object) and left gcc's 5078 B alone: a call to a static function became a 32-bit relocation."""
    text = "\t.globl\tmain\nmain:\n\tjsr\t(helper,%pc)\nhelper:\n\trts\n"
    assert size.in_text(text).count(".globl") == 1


def test_the_like_columns_put_gcc_on_ours_pc_relative_data_model():
    """Ours addresses data pc-relative, gcc absolute: fcmp_all 68020 Os was +260 B against gcc and +148 B against `gcc -mpcrel` (5338 / 5078 / 5190 B), so the plain column mixed a data-model cost into the code-size gap."""
    source = HERE.parent / "tests/run/c/frameless.c"
    plain = size.gcc_command("m68k-linux-gnu-", "68020", "Os", Path("x.s"), source, [])
    like = size.gcc_command("m68k-linux-gnu-", "68020", "Os", Path("x.s"), source, [], pcrel=True)
    assert "-mpcrel" not in plain and "-mpcrel" in like, (plain, like)


def test_the_link_has_the_libc_stubs_for_a_program_that_calls_memcpy():
    """30 of 211 programs (every one that copies a struct or calls memcpy/memset or an `llrm_os_*` routine) were SKIP rows: the link had no libc, so a third of the corpus went unmeasured."""
    command = size.link_command("m68k-linux-gnu-", "68000", Path("p.o"), Path("p.exe"), Path("p.map"), "crt.o", "rt.a", "stubs.a")
    assert command[command.index("-Wl,--start-group") + 1 : command.index("-Wl,--end-group")] == ["rt.a", "stubs.a"], command


def test_gcc_gets_the_posix_headers_and_no_x86_code_model():
    """os_exit and bench_io (`#include "llrm_os.h"`) and memcpy_runtime (`-mcmodel=medium`, an x86 flag) had no gcc cell: 'No such file' and 'unrecognized command-line option'."""
    command = size.gcc_command("m68k-linux-gnu-", "68000", "Os", Path("x.s"), HERE.parent / "tests/run/c/frameless.c", ["-O2", "-mcmodel=medium"])
    assert str(HERE.parent / "runtime/shared/posix") in command and not any(flag.startswith("-mcmodel") for flag in command), command


def test_ours_is_llrm_ccs_own_object_not_gnu_as_on_its_text():
    """Reading ours through GNU as made fcmp_all 68020 Os 544 B too large (5338 against 4794 as an object), loop_copy_directions 622 B: every `lea (sym,%pc)` was an 8 B full-format word where the object has the pin's 4 B `R_68K_PC16`, so most of the 68020 cells 'over gcc' were the assembler."""
    command = size.ours_command("llrm-cc", "Os", [], Path("o.o"), Path("p.c"))
    assert "-c" in command and "-S" not in command and command[command.index("-o") + 1] == "o.o", command


def test_gcc_is_hosted_like_ours_so_both_link_the_same_stub_calls():
    """m32_word_vectors -O1: ours (hosted since #10561) called the stub memset, gcc built `-ffreestanding` inlined its fills: the stub's bytes and clocks counted on one side only."""
    command = size.gcc_command("", "68000", "O2", Path("p.s"), Path(__file__), [])
    assert "-ffreestanding" not in command, command


def test_the_harness_libc_stubs_are_not_program_bytes_but_compiler_rt_members_are():
    """mem_length_boundaries_m32 -Os 68000 read gcc 152 B + 454 B of stub memset/memcpy against ours 142 B: a call to the platform's libc counted the harness stand-in; compiler-rt (`__ashldi3`) travels with the compiler and stays counted."""
    sizes = {"/c/p.o": 100, "/c/stubs_68000.a(mem.o)": 454, "/c/stubs_68000.a(crt.o)": 30, "/c/compiler-rt_68000_Os.a(rt_68000_Os_int_ashldi3.o)": 40}
    assert size.members_of(sizes) == {"rt_68000_Os_int_ashldi3.o": 40}
