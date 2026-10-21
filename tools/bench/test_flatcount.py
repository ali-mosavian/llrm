"""The flat (dos32) kernel's work in an emulator, priced by the 486's clocks (tools/bench/flatcount.py)."""

from __future__ import annotations

import sys
import struct
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import flatcount  # noqa: E402

BASE = 0x08048000
KERNEL = BASE + 0x40


def image(tmp_path: Path, kernel: bytes) -> Path:
    """A flat ELF of one segment: at BASE `call KERNEL; hlt`, at KERNEL the kernel; its map names KERNEL as BENCHK."""
    code = bytearray(0x40) + kernel
    code[0:6] = b"\xe8" + struct.pack("<i", KERNEL - (BASE + 5)) + b"\xf4"
    header = bytearray(52)
    header[0:16] = b"\x7fELF\x01\x01\x01" + bytes(9)
    struct.pack_into("<HHIIIIIHHHHHH", header, 16, 2, 3, 1, BASE, 52, 0, 0, 52, 32, 1, 0, 0, 0)
    phdr = struct.pack("<8I", 1, 84, BASE, BASE, len(code), len(code), 7, 4096)
    path = tmp_path / "k.elf"
    path.write_bytes(bytes(header) + phdr + bytes(code))
    path.with_suffix(".map").write_text(f"{KERNEL:08x}*      BENCHK\n")
    return path


def test_a_kernels_instructions_and_486_clocks_are_counted_from_its_entry_to_its_return(tmp_path):
    # mov ecx,3; l: dec ecx; jnz l; ret
    elf = image(tmp_path, b"\xb9\x03\x00\x00\x00\x49\x75\xfd\xc3")
    ran = flatcount.run(elf, "BENCHK", tmp_path)
    assert not ran.error, ran.error
    # 1 + 3 * 2 + the ret; the table: mov 1, dec 1 each, jnz 3 taken twice and 1 not, ret 5
    assert (ran.region.instructions, ran.region.clocks) == (8, 1 + 3 + (3 + 3 + 1) + 5)


def test_a_repeated_string_instruction_is_one_instruction_and_one_with_ecx_zero_too(tmp_path):
    """DOSBox counts a REP once; the emulator's hook fires per iteration, and once more with ECX at 0."""
    # mov ecx,5; rep stosd; xor ecx,ecx; rep stosd; ret  (edi 0x1000 is writable memory)
    elf = image(tmp_path, b"\xbf\x00\x10\x00\x00\xb9\x05\x00\x00\x00\xf3\xab\x31\xc9\xf3\xab\xc3")
    ran = flatcount.run(elf, "BENCHK", tmp_path)
    assert not ran.error and ran.region.instructions == 6, (ran.error, ran.region)


def test_the_stack_is_below_the_image_as_the_extenders_is(tmp_path):
    """The runtime's str_is_tmp_ tests a pointer against the temp-string table in the image: 4 instructions for one below it,
    6 for one above. The stack sat above the image here and below it in DOSBox, so jpeg_single bq32 counted 56,380,382 here and
    56,089,617 there (159,522 stack strings, 2 more each)."""
    # cmp esp, BASE; jae +1 (over the nop); nop; ret
    elf = image(tmp_path, b"\x81\xfc" + struct.pack("<I", BASE) + b"\x73\x01\x90\xc3")
    ran = flatcount.run(elf, "BENCHK", tmp_path)
    assert not ran.error and ran.region.instructions == 4, (ran.error, ran.region)


def test_an_image_that_does_not_start_above_the_stack_is_an_error_not_a_drifting_count(tmp_path, monkeypatch):
    monkeypatch.setattr(flatcount, "STACK", BASE + 0x1000)
    ran = flatcount.run(image(tmp_path, b"\xc3"), "BENCHK", tmp_path)
    assert ran.region is None and "above the stack" in ran.error, ran.error


def test_a_call_the_emulator_does_not_know_stops_the_run_and_is_named(tmp_path):
    elf = image(tmp_path, b"\xcd\x2f\xc3")  # int 2fh
    ran = flatcount.run(elf, "BENCHK", tmp_path)
    assert ran.region is None and "int 0x2f" in ran.error, ran.error


def test_a_region_that_is_not_in_the_map_is_an_error_not_zero(tmp_path):
    ran = flatcount.run(image(tmp_path, b"\xc3"), "NOPE", tmp_path)
    assert ran.region is None and "NOPE" in ran.error


def test_the_emulators_count_and_dosboxs_agree_within_a_few_instructions_not_where_a_rep_was_sliced():
    """DOSBox counts a long REP in slices of its cycle budget: scroll read 58,321 there and 21,015 here."""
    assert flatcount.agrees(372144, 372144) and flatcount.agrees(22784, 22785) and flatcount.agrees(84627970, 84337547)
    assert not flatcount.agrees(21015, 58321) and not flatcount.agrees(1000000, 1100000)


def test_a_flat_row_gets_clocks_only_where_the_emulator_ran_the_kernel_dosbox_counted(monkeypatch, tmp_path):
    import types

    import bench

    ran = lambda instructions, clocks=7: types.SimpleNamespace(error="", rep_iterations=0, region=types.SimpleNamespace(instructions=instructions, clocks=clocks))  # noqa: E731
    key = ("fib", "bq32", "O2")
    monkeypatch.setattr(flatcount, "run", lambda *a: ran(100))
    assert bench.flat_clocks(key, tmp_path / "x.elf", "BenchFib", 100, tmp_path) == {"clocks": 7}
    assert bench.flat_clocks(key, tmp_path / "x.elf", "BenchFib", 5000, tmp_path) == {}, "another run's price is no price"
    monkeypatch.setattr(flatcount, "run", lambda *a: types.SimpleNamespace(error="int 0x2f", region=None))
    assert bench.flat_clocks(key, tmp_path / "x.elf", "BenchFib", 100, tmp_path) == {}
    line = bench.clock_lines({("fib", "bq32", "O2"): {"clocks": 7, "instructions": 100}}, ["fib"])
    assert line == ["fib bq32 -O2: 7 clocks, 100 instructions"], "a flat row has no memory operands count"


def test_a_kernel_that_does_not_return_says_which_reason_it_stopped_for(tmp_path):
    """grep's kernel (10 MB through DOS, ~10^9 instructions) read `entered but never returned`: it had hit the emulator's limit."""
    loop = flatcount.run(image(tmp_path, b"\xeb\xfe"), "BENCHK", tmp_path, limit=1000)  # jmp $
    assert loop.region is None and "limit of 1000 instructions" in loop.error, loop.error
    quit_ = flatcount.run(image(tmp_path, b"\xb8\x00\x4c\x00\x00\xcd\x21\xc3"), "BENCHK", tmp_path)  # mov eax,4c00h; int 21h
    assert quit_.region is None and "exited before the kernel returned" in quit_.error, quit_.error
    import bench

    huge = flatcount.LIMIT
    assert bench.flat_clocks(("grep", "bq32", "O2"), tmp_path / "x.elf", "BenchGrep", huge, tmp_path) == {}, "no emulator run for a kernel that long"


FIXTURES = Path(__file__).parent / "fixtures"


def test_an_elf_whose_tables_are_written_over_its_code_is_refused_not_run(tmp_path):
    """frames at bq32 linked with a 4 KiB gap before CONST2: jwlink put the relocation tables at 0x102f, inside the code to 0x19f5, and the emulator
    ran them (`Invalid memory read`): 21 of 39 bq32 rows had no clocks."""
    ran = flatcount.run(FIXTURES / "frames_tables_over_code.elf", "BENCHFRAMES", tmp_path)
    assert ran.region is None and ran.error.startswith("jwlink wrote a corrupt ELF: "), ran.error
    assert flatcount.layout_errors(FIXTURES / "frames_tables_after_image.elf") == []


def test_an_emulator_fault_names_the_instruction_that_faulted_not_a_segment_selector(tmp_path):
    """The warning said `at 8`: the code selector, which every fault in the flat image has."""
    elf = image(tmp_path, b"\xa1\x00\x00\x00\x0b\xc3")  # mov eax,[0xb000000]; ret: above the emulated memory
    ran = flatcount.run(elf, "BENCHK", tmp_path)
    assert ran.error and f"eip {KERNEL:#x}" in ran.error, ran.error


def test_dosbox_counts_a_long_rep_in_slices_and_the_emulators_count_is_judged_with_that_allowance(tmp_path):
    """matmul, ring, sieve, bintree and huge on head have a `rep stosd` over a page in the kernel: DOSBox counted 131 to 20,104 more instructions
    than the emulator (one more for every 4 iterations), the count read as another run's, and the rows had no clocks."""
    # xor eax,eax; mov edi,0x08800000; mov ecx,1000; rep stosd; ret
    elf = image(tmp_path, b"\x31\xc0\xbf\x00\x00\x80\x08\xb9\xe8\x03\x00\x00\xf3\xab\xc3")
    ran = flatcount.run(elf, "BENCHK", tmp_path)
    assert not ran.error, ran.error
    assert (ran.region.instructions, ran.rep_iterations) == (5, 999)
    dosbox = ran.region.instructions + 999 // 4
    assert flatcount.agrees(ran.region.instructions, dosbox, ran.rep_iterations)
    assert not flatcount.agrees(ran.region.instructions, dosbox), "the count alone is 249 off"
    assert not flatcount.agrees(ran.region.instructions, dosbox + 500, ran.rep_iterations)
