"""`uv run --project tools --with pytest python -m pytest tools/test_alloc_diff.py`

alloc_diff.py reads two listings of one function and names the first hunk where the allocation parts: a remade constant
against a copy, a spill and its reload. Toy listings, not a cell."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import alloc_diff  # noqa: E402

PIN = """\
   0:\t31 f6\txor    esi,esi
   2:\t45 31 c0\txor    r8d,r8d
   5:\t89 f8\tmov    eax,edi
   7:\t01 f0\tadd    eax,esi
   9:\tc3\tret
"""

# known_loads_walks: two zeros the pin remakes stay copies, and one value is spilled and reloaded around the add.
OURS = """\
   0:\t89 f8\tmov    eax,edi
   2:\t44 89 c0\tmov    r8d,r8d
   5:\t89 74 24 fc\tmov    DWORD PTR [rsp-0x4],esi
   9:\t89 f8\tmov    eax,edi
   b:\t03 44 24 fc\tadd    eax,DWORD PTR [rsp-0x4]
   f:\tc3\tret
"""


def test_classes_of_x86_intel_instructions():
    kinds = {text: alloc_diff.kind(text) for text in (
        "xor esi,esi", "mov r8d,0x5", "mov eax,edi", "mov DWORD PTR [rsp-0x4],esi", "mov esi,DWORD PTR [rsp+0x10]", "add eax,esi",
    )}
    assert kinds == {
        "xor esi,esi": "const", "mov r8d,0x5": "const", "mov eax,edi": "copy",
        "mov DWORD PTR [rsp-0x4],esi": "spill", "mov esi,DWORD PTR [rsp+0x10]": "reload", "add eax,esi": "other",
    }


def test_classes_of_arm64_instructions():
    assert [alloc_diff.kind(text) for text in ("str d6, [sp, #88]", "ldr d23, [sp]", "stp d27, d23, [sp, #8]", "ldp d9, d8, [sp, #48]", "mov x1, x2", "movz w0, #0x1")] == [
        "spill", "reload", "spill", "reload", "copy", "const",
    ]


def test_the_report_counts_the_constants_the_pin_remakes_and_the_spill_pair_ours_adds():
    """Fails if a remade constant, a spill or a folded reload is not counted: the instrument would print 'no hunk'."""
    ours, pin = alloc_diff.instructions(OURS), alloc_diff.instructions(PIN)
    text = alloc_diff.report(ours, pin)
    row = {line.split()[0]: line.split()[1:] for line in text.splitlines()[2:7]}
    assert row["const"] == ["0", "2", "-2"], text
    assert row["spill"] == ["1", "0", "+1"], text
    assert row["reload"] == ["1", "0", "+1"], text
    assert "hunk 1" in text and "const -2" in text.split("hunk 1", 1)[1].split("hunk 2")[0], text


def test_identical_listings_report_no_hunk():
    ours = alloc_diff.instructions(PIN)
    assert "no hunk changes the count of a class" in alloc_diff.report(ours, ours)


def test_register_names_and_targets_do_not_make_a_hunk():
    a = alloc_diff.instructions("   0:\t00\tmov    eax,edi\n   2:\t00\tjne    2a <f+0x2a>\n")
    b = alloc_diff.instructions("   0:\t00\tmov    ecx,esi\n   2:\t00\tjne    28 <f+0x28>\n")
    assert alloc_diff.hunks(a, b) == []
