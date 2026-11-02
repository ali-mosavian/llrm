import debug_identity as d


def test_a_debug_section_is_not_a_difference():
    off = {".text": b"\x90", ".data": b"1"}
    on = {".text": b"\x90", ".data": b"1", ".debug_info": b"x", ".rela.debug_info": b"r"}
    assert d.differences(off, on) == []


def test_changed_code_bytes_are_a_difference():
    """A mark that moved an instruction changed `.text`; only the debug sections were allowed to."""
    assert d.differences({".text": b"\x90\x90"}, {".text": b"\x90"}) == [".text"]


def test_a_section_only_one_side_has_is_a_difference():
    assert d.differences({".text": b"a"}, {".text": b"a", ".extra": b"b"}) == [".extra"]


def test_the_positive_control_needs_debug_sections_in_the_g_object():
    """A -g object with no .debug_* section proves nothing about the code: the leg did not run -g."""
    assert d.has_debug({".text": b"a", ".debug_info": b"x"})
    assert not d.has_debug({".text": b"a"})


def test_a_compile_that_hands_back_bytes_is_compared_from_those_bytes(tmp_path, monkeypatch):
    """The sweep's compile removes the object it wrote: opening the path crashed with FileNotFoundError and compared nothing."""
    import shutil
    import subprocess

    if not (shutil.which("cc") and shutil.which("objcopy")):
        return
    (tmp_path / "x.c").write_text("int f(int a) { return a + 1; }\n")
    plain, debug = tmp_path / "plain.o", tmp_path / "debug.o"
    subprocess.run(["cc", "-c", "-O1", str(tmp_path / "x.c"), "-o", str(plain)], check=True)
    (tmp_path / "info").write_bytes(b"debug")
    subprocess.run(["objcopy", "--add-section", f".debug_info={tmp_path / 'info'}", str(plain), str(debug)], check=True)
    stub = tmp_path / "cc-stub"
    stub.write_text(f'#!/bin/sh\ng=0\nfor a in "$@"; do [ "$a" = -g ] && g=1; done\nfor a in "$@"; do case $a in -o) next=1;; *) [ -n "$next" ] && {{ if [ $g = 1 ]; then cp {debug} "$a"; else cp {plain} "$a"; fi; next=; }};; esac; done\nexit 0\n')
    stub.chmod(0o755)
    monkeypatch.setattr("sys.argv", ["debug_identity", str(stub), "--src", str(tmp_path), "--targets", "m64", "--levels", "O2"])
    assert d.main() == 0


def test_the_section_name_table_is_not_a_difference_it_names_the_debug_sections():
    """`.shstrtab` holds the names of the sections, so adding `.debug_info` changes it by itself."""
    assert d.differences({".text": b"a", ".shstrtab": b"\0.text\0"}, {".text": b"a", ".shstrtab": b"\0.text\0.debug_info\0"}) == []


def test_an_object_with_debug_sections_added_reads_the_same_apart_from_them(tmp_path):
    """-g puts section symbols and a file symbol in the symbol table and shifts every index after them: read raw, `.symtab`, `.strtab` and
    the relocation sections differed in all 158 objects of the corpus, with or without the marks. Read as symbols and relocations by name
    they match."""
    import shutil
    import subprocess

    if not (shutil.which("cc") and shutil.which("objcopy")):
        return
    (tmp_path / "x.c").write_text("extern int g(int);\nint data = 3;\nint f(int a) { return g(a) + data; }\n")
    plain, debug = tmp_path / "plain.o", tmp_path / "debug.o"
    subprocess.run(["cc", "-c", "-O1", "-fno-pic", str(tmp_path / "x.c"), "-o", str(plain)], check=True)
    (tmp_path / "info").write_bytes(b"debug")
    subprocess.run(["objcopy", "--add-section", f".debug_info={tmp_path / 'info'}", str(plain), str(debug)], check=True)
    off, on = d.sections_of(plain.read_bytes()), d.sections_of(debug.read_bytes())
    assert d.has_debug(on) and not d.has_debug(off)
    assert d.differences(off, on) == []
    assert any(key.startswith("<relocs") and off[key] for key in off), "the object has no relocation to compare"
