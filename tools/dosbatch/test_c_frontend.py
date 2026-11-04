"""`c_compile` names the C compiler a run builds its programs with: llrm-cc."""

import os
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dosbatch  # noqa: E402
import run_tests  # noqa: E402


def command(monkeypatch, target="dos16"):
    monkeypatch.setattr(dosbatch, "m_flag", lambda target: "-m16" if target == "dos16" else "-m32")
    monkeypatch.setattr(dosbatch, "target_bits", lambda target: 16 if target == "dos16" else 32)
    return dosbatch.c_compile(Path("a.c"), Path("a.obj"), target, "-O2", Path("inc"), ["-mabi=sysv"])


def test_llrm_cc_is_the_c_frontend(monkeypatch):
    """The default was llrm-c, which a build without the c-legacy feature lacks: every tests/run C program failed in the landing's behavioural stage."""
    words, env = command(monkeypatch)
    assert Path(words[0]).name == "llrm-cc" and env
    assert dosbatch.c_frontend() == "llrm-cc" and Path(run_tests.compile_command(run_tests.Program(source=Path("a.c"), flags=[], known=None), Path("a.obj"))[0]).name == "llrm-cc"


def test_llrm_cc_takes_the_target_as_a_row_not_a_flag(monkeypatch):
    """Passed -m16, llrm-cc handed clang a flag it has no meaning for; the target is LLRM_CC_TARGET."""
    for target, row in (("dos16", "x86_16-wat"), ("dos32", "x86_32-wat")):
        words, env = command(monkeypatch, target)
        assert Path(words[0]).name == "llrm-cc" and "-m16" not in words and "-m32" not in words
        assert env == {"LLRM_CC_TARGET": row} and "-c" in words and words[-2:] == ["-o", "a.obj"]


def test_the_isel_switch_reaches_llrm_cc_too(monkeypatch):
    monkeypatch.setenv("LLRM_ISEL", "dag")
    assert dosbatch.with_isel(["/x/llrm-cc", "a.c"])[1] == "--isel=dag"


def test_a_target_without_a_row_is_a_build_error_not_a_key_error(monkeypatch):
    """x86-m64 has no Watcom row: the run died in a KeyError: 64 instead of skipping the program with a reason."""
    monkeypatch.setattr(dosbatch, "target_bits", lambda target: 64)
    try:
        dosbatch.c_compile(Path("a.c"), Path("a.obj"), "x86-m64", "-O2", Path("inc"), [])
    except dosbatch.BuildError as why:
        assert "no row for x86-m64" in str(why)
    else:
        raise AssertionError("no error")


def test_a_c_program_itself_is_built_by_llrm_cc_where_the_run_names_it(monkeypatch, tmp_path):
    """Only the `link:` extras went through LLRM_C_FRONTEND: a pass of 463 programs 'on llrm-cc' ran llrm-c for all but 21 of them
    (wccq errors in its log), and passed with LLRM_CLANG_M16 unset."""
    import run_tests  # noqa: E402

    monkeypatch.delenv("LLRM_ISEL", raising=False)
    source = tmp_path / "p.c"
    source.write_text("int main(void) { return 0; }\n")
    monkeypatch.setattr(dosbatch, "c_include", lambda target, work: tmp_path)
    program = run_tests.Program(source=source, flags=["-O2", "-m32", "-march=i486"], known=None)
    command = run_tests.compile_command(program, tmp_path / "p.obj")
    assert Path(command[0]).name == "llrm-cc" and "-c" in command and "-m32" not in command and "-march=i486" in command, command
    assert run_tests.compile_env(program) == {"LLRM_CC_TARGET": "x86_32-wat"}
    assert run_tests.compile_env(run_tests.Program(source=source, flags=[], known=None)) == {"LLRM_CC_TARGET": "x86_16-wat"}


def test_the_start_up_of_a_llrm_cc_c_program_enters_main_(monkeypatch):
    """The Watcom rows name the entry `main_`: with the start-up asking for `_main`, 420 of 463 programs failed to link
    ("E2028: _main is an undefined reference")."""
    monkeypatch.setattr(dosbatch, "os_layer", lambda target, field, language: "A=1 B=2")
    assert dosbatch.os_defines("x86-m16", "c") == ("A=1", "B=2", "LL_MAIN=main_")
    assert dosbatch.os_defines("x86-m16", "nib") == ("A=1", "B=2")


def test_a_sysv_or_cdecl_program_keeps_the_start_ups_default_entry(monkeypatch):
    """-mabi=sysv names `_main`, not Watcom's `main_`: 21 rows failed "main_ is an undefined reference"."""
    monkeypatch.setattr(dosbatch, "os_layer", lambda target, field, language: "A=1")
    assert dosbatch.os_defines("x86-m32", "c", "sysv") == ("A=1",)
    assert dosbatch.os_defines("x86-m32", "c", "cdecl") == ("A=1",)
    assert dosbatch.os_defines("x86-m16", "c", "ia16") == ("A=1",)  # ia16_abi: "main_ is an undefined reference"
    assert dosbatch.os_defines("x86-m32", "c", None) == ("A=1", "LL_MAIN=main_")


def test_start_up_objects_built_with_different_entries_do_not_share_a_path(tmp_path):
    """Rows run at once in one work directory: the sysv rows' `_main` start-up and the Watcom rows' `main_` one were written to one
    file and replaced each other (a pass fell from 392 to 215, "_main is an undefined reference")."""
    a = dosbatch.runtime_path(tmp_path, "runtime/shared/dos/m32/start.asm", ("A=1",))
    b = dosbatch.runtime_path(tmp_path, "runtime/shared/dos/m32/start.asm", ("A=1", "LL_MAIN=main_"))
    assert a != b and a.parent == b.parent == tmp_path


def test_llrm_cc_reads_the_rows_own_os_header_not_llrm_cs(monkeypatch, tmp_path):
    """A -I of llrm-c's llrm_os.h came before the row's -isystem one: os_files, os_find, os_more... called the OS layer by llrm-c's
    pragmas and read -6 for 6 (7 programs of the DOSBox pass)."""
    import run_tests  # noqa: E402

    monkeypatch.delenv("LLRM_ISEL", raising=False)
    source = tmp_path / "p.c"
    source.write_text("int main(void) { return 0; }\n")
    monkeypatch.setattr(dosbatch, "c_include", lambda target, work: Path("/llrm-c-header-dir"))
    command = run_tests.compile_command(run_tests.Program(source=source, flags=[], known=None), tmp_path / "p.obj")
    assert "/llrm-c-header-dir" not in command, command
    assert "/llrm-c-header-dir" not in dosbatch.c_compile(source, tmp_path / "p.obj", "x86-m16", "-O2", Path("/llrm-c-header-dir"), [])[0]


def test_a_nib_programs_declarations_are_searched_and_the_header_dir_still_is_not(tmp_path):
    """Nib's foreign C includes NAME.h from the directory of its exports (interop: 'main.h' file not found, #6025), and that directory
    alone: the one `include` names holds llrm-c's llrm_os.h, which shadowed the row's and sent the QB runtime's bq16 programs to handle 426."""
    source = tmp_path / "geometry.c"
    command = dosbatch.c_compile(source, tmp_path / "g.obj", "x86-m16", "-O2", Path("/llrm-c-header-dir"), [], declarations=Path("/exports"))[0]
    assert command[command.index("-I") + 1] == "/exports", command
    assert "/llrm-c-header-dir" not in command, command


def test_a_watcom_abi_program_links_the_start_up_that_calls_main_(monkeypatch):
    """-mabi=watcom spells `main` as `main_`: watcom_far_sum, fsgs_call, register_upper_half and wc16_mixed failed to link
    ('_main is an undefined reference') when only some runs asked for LL_MAIN=main_ (#5540)."""
    monkeypatch.setattr(dosbatch, "os_layer", lambda target, field, language: "A=1")
    assert dosbatch.os_defines("x86-m16", "c", "watcom") == ("A=1", "LL_MAIN=main_")


def test_an_absolute_runtime_file_names_an_object_in_the_work_directory():
    """llrm-nib names the hook by absolute path: the object's name began with "/", a directory that did not exist (jwasm A1106 in NibCutTests)."""
    name = dosbatch.object_name(str(dosbatch.ROOT / "runtime/nib/x86-m16/init.asm"))
    assert "/" not in name and name == dosbatch.object_name("runtime/nib/x86-m16/init.asm")


def test_the_qb_runtime_c_sources_build_with_the_c_frontend_not_llrm_c(monkeypatch, tmp_path):
    """qb32.build and qbruntime._compile_c named llrm-c: with it unbuilt every QB program failed the landing's link check ("llrm was built without the toolchain feature")."""
    import qb32
    import qbruntime

    asked = []
    monkeypatch.setattr(dosbatch, "_host", lambda command, env=None: asked.append((command, env)))
    qbruntime._compile_c(Path("a.c"), tmp_path / "a.obj", tmp_path)
    (command, env), = asked
    assert Path(command[0]).name == "llrm-cc" and "-mbss-begin" in command and env == {"LLRM_CC_TARGET": "x86_16-wat"}, (command, env)
    assert not any(Path(one).name == "llrm-c" for one in command)
    assert str(tmp_path) not in command, "llrm-c's llrm_os.h (a plain __cdecl, ignored by the fork clang) would shadow the row's"
    assert qb32._stamp(dosbatch.c_frontend()).startswith("llrm-cc")


def test_the_real_mode_runtime_objects_are_cached_per_row(monkeypatch, tmp_path):
    """bq16 programs died on `KeyError: 426` (an INT 21h write to a garbage handle): the runtime's C was built with llrm-c's llrm_os.h, whose plain `__cdecl` the
    fork clang ignores, so `LL$WRITE_FILE` got register arguments; the cache key ignored the build, so the old objects were served after the fix."""
    import qb32
    import qbbuild
    import qbruntime

    keys = []
    monkeypatch.setattr(qb32, "cached", lambda key, obj, produce: keys.append(key))
    qbbuild.runtime16(tmp_path)
    assert keys and all(qbruntime.c_build_key() in key for key in keys) and "-I include" not in qbruntime.c_build_key()


def test_a_qb_program_links_the_start_up_that_calls_underscore_main(monkeypatch):
    """The QB link took C's defines, so under llrm-cc its start-up asked for `main_`, which no QB program defines (jwlink E2028: main_ is an undefined reference)."""
    monkeypatch.setattr(dosbatch, "subprocess", type("S", (), {"run": staticmethod(lambda *a, **k: type("R", (), {"stdout": "A=1"})())}))
    assert dosbatch.os_defines("x86-m32", "c") == ("A=1", "LL_MAIN=main_")
    assert dosbatch.os_defines("x86-m32", "qb") == ("A=1",)


def test_the_real_mode_start_up_finds_qb_start_by_the_name_start_asm_calls(tmp_path):
    """The m16 fork clang ignores `__cdecl`: rtinit.c defined `_qb_start@3`, start.asm asks for `_qb_start`, and every real-mode QB program failed
    to link (jwlink E2028: _qb_start is an undefined reference)."""
    import qbruntime

    if not (os.environ.get("LLRM_CLANG_M16") and os.environ.get("LLRM_OW")):
        pytest.skip("needs LLRM_CLANG_M16 and LLRM_OW")
    obj = tmp_path / "rtinit.obj"
    qbruntime._compile_c(dosbatch.ROOT / "runtime/qb/rtinit.c", obj, dosbatch.c_include(dosbatch.REAL_MODE, tmp_path))
    names = obj.read_bytes()
    assert b"\x09_qb_start" in names and b"_qb_start@3" not in names


@pytest.mark.parametrize("model", ["m16", "m32"])
def test_an_initializer_lands_in_the_xi_segments_start_up_walks(tmp_path, model):
    """`#pragma data_seg("XI", ...)` is ignored by the fork clang: nh_xinit's table entry sat in plain data, nothing registered the heap component,
    and every m32 QB program died at start-up ("Out of memory", "String formula too complex")."""
    import qb32
    import qbruntime

    if not (os.environ.get("LLRM_CLANG_M16") and os.environ.get("LLRM_OW")):
        pytest.skip("needs LLRM_CLANG_M16 and LLRM_OW")
    target, platform = (dosbatch.REAL_MODE, qbruntime.platform_directory()) if model == "m16" else (qb32.TARGET, qb32.PLATFORM)
    include = dosbatch.c_include(target, tmp_path)
    obj = tmp_path / "nheap.obj"
    flags = ["-I", str(qbruntime.RUNTIME), "-I", str(platform), *qbruntime.device_includes(platform)]
    dosbatch._host(*dosbatch.c_compile(dosbatch.ROOT / "runtime/qb/nheap.c", obj, target, "-Os", include, [], flags))
    names = obj.read_bytes()
    assert all(bytes([len(name)]) + name in names for name in (b"XIB", b"XI", b"XIE"))


def test_a_program_linked_without_an_entry_calls_the_start_ups_default_main(monkeypatch):
    """The x86_16-dos and x86_32-dos rows (cdecl, sysv) name no --entry; link_objects took os_defines' llrm-cc default, `main_`, and gate run 2
    failed a_program_runs_in_each_model and a_static_huge_object... with 'E2028: main_ is an undefined reference'."""
    monkeypatch.setattr(dosbatch, "os_layer", lambda target, field, language: "A=1")
    assert dosbatch.entry_defines("x86-m16", None) == ("A=1",)
    assert dosbatch.entry_defines("x86-m16", "main_", near=True) == ("A=1", "NEAR_CODE=1", "LL_MAIN=main_")


def test_the_object_cache_key_names_the_clang(monkeypatch, tmp_path):
    """w13: the key ignored the compiler, so swapping LLRM_CLANG_M16 served objects another clang built and the results looked valid."""
    import qbruntime

    def clang(name, version):
        path = tmp_path / name
        path.write_text(f"#!/bin/sh\necho 'clang version {version}'\n")
        path.chmod(0o755)
        return str(path)

    keys = []
    for name, version in (("a", "1"), ("b", "2")):
        monkeypatch.setenv("LLRM_CLANG_M16", clang(name, version))
        dosbatch.clang_identity.cache_clear()
        keys.append(qbruntime.c_build_key())
    assert keys[0] != keys[1]


def test_the_runtime_object_cache_key_follows_the_command_that_builds_the_objects(monkeypatch):
    """#6025 gave every C compile `-I <generated include>`; its llrm_os.h shadowed the row's and the OS layer lost its stack convention. The cache key
    named the flags and the compiler binary but not dosbatch.c_compile's command, so a tree whose compiler had not been relinked kept being served
    the objects of the other command: test_a_flat_programs_count_is_net_of_the_empty_startup_program died with KeyError 426 (a pointer for a DOS
    handle) in some runs and passed in others, and reverting a PR changed nothing. The key must change with the command."""
    import qbruntime

    before = qbruntime.c_build_key()
    real = dosbatch.c_compile

    def with_a_flag(source, obj, target, level, include, abi, extra=()):
        command, env = real(source, obj, target, level, include, abi, extra)
        return [*command[:2], "-DSOMETHING_NEW", *command[2:]], env

    monkeypatch.setattr(dosbatch, "c_compile", with_a_flag)
    assert qbruntime.c_build_key() != before
