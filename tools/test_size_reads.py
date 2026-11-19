"""A table read the byte count of an object or assembly FILE as code size: tools/ab/cells.sh fell back to `stat -c %s "$w/$v.o"` when size(1)
could not parse an OMF, z80 or 6502 object (wasm-b's A/B read +1052 where the assembled _CODE was +175). Code bytes come from
tools/ab/object_bytes.py, which errors instead. A script that takes the size of an object, assembly or object-named file must be listed
here with the reason (a shipped-file size is not a code column)."""
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SIZE_READ = re.compile(r"st_size|getsize|stat -c ?%s|stat -f ?%z|wc -c")
EMPTINESS = re.compile(r"st_size\s*(==|!=|>)\s*0|stat\(\)\.st_size\s*>\s*0")
OBJECT = re.compile(r"""\.(o|obj|rel|s|asm)\b|\bobj\w*|\bobject\w*|\basm\w*""", re.I)

# (file, text of the line): why the file's size is the intended number.
ALLOWED = {
    ("tools/dosbatch/qbruntime.py", '"bytes": output.stat().st_size,'): "the shipped LLRMQB.LIB archive's size in its manifest, not a code column",
    ("tools/dosbatch/qbruntime.py", '"objects": {obj.name: obj.stat().st_size for obj in objects},'): "the shipped library members' file sizes in the same manifest",
}


# size(1) piped to awk for a code column: the Berkeley first column (`NR==2{print $1}`) or `-A` rows of `.text` alone. The audit rule is code + rodata + data;
# tools/ab/object_bytes.py gives both. `.rodata` or `.data` in the same line makes it a code+data read.
TEXT_ONLY = re.compile(r"\bsize\b[^|\n]*\|\s*awk\b[^\n]*(NR==2|\.text)")
DATA_TOO = re.compile(r"\.(rodata|data)\b|\$3|\$2\+\$3|\+\s*data")


# `$1==".text"` takes the one section: gcc's `.text.startup` (main), `.text.unlikely` and `.text.hot` are dropped, data or not (strlen_idiom gcc -O2: 38 B read, 127 B real).
EXACT_TEXT = re.compile(r'\$1==\\?".text\\?"')


def text_only(line: str) -> bool:
    if "text-only by design:" in line or not TEXT_ONLY.search(line):
        return False
    return bool(EXACT_TEXT.search(line)) or not DATA_TOO.search(line)


def reads(line: str) -> bool:
    return bool(SIZE_READ.search(line)) and not EMPTINESS.search(line) and bool(OBJECT.search(line))


def scripts() -> list[str]:
    names = subprocess.run(["git", "ls-files", "tools", "crates"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.split()
    return [n for n in names if n.endswith((".py", ".sh")) and not Path(n).name.startswith("test") and n != "tools/ab/object_bytes.py"]


def test_no_script_takes_the_size_of_an_object_or_assembly_file_as_code_bytes():
    hits = []
    for name in scripts():
        for number, line in enumerate((ROOT / name).read_text(errors="ignore").splitlines(), 1):
            if reads(line) and (name, line.strip()) not in ALLOWED:
                hits.append(f"{name}:{number}: {line.strip()}")
    assert not hits, "file size read as a code size (use tools/ab/object_bytes.py, or list it in ALLOWED with the reason):\n" + "\n".join(hits)


# (file, snippet of the line): text only, owner to extend to code + rodata + data, or to say `# text-only by design: WHY` on the line.
TEXT_ONLY_ALLOWED = {
    ("tools/softfloat_cells/cells.sh", "text $(size -A"): "softfloat_cells owner",
}


def test_no_script_reads_the_text_column_of_size_as_the_whole_program():
    hits = []
    for name in scripts():
        for number, line in enumerate((ROOT / name).read_text(errors="ignore").splitlines(), 1):
            if text_only(line) and not any(name == f and snippet in line for f, snippet in TEXT_ONLY_ALLOWED):
                hits.append(f"{name}:{number}: {line.strip()[:150]}")
    assert not hits, "size(1) text column read as program bytes (add data/rodata, use tools/ab/object_bytes.py, or list with the owner):\n" + "\n".join(hits)


def test_the_text_column_reads_are_recognised():
    """cmp_ab.sh: `size o.a.o | awk 'NR==2{print $1}'` reads the text column of an object whose data and bss are not counted."""
    assert text_only("bytesA=$(size $t/o.a.o | awk 'NR==2{print $1}')")
    assert text_only("t=$(size -A \"$out\" | awk '$1==\".text\"{print $2}')")
    assert not text_only("t=$(size -A \"$o\" | awk '$1==\".text\"{print $2}')  # text-only by design: Dag and Old read the same data")
    assert text_only("bytes() { size -A \"$1\" | awk '$1==\".text\"||$1~/^\\.rodata/{s+=$2} END{print s+0}'; }"), "exact .text drops .text.startup even with rodata"
    assert not text_only("bytes() { size -A \"$1\" | awk '$1~/^\\.text/||$1~/^\\.rodata/{s+=$2} END{print s+0}'; }")


def test_obj_bytes_counts_a_main_in_text_startup():
    """gcc puts `main` in `.text.startup`: obj_bytes read `.text` alone and counted 0 B for it."""
    import shutil
    import tempfile

    if not shutil.which("gcc") or not shutil.which("size"):
        return
    with tempfile.TemporaryDirectory() as where:
        source = Path(where) / "m.c"
        source.write_text('__attribute__((section(".text.startup"))) int main(void) { return 42; }\n')
        subprocess.run(["gcc", "-O2", "-c", str(source), "-o", f"{where}/m.o"], check=True)
        out = subprocess.run(["bash", "-c", f'. {ROOT}/runtime/compiler-rt/tests/obj_bytes.sh; obj_bytes {where}/m.o'], capture_output=True, text=True, check=True).stdout
    assert int(out) > 0, out


def test_text_only_allowances_still_match_a_line_and_name_an_owner():
    """An entry whose script was fixed or removed is dropped: the list only shrinks."""
    for name, snippet in TEXT_ONLY_ALLOWED:
        lines = (ROOT / name).read_text().splitlines()
        assert any(snippet in line and text_only(line) for line in lines), f"{name}: {snippet!r} is gone or no longer text only: drop the entry"
        assert TEXT_ONLY_ALLOWED[(name, snippet)]


def test_the_cells_fallback_that_read_whole_file_bytes_is_caught():
    """cells.sh line 29 (before): `[ -n "${text[$v]}" ] || text[$v]=$(stat -c %s "$w/$v.o")`: the file's bytes in a code column."""
    assert reads('    [ -n "${text[$v]}" ] || text[$v]=$(stat -c %s "$w/$v.o")')
    assert reads("size = os.path.getsize(asm_path)")
    assert reads("n = Path(work, 'f.s').stat().st_size")


def test_the_object_reader_and_emptiness_checks_are_not_flagged():
    assert not reads('    text[$v]=$("$(dirname "$0")/object_bytes.py" "$tg" "$w/$v.o" 2>>"$w/err")')
    assert not reads("if not obj.exists() or obj.stat().st_size == 0:")
    assert not reads("exe_size = exe.stat().st_size")


def test_allowed_entries_name_a_line_that_still_exists_and_a_reason():
    for (name, text), why in ALLOWED.items():
        assert why and text in [one.strip() for one in (ROOT / name).read_text().splitlines()], f"{name}: {text!r} is gone: drop the entry"
