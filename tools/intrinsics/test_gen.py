"""The committed intrinsic_attrs.rs is what gen.py writes, and what LLVM itself attaches."""
import os
import re
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gen  # noqa: E402

SRC = Path(os.environ.get("LLVM_SRC", Path.home() / "scratch/llvm-src"))
BUILD = Path(os.environ.get("LLVM_BUILD", Path.home() / "scratch/llvm-build"))
TBLGEN = BUILD / "bin/llvm-tblgen"
CLANG = BUILD / "bin/clang++"


@pytest.fixture(scope="module")
def td():
    if not TBLGEN.exists() or not (SRC / "llvm/include/llvm/IR/Intrinsics.td").exists():
        pytest.skip(f"needs {TBLGEN} and the LLVM source at {SRC} (LLVM_BUILD, LLVM_SRC)")
    return gen.tblgen()


def test_committed_table_is_what_gen_writes(td):
    """A stale table gives declarations attributes LLVM no longer does."""
    assert gen.generate() == gen.OUT.read_text()


def test_every_emitted_name_is_in_intrinsics_td(td):
    """A name in VALUETRACKING_NAMES, or in intrinsics.rs, that LLVM does not define would be an empty entry."""
    names, _, _, _ = gen.build(td, gen.emitted())
    assert set(names) == gen.emitted()
    assert len(gen.VALUETRACKING_NAMES) == len(set(gen.VALUETRACKING_NAMES))


ATTRIBUTE = r'F\("(\w+)"\)|I\("(\w+)", (\d+)\)|R\((-?\d+), (-?\d+)\)'


def parse(text):
    """The `names` and `sets` of `gen.build`, read back from the Rust a run wrote."""
    sets = []
    for line in re.findall(r"^    Set \{ (.*) \},$", text, re.M):
        function, memory, arguments = re.fullmatch(
            r"function: &\[(.*?)\], memory: &\[(.*?)\], arguments: &\[(.*)\]", line
        ).groups()
        memory = tuple(
            (None if loc == "None" else loc[6:-2], access)
            for loc, access in re.findall(r'\((None|Some\("[a-z_0-9]+"\)), "(\w+)"\)', memory)
        )
        arguments = tuple(
            (int(at), tuple(attribute_of(m) for m in re.finditer(ATTRIBUTE, body)))
            for at, body in re.findall(r"\((\d+), &\[(.*?)\]\)", arguments)
        )
        sets.append((tuple(re.findall(r'"(\w+)"', function)), memory, arguments))
    names = {
        name: (int(at), flag == "true")
        for name, at, flag in re.findall(r'^    \("(llvm\.[^"]+)", (\d+), (true|false)\),$', text, re.M)
    }
    return names, sets


def attribute_of(m):
    flag, int_key, int_value, low, high = m.groups()
    if flag:
        return (flag, None)
    if int_key:
        return (int_key, int(int_value))
    return ("range", (int(low), int(high)))


def test_every_attribute_is_llvms(td, tmp_path):
    """LLVM's own `Intrinsic::getAttributes`, for every intrinsic, less what MIR cannot say."""
    if not CLANG.exists() or not list((BUILD / "lib").glob("libLLVM*.a")):
        pytest.skip(f"needs {CLANG} and the static LLVM libraries")
    everything = tmp_path / "all.rs"
    subprocess.run([sys.executable, gen.__file__, "--all", everything], check=True)
    names, sets = parse(everything.read_text())
    assert len(names) > 16000
    binary = tmp_path / "attrdump"
    subprocess.run(
        [CLANG, "-std=c++17", "-O1", f"-I{SRC}/llvm/include", f"-I{BUILD}/include", Path(gen.__file__).with_name("attrdump.cpp"),
         "-Wl,--start-group", *sorted((BUILD / "lib").glob("libLLVM*.a")), "-Wl,--end-group",
         "-lpthread", "-lz", "-ldl", "-o", binary],
        check=True,
    )
    out = subprocess.run([binary], capture_output=True, text=True, check=True).stdout
    left_out = {f" {one}" for one in gen.UNREPRESENTABLE}
    theirs = {line for line in out.splitlines() if not any(line.endswith(one) for one in left_out)}
    ours = gen.llvm_lines(names, sets)
    assert len(ours) > 80000
    assert sorted(theirs - ours)[:5] == [] and sorted(ours - theirs)[:5] == []
