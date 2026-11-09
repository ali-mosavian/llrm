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
