"""pindiff's instruments: the CPU guard, the OMF code-byte count and the instruction count each had a failure that made a report wrong."""
from __future__ import annotations

import importlib.util
import struct
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module  # a dataclass looks its module up here
    spec.loader.exec_module(module)
    return module


def test_a_pentium4_ir_is_refused_for_an_i486_llc():
    """`clang -m32` IR says pentium4 and llc honours it over -mcpu=i486: cmov and P4 unrolling made every m32 pin diff of 2026-10-27 compare
    two different targets (nbody x87 rows, a 4x LCG unroll, 6 hand-offs sent on them)."""
    guard = load("irguard")
    with pytest.raises(SystemExit, match="pentium4"):
        guard.check('attributes #0 = { "target-cpu"="pentium4" }', "i486", "x.ll")
    guard.check('attributes #0 = { "target-cpu"="i486" }', "i486", "x.ll")
    guard.check("define void @f() { ret void }", "i486", "no attribute at all")


def record(kind: int, body: bytes) -> bytes:
    """An OMF record: type, length (body and checksum byte), body, checksum."""
    return bytes([kind]) + struct.pack("<H", len(body) + 1) + body + b"\0"


def lnames(*names: str) -> bytes:
    return record(0x96, b"".join(bytes([len(name)]) + name.encode() for name in names))


def segdef(name: int, klass: int) -> bytes:
    return record(0x98, bytes([0x48]) + struct.pack("<H", 0) + bytes([name, klass, 1]))


def ledata(segment: int, data: bytes) -> bytes:
    return record(0xA0, bytes([segment]) + struct.pack("<H", 0) + data)


def test_the_code_bytes_of_an_omf_object_leave_the_data_out():
    """m16_bytes counted every LEDATA byte, initialised data included: a module with a 1 KiB table read 1 KiB bigger than the pin's `.text`."""
    omf = load("omfcode")
    obj = lnames("", "X_TEXT", "CODE", "_DATA", "DATA") + segdef(2, 3) + segdef(4, 5) + ledata(1, b"\x90" * 10) + ledata(2, b"\0" * 100)
    assert omf.code_bytes_of(obj) == (10, 100)


def test_the_instruction_count_skips_directives_labels_and_comments():
    """A `.p2align`, a label or `align 16` is not an instruction: counting them put +245 `align` at the head of a gap list."""
    pindiff = load("pindiff")
    text = "\n".join(
        [
            "_f proc near",
            "L0_1:",
            "    mov eax, 1   ; comment",
            "    align 16",
            "    .p2align 4",
            "    rep stosb",
            "    ret # note",
            "_f endp",
        ]
    )
    assert pindiff.instructions(text) == 3


def test_a_module_retargeted_twice_names_the_stack_alignment_once():
    """pindiff hands the pin's `opt` output to `llc`, which retargets again: at m32 (`S32`) the second pass wrote a second `!9999` module flag
    and the pin's llc stopped on `module flag identifiers must be unique` for all 69 programs."""
    pin = load("pin")
    of = pin.target("m32")
    once = pin.retarget("define void @f() {\n  ret void\n}\n", of)
    twice = pin.retarget(once, of)
    assert once.count("override-stack-alignment") == 1
    assert twice.count("override-stack-alignment") == 1


def test_the_default_gives_clang_no_passes_and_reopt_gives_it_the_level():
    """The first ladder ran ours over clang -O3 IR the pin's opt had already run: ours then optimised twice, and nbody_single, crc and recchop
    read as growth that the raw IR (w3's table: crc 58 vs 62) does not show."""
    pindiff = load("pindiff")
    raw = pindiff.front_end_flags("O2", reopt=False)
    assert "-disable-llvm-passes" in raw and "-O2" in raw
    assert "-disable-llvm-passes" not in pindiff.front_end_flags("O2", reopt=True)


def test_a_binary_behind_the_integration_branch_is_reported():
    """An llrm-llc built on an older parity reported dag-vs-old m16 gaps that the DAG fixes since had closed (2026-10-27): the header named
    only md5 and the build's sha, so nothing said the binary was 40 commits stale."""
    import subprocess
    import tempfile

    pindiff = load("pindiff")
    with tempfile.TemporaryDirectory() as tmp:
        git = lambda *a: subprocess.run(["git", "-C", tmp, "-c", "user.name=t", "-c", "user.email=t@t", *a], capture_output=True, text=True, check=True).stdout.strip()
        git("init", "-q", "-b", "main")
        shas = []
        for i in range(4):
            git("commit", "-q", "--allow-empty", "-m", str(i))
            shas.append(git("rev-parse", "HEAD"))
        assert pindiff.commits_behind(f"llrm-llc {shas[1]}", "main", tmp) == (shas[1], 2)
        assert pindiff.commits_behind("llrm-llc unknown", "main", tmp) == (None, None)
        assert "WARNING" in pindiff.staleness(f"llrm-llc {shas[0]}", "main", tmp, limit=2)
        assert pindiff.staleness(f"llrm-llc {shas[2]}", "main", tmp, limit=2) == ""


def test_two_objects_differing_only_in_alignment_have_no_padding_free_delta():
    """Function alignment moved a small row's bytes in steps of 16: six m32 rows read +12 B (16 B more padding before `main`, 4 B less in it)
    and were chased as a per-module cost (2026-10-27)."""
    pindiff = load("pindiff")
    omf = load("omfcode")
    body = bytes.fromhex("b890000000" "b090" "c3")  # mov eax,0x90; mov al,0x90; ret: 0x90 inside an immediate is code
    one = lnames("", "X_TEXT", "CODE") + segdef(2, 3) + ledata(1, body + b"\x90" * 5 + body)
    other = lnames("", "X_TEXT", "CODE") + segdef(2, 3) + ledata(1, body + b"\x66\x90" + bytes.fromhex("0f1f440000") + body)
    sizes = [omf.code_bytes_of(obj)[0] for obj in (one, other)]
    assert sizes[0] != sizes[1]
    free = [pindiff.padding_free(omf.code_blob_of(obj), 32) for obj in (one, other)]
    assert free[0] == free[1] == 2 * len(body)


def test_a_shift_by_the_type_width_in_raw_ir_is_flagged():
    """x_fir at m16 compared the pin's `xor eax,eax` (22 insns) with ours: `unsigned s >> 16` is `lshr i16 %s, 16`, UB, and the pin folded
    the function away. The row read +190 B at -O2 and was chased as a gap (2026-10-27)."""
    guard = load("irguard")
    assert guard.undefined_shifts("  %33 = lshr i16 %32, 16\n  %9 = shl nuw i8 %a, 9\n  %x = ashr exact i32 %v, 32") == [
        "lshr i16 %32, 16",
        "shl nuw i8 %a, 9",
        "ashr exact i32 %v, 32",
    ]
    assert guard.undefined_shifts("  %1 = lshr i16 %0, 15\n  %2 = shl i32 %a, 16\n  %3 = lshr i32 %b, %c") == []


def test_a_pin_llc_that_replaces_the_datalayout_is_stated():
    """The pin's llc takes the datalayout from its triple, whatever the module says: at m16 it runs ISel, LSR and CodeGenPrepare with
    `p:32:32` pointers (w3, particle's GEP shape), while ours has p:16. The m16 pin column carried that bias unsaid."""
    pindiff = load("pindiff")
    wanted = "e-p:16:16-p1:32:16:16:16-i32:16-n8:16:32"
    used = "e-m:e-p:32:32-p270:32:32-i128:128-f64:32:64-n8:16:32-S128"
    note = pindiff.layout_note(wanted, used)
    assert "p:32:32" in note and "p:16:16" in note and note.startswith("# NOTE")
    assert pindiff.layout_note(wanted, wanted) == ""


def test_no_row_of_the_corpus_shifts_by_the_width_of_its_type_at_m16():
    """17 m16 rows of the 2026-10-17 copy of the corpus had `unsigned s >> 16` on a 16-bit unsigned: UB the pin folded to poison, and the
    m16 pin column was wrong for them until 2026-10-27. The repo's corpus (`vsgcc/programs.py`) is written with u32/i32; this keeps it so."""
    import os

    pindiff = load("pindiff")
    if not (os.environ.get("LLRM_OW") and os.environ.get("LLRM_PIN_CLANG") and os.environ.get("LLRM_CLANG_M16")):
        # The gate sets all three; where it requires the pin's tools (LLRM_REQUIRE_PINTOOLS) a missing one is red, not a skipped check.
        (pytest.fail if os.environ.get("LLRM_REQUIRE_PINTOOLS") else pytest.skip)("needs LLRM_OW, LLRM_PIN_CLANG and LLRM_CLANG_M16")
    import tempfile

    guard = load("irguard")
    sources = os.environ.get("ROWS_SRC")
    with tempfile.TemporaryDirectory() as work:
        folder = Path(sources) if sources else pindiff.corpus_sources(Path(work) / "src")
        bad, compiled = {}, 0
        for source in sorted(folder.glob("*.c")):
            ir = Path(work) / f"{source.stem}.ll"
            binary, flags = pindiff.clang_flags("m16")
            import subprocess

            done = subprocess.run([binary, *flags, "-march=i486", *pindiff.front_end_flags("O2", False), "-o", str(ir), str(source)], capture_output=True)
            compiled += done.returncode == 0
            if done.returncode == 0 and (shifts := guard.undefined_shifts(ir.read_text())):
                bad[source.stem] = shifts[0]
        assert compiled >= 60, f"only {compiled} sources compiled"
        assert not bad


def test_compile_failures_are_counted_per_side_so_a_crash_cannot_hide_as_excluded():
    """crc at m16 -O2 made llrm-llc panic and x_ll_div made it refuse; both were one `# no figures` line among others, then read as
    'excluded rows' in the report (2026-10-27). A compiler crash is counted apart from the pin's or clang's failures."""
    pindiff = load("pindiff")
    row = {"name": "ok", "bytes": 10, "free": 10, "old_bytes": None, "old_free": None}
    results = {
        "ok": row,
        "crc": "ours: thread 'main' panicked at calcspillweights.rs:267:40:",
        "x_ll_div": "ours: llrm-llc: @f: --isel=dag: Legalize(Unsupported)",
        "x_strlen": "pin llc: PtrToAddr result must be address width",
        "lru": "clang: error: expected member name",
        "x_fir": "undefined shift in the source: lshr i16 %32, 16",
        "oldfail": {"name": "oldfail", "bytes": 5, "free": 5, "old_bytes": None, "old_free": None},
        "objfail": {"name": "objfail", "bytes": None, "free": None, "old_bytes": None, "old_free": None},
    }
    line = pindiff.failure_summary(results, old=True)
    assert line.startswith("# FAILED")
    assert "ours 2 (crc, x_ll_div)" in line and "1 panic" in line
    assert "ours-old 2 (ok, oldfail)" in line and "ours-obj 1 (objfail)" in line
    assert "pin 1 (x_strlen)" in line and "clang 1 (lru)" in line and "source 1 (x_fir)" in line
    assert pindiff.failure_summary({"ok": row}, old=False) == "# FAILED none"


def test_a_new_compile_failure_outside_the_known_list_is_unexpected():
    """The 2026-10-27 sweep found three failures in 1420 configs (crc spill-weight panic, x_ll_div freeze, lru far); a fourth must not hide in
    their noise, and a fixed one must leave the list."""
    sweep = load("crashsweep")
    seen = [
        ("m16", "O2", "crc", "dag", "thread 'main' panicked at crates/codegen/llrm-codegen/src/liveness/calcspillweights.rs:267:40: | Copy from non-existing value"),
        ("m32", "O1", "x_ll_div", "dag", 'llrm-llc: @f: --isel=dag: Legalize("Unsupported { op: Freeze }")'),
        ("m16", "O0", "lru", "-", "clang: lru.c:5:22: error: expected member name"),
    ]
    assert sweep.unexpected(seen) == []
    new = ("m16", "O2", "fib", "dag", "thread 'main' panicked at x.rs:1:1")
    other_level = ("m16", "O1", "crc", "dag", "thread 'main' panicked at crates/codegen/llrm-codegen/src/liveness/calcspillweights.rs:267:40:")
    assert sweep.unexpected([*seen, new, other_level]) == [new, other_level]
    assert [entry.name for entry in sweep.fixed(seen[:1])] == ["x_ll_div", "x_ll_div", "lru"]


def test_a_regcall_ir_is_given_the_watcom_abi_not_sysv():
    """After llrm-cc's m32 default became regcall every m32 row read `no figures`: ours was run with -mabi=sysv ('calling convention 92')."""
    pindiff = load("pindiff")
    cfg = pindiff.MACHINES["m32"]
    assert pindiff.abi_flags(cfg, "define x86_regcallcc i32 @f()") == ["-mabi=watcom", "-mregcall-default"]
    assert pindiff.abi_flags(cfg, "define i32 @f()") == ["-mabi=sysv"]


def test_a_known_failure_that_is_gone_is_reported_not_failed():
    """The sweep exited 1 when #5202 fixed x_ll_div's Freeze crash: the fixing PR could not land until it edited KNOWN in step, and w9/w13
    could not drop the entry at all while crashsweep.py was not on feat/parity (2026-10-27). A fix is reported (`FIXED?`); only a new crash fails."""
    sweep = load("crashsweep")
    crc = ("m16", "O2", "crc", "dag", "thread 'main' panicked at crates/codegen/llrm-codegen/src/liveness/calcspillweights.rs:267:40:")
    lru = ("m16", "O0", "lru", "-", "clang: lru.c:5:22: error")
    code, lines = sweep.verdict([crc, lru])
    assert code == 0 and any(line.startswith("FIXED? x_ll_div dag") for line in lines)
    new = ("m32", "O2", "fib", "dag", "thread 'main' panicked at x.rs:1:1")
    code, lines = sweep.verdict([crc, lru, new])
    assert code == 1 and any(line.startswith("UNEXPECTED m32 O2 fib dag") for line in lines)


def test_rowdelta_reads_back_what_pindiff_wrote_and_lists_only_moved_rows():
    """An A/B of two builds was one-off awk and a scratch script (x_radix O2, 2026-10-27); it must read the table pindiff prints."""
    pindiff = load("pindiff")
    rowdelta = load("rowdelta")

    def row(name, ins, free):
        return {"name": name, "ins": ins, "pin_ins": 100, "bytes": free + 4, "free": free, "pin_bytes": 300, "pin_free": 296, "old_bytes": None, "old_free": None}

    before = "\n".join(pindiff.table([row("a", 134, 462), row("b", 50, 100), row("gone", 1, 1)], old=False))
    after = "\n".join(pindiff.table([row("a", 124, 379), row("b", 50, 100), row("new", 1, 1)], old=False))
    assert rowdelta.parse(before)["a"] == (134, 100, 462, 296)
    assert rowdelta.deltas(before, after) == ["a: ins 134->124 (pin 100), nopad 462->379 (pin 296)"]


def _row(name, ins, pin_ins):
    return {"name": name, "ins": ins, "pin_ins": pin_ins, "bytes": 100, "pin_bytes": 100, "free": 90, "pin_free": 90, "old_bytes": None, "old_free": None}


def test_the_dynamic_columns_rank_by_clocks_and_flag_what_runs_slower_or_differs():
    """tile was 79 instructions below the pin and ran 94.7% more instructions (static counts flattered it): a row that is smaller and slower is flagged
    CLK>pin and listed before one that is larger and faster; an output that differs is flagged whatever the clocks."""
    pindiff = load("pindiff")
    dyn = {
        "tile": {"ins": 4210, "pin_ins": 2162, "clk": 8321, "pin_clk": 4277, "same": True},
        "big": {"ins": 900, "pin_ins": 800, "clk": 1000, "pin_clk": 1500, "same": True},
        "odd": {"ins": 10, "pin_ins": 10, "clk": 90, "pin_clk": 100, "same": False},
    }
    lines = pindiff.table([_row("big", 200, 100), _row("tile", 100, 179), _row("odd", 10, 10)], False, dyn)
    names = [line.split()[0] for line in lines[1:-1]]
    assert names == ["tile", "odd", "big"], lines
    assert "CLK>pin" in lines[1] and "OUTPUT-DIFFERS" not in lines[1]
    assert "OUTPUT-DIFFERS" in lines[2] and "CLK>pin" not in lines[2]
    assert "clk" in lines[0]


def test_the_dynamic_run_compiles_ours_at_the_level_asked_not_os_as_o2():
    """--dyn built ours at -O2 for Os: x_reverse -Os ran 19.7% slower than the pin (a `movzx` byte load the size gate removes), a gap its static columns
    (compiled at -Os) did not have."""
    dyn = load("dyn")
    from pathlib import Path

    assert "-Os" in dyn.ours_command("llc", "Os", "i486", Path("a.ll"), Path("a.o"))
    assert "-O2" not in dyn.ours_command("llc", "Os", "i486", Path("a.ll"), Path("a.o"))
    assert "-O3" in dyn.ours_command("llc", "O3", "i486", Path("a.ll"), Path("a.o"))
    assert "-march=i486" in dyn.ours_command("llc", "O3", "i486", Path("a.ll"), Path("a.o"))


def test_m32_clang_flags_do_not_default_every_function_to_regcall(monkeypatch):
    """targets.toml gave m32 `-fdefault-calling-conv=regcall` for llrm-cc; pindiff ran ours with sysv, so all 71 m32 programs stopped at
    `llrm-llc: @___regcall3__f: calling convention 92` and the table was empty."""
    pindiff = load("pindiff")
    monkeypatch.setenv("LLRM_OW", "/nonexistent")
    _, flags = pindiff.clang_flags("m32")
    assert "-fdefault-calling-conv=regcall" not in flags


def test_an_os_row_is_not_the_o2_row():
    """dyn/pindiff -Os rows were read as O2 when the IR handed to ours was the pin's, so a size gate of ours could not show. Raw IR through
    ours: the division by 7 is a multiply at -O2 and stays a division at -Os (same program, same llc)."""
    import os
    import tempfile

    llc = os.environ.get("LLRM_LLC")
    if not llc or "LLRM_OW" not in os.environ or "LLRM_PIN_CLANG" not in os.environ:
        pytest.skip("needs LLRM_LLC, LLRM_OW and LLRM_PIN_CLANG")
    pindiff = load("pindiff")
    with tempfile.TemporaryDirectory() as tmp:
        src = Path(tmp) / "d7.c"
        src.write_text("int f(int x) { return x / 7; }\n")
        asm = {}
        for level in ("O2", "Os"):
            work = Path(tmp) / level
            work.mkdir()
            assert isinstance(pindiff.one(src, "m32", level, llc, work, False), dict)
            asm[level] = (work / "d7.ours.s").read_text()
    assert "imul" in asm["O2"] and "idiv" in asm["Os"], asm


def test_drift_lists_only_rows_that_grew_the_most_instructions_first():
    """crc went 70 -> 129 instructions between two parity commits (2026-10-27) and nothing but a hand diff of two tables showed it; the others here held, shrank or grew only in padding-free bytes."""
    pindiff = load("pindiff")
    drift = load("drift")

    def row(name, ins, free):
        return {"name": name, "ins": ins, "pin_ins": 69, "bytes": free + 4, "free": free, "pin_bytes": 300, "pin_free": 191, "old_bytes": None, "old_free": None}

    before = "\n".join(pindiff.table([row("crc", 70, 189), row("fib", 20, 90), row("gcd", 50, 120), row("sort", 80, 300), row("new_only", 5, 5)][:4], old=False))
    after = "\n".join(pindiff.table([row("crc", 129, 359), row("fib", 20, 90), row("gcd", 45, 100), row("sort", 80, 320), row("new_only", 9, 9)], old=False))
    listed = drift.grown(before, after)
    assert [name for name, *_ in listed] == ["crc", "sort"]
    assert listed[0][1:3] == (59, 170)
    assert drift.grown(after, before) == [("gcd", 5, 20, "gcd: ins 45->50 (+5, pin 69), nopad 100->120 (+20, pin 191)")]


def test_drift_header_names_the_binary_the_ir_and_the_md5s(tmp_path):
    """crc read 129 instructions on the pindiff path and 72 on the product's (2026-10-27): the table must say which instrument made it."""
    drift = load("drift")
    before, after = tmp_path / "a", tmp_path / "b"
    before.write_bytes(b"one")
    after.write_bytes(b"two")
    lines = drift.header(["aaa", "bbb"], [before, after])
    assert "llrm-llc" in lines[0] and "NOT llrm-c" in lines[0] and "clang IR" in lines[0]
    assert lines[1] == "# before: aaa llrm-llc md5 f97c5d29941b"
    assert lines[2] == "# after: bbb llrm-llc md5 b8a9f715dbb6"



def test_the_libgcc_members_a_pin_object_calls_are_pulled_once_with_their_own_needs():
    """x_ll_div read +60% bytes at m32 (2026-10-27): ours embeds the i64 divide helpers, the pin's object only calls `__divdi3`, and
    nothing counted the ~1 KB libgcc a link adds on its side."""
    pindiff = load("pindiff")
    defined = {"__divdi3": "_divdi3.o", "__udivdi3": "_udivdi3.o", "__udivmoddi4": "_udivmoddi4.o"}
    needs = {"_divdi3.o": {"__udivmoddi4"}, "_udivdi3.o": {"__udivmoddi4"}, "_udivmoddi4.o": set()}
    pulled = pindiff.linked_members({"__divdi3", "__udivdi3", "memcpy"}, defined, lambda member: needs[member])
    assert sorted(pulled) == ["_divdi3.o", "_udivdi3.o", "_udivmoddi4.o"]
def test_a_regcall_ir_is_given_the_watcom_abi_not_sysv():
    """After llrm-cc's m32 default became regcall every m32 row read `no figures`: ours was run with -mabi=sysv ('calling convention 92')."""
    pindiff = load("pindiff")
    cfg = pindiff.MACHINES["m32"]
    assert pindiff.abi_flags(cfg, "define x86_regcallcc i32 @f()") == ["-mabi=watcom", "-mregcall-default"]
    assert pindiff.abi_flags(cfg, "define i32 @f()") == ["-mabi=sysv"]


def test_the_front_end_is_told_fno_builtin_so_its_ir_matches_the_pins_attributes():
    """The wat rows are hosted on purpose: without -fno-builtin builtins fold into memcpy/memmove and the IR, hence every count, differs from the pin's (clang-frontend session, 2026-10-27)."""
    pindiff = load("pindiff")
    assert "-fno-builtin" in pindiff.front_end_flags("O2", False)
    assert "-fno-builtin" in pindiff.front_end_flags("O2", True)


def test_an_m16_row_whose_pin_object_calls_an_i64_divide_is_marked_not_compared(tmp_path):
    """At m16 the pin's object calls __udivdi3 with no 16-bit libgcc to add: the row's bytes compared ours-with-helpers to a bare call (x_ll_div, 2026-10-27)."""
    pindiff = load("pindiff")
    row = {"name": "x", "ins": 5, "pin_ins": 5, "bytes": 9, "pin_bytes": 9, "free": 9, "pin_free": 9, "lib": [], "nolib": ["__udivdi3"]}
    line = pindiff.table([{**row, "ins": 50}], False)[1]
    assert "no pin comparison (no 16-bit libgcc)" in line and "INS>pin" not in line
