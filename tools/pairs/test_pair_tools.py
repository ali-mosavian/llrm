"""objsweep and rowdiff tell a changed object and a worse row from an unchanged one, and say a compile that failed instead of counting it unchanged."""
from __future__ import annotations

import importlib.util
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def fake_llrm(path: Path, body: str) -> str:
    """A stand-in llrm-cc: writes `body` to the -o file; a source containing FAIL makes it exit 1."""
    path.write_text(f'#!/bin/bash\n# {'#' * 5000 * (1 + ord(body[0]) % 2)}\n[ "$1" = --version ] && {{ echo "llrm-cc abc123"; exit 0; }}\nfor a; do [ "$p" = -o ] && o=$a; p=$a; done\ngrep -q FAIL "$a" && {{ echo "llrm-cc: boom" >&2; exit 1; }}\necho {body} > "$o"\n')
    path.chmod(0o755)
    return str(path)


def test_objsweep_names_a_changed_object_and_a_one_sided_failure(tmp_path, monkeypatch):
    monkeypatch.setenv("LLRM_PAIR_CROSS_BRANCH", "1")  # the fake builds stamp a sha no repository has; their ancestry is test_ancestry's; --isel=old since no diff of them can be read
    src = tmp_path / "b"
    src.mkdir()
    (src / "same.c").write_text("int a;")
    (src / "broken.c").write_text("FAIL")
    old, new = fake_llrm(tmp_path / "old", "x"), fake_llrm(tmp_path / "new", "y")
    done = subprocess.run([sys.executable, str(HERE / "objsweep.py"), old, new, "--src", str(src), "--levels", "O2", "--targets", "m32", "--isel=old"], capture_output=True, text=True)
    assert done.returncode == 1
    assert "1 objects changed" in done.stdout and "changed same m32 -O2" in done.stdout
    assert "failed on both sides" in done.stdout and "1 failed on both sides" in done.stdout


def test_objsweep_measures_the_object_not_the_assembly_text(tmp_path, monkeypatch):
    monkeypatch.setenv("LLRM_PAIR_CROSS_BRANCH", "1")  # the fake builds stamp a sha no repository has; their ancestry is test_ancestry's; --isel=old since no diff of them can be read
    """objsweep wrote to `x.old`: llrm-cc wrote assembly text there, so x_fir m16 read +350 B (text) where the object grew 21 B."""
    src = tmp_path / "b"
    src.mkdir()
    (src / "p.c").write_text("int a;")
    for name, obj in (("old", "A"), ("new", "B")):
        pad = "#" * (5000 if name == "old" else 9000)  # two builds, not one relinked
        (tmp_path / name).write_text(f'#!/bin/bash\n# {pad}\n[ "$1" = --version ] && {{ echo "llrm-cc abc123"; exit 0; }}\nfor a; do [ "$p" = -o ] && o=$a; p=$a; done\ncase "$o" in *.o|*.obj) echo {obj} > "$o";; *) echo TEXT > "$o";; esac\n')
        (tmp_path / name).chmod(0o755)
    done = subprocess.run([sys.executable, str(HERE / "objsweep.py"), str(tmp_path / "old"), str(tmp_path / "new"), "--src", str(src), "--levels", "O2", "--targets", "m32", "--isel=old"], capture_output=True, text=True)
    assert "changed p m32 -O2" in done.stdout, done.stdout + done.stderr


def test_rowdiff_counts_rows_above_a_and_names_a_row_on_one_side():
    rowdiff = load("rowdiff")
    row = lambda c, b: {"clocks": c, "code": b}
    a = {("p", "llrm"): row(100, 50), ("q", "llrm"): row(10, 10)}
    b = {("p", "llrm"): row(120, 50), ("r", "llrm"): row(1, 1)}
    lines, above = rowdiff.diff(a, b, set())
    assert above == 1
    assert "p -O2: clocks 100->120 (+20.0%), code 50->50 (+0.0%)" in lines
    assert "q -O2: only in A" in lines and "r -O2: only in B" in lines


def test_the_stub_defines_strlen(tmp_path):
    """The pinned clang turns x_strlen's loop into a call to strlen; the stub lacked it and all four clang rows of x_strlen failed to link (the table had no reference)."""
    stub = HERE.parents[1] / "crates/target/llrm-x86-m32/vsgcc/stub.s"
    subprocess.run(["gcc", "-m32", "-c", str(stub), "-o", str(tmp_path / "s.o")], check=True)
    symbols = subprocess.run(["nm", str(tmp_path / "s.o")], capture_output=True, text=True, check=True).stdout
    assert " T strlen" in symbols


def test_a_pair_of_one_build_relinked_is_void(tmp_path, monkeypatch):
    monkeypatch.setenv("LLRM_PAIR_CROSS_BRANCH", "1")  # the fake builds stamp a sha no repository has; their ancestry is test_ancestry's; --isel=old since no diff of them can be read
    """Two llrm-cc built into one shared target dir differed by 83 path bytes; objsweep read 0 objects changed of two copies of one compiler."""
    voidpair = load("voidpair")
    base = bytes(range(256)) * 1024
    a, b, c = tmp_path / "a", tmp_path / "b", tmp_path / "c"
    stamp = b'#!/bin/sh\necho "llrm-cc abc123"\nexit 0\n'  # the binaries must answer --version; the rest is never run
    a.write_bytes(stamp + base)
    b.write_bytes(stamp + base[:100] + b"x" * 83 + base[183:])
    c.write_bytes(stamp + bytes(reversed(base)))
    for one in (a, b, c):
        one.chmod(0o755)
    assert voidpair.void(str(a), str(b)).startswith("VOID PAIR")
    assert voidpair.void(str(a), str(c)) == ""
    done = subprocess.run([sys.executable, str(HERE / "objsweep.py"), str(a), str(b), "--src", str(tmp_path)], capture_output=True, text=True)
    assert done.returncode == 2 and "VOID PAIR" in done.stdout


def test_runpinned_stops_when_the_dist_lacks_the_linker(tmp_path):
    """A VM without jwasm/jwlink/dosbox-x in the dist ran no test and the pair's run diff was empty, read as 'no new failure'."""
    dist = tmp_path / "dist"
    dist.mkdir()
    (dist / "llrm-cc").write_text("")
    repo = HERE.parents[1]
    done = subprocess.run(["bash", str(HERE / "runpinned.sh"), "0" * 9, str(dist), str(tmp_path / "out.txt")], capture_output=True, text=True, env={"PIN_TREES": str(tmp_path), "PATH": "/usr/bin:/bin:/usr/local/bin", "HOME": str(tmp_path)}, cwd=repo)
    assert done.returncode == 2 and "is missing" in done.stderr, done.stderr
    assert not (tmp_path / "out.txt").exists()


def test_rundiff_sets_aside_unbuilt_programs_and_names_a_new_failure():
    """A VM without V:\\LINK failed 137 programs per side; pairruns called the pair void, and the 7 that ran were not compared."""
    rundiff = load("rundiff")
    group = "[c] 3 programs: 1 pass, 0 known, 2 fail\n"
    base = group + 'FAIL  qb/a.bas: not built: Bad command or filename - "V:\\LINK"\n' + "FAIL  c/x.c [-O2]: wrong output\n"
    new = group + 'FAIL  qb/a.bas: not built: Bad command or filename - "V:\\LINK"\n' + "FAIL  c/y.c [-O2]: wrong output\n"
    added, fixed, census = rundiff.compare(base, new)
    assert added == ["NEW FAIL c/y.c [-O2]: wrong output"] and fixed == ["c/x.c [-O2]"]
    assert "not built (V:\\LINK etc.): base 1, new 1" in census


def test_bisect_stops_when_the_command_cannot_run(tmp_path):
    """A runner that refused stale binaries exited 1 like a failing test; every later probe read as bad and 40bc8dd60 was named the culprit of stft_single."""
    source = (HERE / "firstbad.py").read_text()
    assert "not 0 or 1" in source and "dist.glob(\"llrm-*\")" in source


def test_board_and_cbisect_say_what_they_need_instead_of_running():
    """Run without QCPORT or a worktree they print their usage and exit 2: a board with no corpus measured an empty compile once."""
    for script in ("board.py", "cbisect.py"):
        done = subprocess.run([sys.executable, str(HERE / script)], capture_output=True, text=True, env={"PATH": "/usr/bin:/bin"})
        assert done.returncode == 2, (script, done.stderr)


def fake_selector(path: Path, extra: int = 0, refuse: bool = False) -> str:
    """A stand-in llrm-cc with two functions on the `isel` channel; `--isel=dag` gives `g` EXTRA more instructions, or is refused."""
    path.write_text(
        '#!/bin/bash\n[ "$1" = --version ] && { echo "llrm-cc abc1234"; exit 0; }\n'
        'for a; do case $a in --isel=*) s=${a#--isel=};; esac; [ "$p" = -o ] && o=$a; p=$a; done\n'
        f'[ "$s" = dag -o "$s" = shadow ] && {{ [ {int(refuse)} = 1 ] && {{ echo "@f: --isel=dag: cannot select" >&2; exit 1; }}; n={extra}; }} || n=0\n'
        'echo x > "$o"\necho "[isel] f selector=$s instructions=4 copies=1 executed=9 memory=2" >&2\n'
        'echo "[isel] g selector=$s instructions=$((7+n)) copies=0 executed=20 memory=5" >&2\n'
    )
    path.chmod(0o755)
    return str(path)


def shadow(tmp_path, binary: str, *extra: str):
    src = tmp_path / "b"
    src.mkdir(exist_ok=True)
    (src / "p.c").write_text("int a;")
    return subprocess.run([sys.executable, str(HERE / "isel_shadow.py"), binary, "--src", str(src), "--levels", "O2", "--targets", "m32", *extra], capture_output=True, text=True)


def test_isel_shadow_old_against_old_is_empty(tmp_path):
    """The premise: a harness that reports a difference between a selector and itself reports nothing true."""
    done = shadow(tmp_path, fake_selector(tmp_path / "llrm"), "--new-isel", "old")
    assert done.returncode == 0, done.stdout
    assert "0 functions differ of 2; 1 of 1 cells identical, 0 refused" in done.stdout


def test_isel_shadow_shows_a_seeded_one_instruction_difference_as_one_function(tmp_path):
    done = shadow(tmp_path, fake_selector(tmp_path / "llrm", extra=1))
    assert done.returncode == 1
    assert "1 functions differ of 2" in done.stdout
    assert "p m32 -O2 g: instructions 7 -> 8" in done.stdout and " f:" not in done.stdout


def test_isel_shadow_counts_a_refused_compile_and_never_as_equal(tmp_path):
    done = shadow(tmp_path, fake_selector(tmp_path / "llrm", refuse=True))
    assert done.returncode == 1
    assert "1 refused" in done.stdout and "REFUSED p m32 -O2: old: ok; new: @f: --isel=dag: cannot select" in done.stdout


def test_isel_shadow_reads_m16_function_bytes_from_a_jwasm_listing():
    """m16 bytes were the object's total: a function that grew by one byte moved nothing the report named."""
    shadow = load("isel_shadow")
    listing = "00000000                        _id@3 proc far\n00000001                        _id@3 endp\n00000001                        _sum@3 proc far\n00000015                        _sum@3 endp\n"
    assert shadow.parse_listing(listing) == {"id": "1", "sum": "20"}


def test_isel_shadow_lists_a_function_the_dag_path_refused_and_never_as_equal():
    """`--isel=shadow` falls back to the old selector for a refused function: its row then equals the old one, and the report read zero."""
    shadow = load("isel_shadow")
    old = shadow.parse_rows("[isel] f selector=old instructions=4 copies=1 executed=9 memory=2\n")
    new = shadow.parse_rows("[isel] f selector=shadow instructions=4 copies=1 executed=9 memory=2\n[isel] f refused: --isel=dag: a function of several blocks\n")
    assert shadow.differences(old, new) == ["f: REFUSED --isel=dag: a function of several blocks"]


def test_the_pin_is_told_our_stack_alignment_and_pushes_a_one_argument_call():
    """The pin priced m32 at the Linux triple's 16-byte stack: it kept a reserved call frame (`subl $12,%esp; movl $2000,(%esp)`) where at our 4 it
    pushes, so pair csr counted 197 pushes and 206 `add esp` against the pin's 71 and 146."""
    sys.path.insert(0, str(HERE))
    import pin

    of = pin.target("m32")
    text = 'declare void @report(i32)\ndefine i32 @main(ptr %f) {\n  %r = call i32 %f(i32 2000)\n  call void @report(i32 %r)\n  ret i32 0\n}\n'
    told = pin.retarget(text, of)
    assert pin.stack_alignment(of) == 4 and '"override-stack-alignment", i32 4' in told
    if not (pin.PINNED_BIN / "llc").exists():
        return
    done = pin.llc(["-O2", "-filetype=asm", "-o", "-"], text, of)
    assert "pushl\t$2000" in done.stdout and "subl\t$12, %esp" not in done.stdout, done.stdout


def test_the_pins_clang_and_gcc_are_built_at_our_stack_alignment():
    """The reference objects of the rows (build.sh) and pinclocks.sh took the Linux 16-byte stack: a reserved call frame where our m32 target (S32) pushes."""
    build = (HERE.parents[1] / "crates/target/llrm-x86-m32/vsgcc/build.sh").read_text()
    assert "$CLANG $F -mstack-alignment=4" in build and "gcc $F -mpreferred-stack-boundary=2" in build
    assert build.count("-mstack-alignment=4") == 1
    assert "$CLANG $F -mstack-alignment=4" in (HERE / "pinclocks.sh").read_text()


def test_pair_builds_the_binaries_its_rows_and_sizes_steps_run():
    """pair csr: rows-report.txt was a traceback and sizes.txt one line, "stale binaries ... run cargo build --release --bins", because the tree's llrm-run and llrm-qb were older than the source."""
    text = (HERE / "pair.sh").read_text()
    build = text.index("cargo build --release --bins")
    assert build < text.index("nice -n 10 tools/rows/run.sh") and build < text.index("python3 tools/sizes.py")


def test_pindiff_pin_side_has_no_forced_frame_pointer():
    """pindiff ran the pin with -frame-pointer=all: every w-O*/*.pin.s opened `push ebp; mov ebp, esp`, so hanoi2/queens -Os read as encoding-byte gaps."""
    pindiff = load("pindiff")
    assert pindiff.forced_frame("f:\n\tpush\tebp\n\tmov\tebp, esp\n", "define void @f() {\n  ret void\n}\n")
    assert not pindiff.forced_frame("f:\n\tret\n", "define void @f() {\n  ret void\n}\n")
    assert not pindiff.forced_frame("f:\n\tpush\tebp\n\tmov\tebp, esp\n\tand\tesp, -8\n", "define void @f() {\n  ret void\n}\n")
    assert "-frame-pointer=all" not in (HERE / "pindiff.py").read_text()


def test_a_matrix_row_whose_pin_is_built_at_another_stack_alignment_is_refused_unless_labelled_cf6():
    """The m32 O1 matrix put the pin at the Linux 16-byte stack against our 4: select_of_constants (+39 against a matched pin) read -773, wc_mixed
    +15 read -79, and the cells looked closed or opened by the ABI and not by the code."""
    cells = load("m32cells")
    ours, matched, linux = cells.columns()
    assert (ours.stack_alignment, matched.stack_alignment, linux.stack_alignment) == (4, 4, 16)
    cells.check_row(ours, matched)
    cells.check_row(ours, linux)
    unlabelled = cells.Column("pin", 16)
    try:
        cells.check_row(ours, unlabelled)
    except cells.Refused as why:
        assert "stack alignment 16 against ours 4" in str(why)
    else:
        raise AssertionError("an unlabelled pin at 16 against ours at 4 was accepted")
    assert "-mstack-alignment=4" in cells.pin_flags(matched) and not any(f.startswith("-mstack-alignment") for f in cells.pin_flags(linux))


def test_m32run_links_a_non_pic_object_that_calls_through_the_plt_without_crashing(tmp_path):
    """spill_store_order and memfuncs_runtime segfaulted in `memset@plt` and read as base miscompiles: our -fno-pic objects were linked into clang's
    default PIE, whose PLT needs ebx = GOT."""
    run = load("m32run")
    clang = subprocess.run([sys.executable, str(HERE.parent / "pin_clang.py")], capture_output=True, text=True).stdout.strip()
    if not clang:
        return
    # What our objects do: a call with a PLT32 relocation and no GOT register set up.
    source = tmp_path / "a.s"
    source.write_text(".text\n.globl main\nmain:\n\tsubl $12, %esp\n\tpushl $64\n\tpushl $7\n\tpushl $buf\n\tcall memset@PLT\n\taddl $12, %esp\n\tmovsbl buf+63, %eax\n\tpushl %eax\n\tcall report\n\taddl $16, %esp\n\txorl %eax, %eax\n\tret\n.bss\n.lcomm buf, 64\n")
    shim = tmp_path / "r.c"
    shim.write_text(run.SHIM)
    for name, text in (("a", "s"), ("r", "c")):
        subprocess.run([clang, "-m32", "-fno-pic", "-c", str(tmp_path / f"{name}.{text}"), "-o", str(tmp_path / f"{name}.o")], check=True)
    link = run.link_command(clang, [str(tmp_path / "a.o"), str(tmp_path / "r.o")], str(tmp_path / "a.out"))
    if subprocess.run(link, capture_output=True).returncode:
        return  # no 32-bit libc on this host
    done = subprocess.run([str(tmp_path / "a.out")], capture_output=True)
    assert done.returncode == 0 and done.stdout == b"7\n", (done.returncode, done.stdout)


def assemble32(tmp_path: Path, name: str, text: str) -> Path:
    source = tmp_path / f"{name}.s"
    source.write_text(text)
    obj = tmp_path / f"{name}.o"
    subprocess.run(["as", "--32", str(source), "-o", str(obj)], check=True)
    return obj


def test_an_m32_cell_counts_the_constants_kept_outside_the_text(tmp_path):
    """The m32 matrix summed `.text` alone: ours with a 64 B table in .rodata and 8 B of code read better than a reference of 30 B of immediates and no
    table (72 vs 30 B once the table counts)."""
    cells = load("m32cells")
    table = assemble32(tmp_path, "table", ".text\nf: movl tab(,%eax,4), %eax\nret\n.section .rodata\ntab: .fill 64,1,7\n")
    immediates = assemble32(tmp_path, "imm", ".text\nf: cmpl $3, %eax\nja 1f\nmovl $5, %eax\nret\n1: movl $9, %eax\nmovl $11, %edx\nmovl $13, %ecx\nret\n")
    ours, other = cells.footprint(table), cells.footprint(immediates)
    assert ours.code < other.code, "premise: the code alone favours the table"
    assert (ours.data, other.data) == (64, 0)
    assert cells.worse(ours, other) == [f"size {cells.size(ours)} vs {cells.size(other)}", f"static {cells.static(ours)} vs {cells.static(other)}"]
    assert cells.worse(other, ours) == []


def test_a_table_moved_from_data_to_bss_is_not_worse_but_extra_bss_is(tmp_path):
    """#8466's rule: bss is judged only inside static (data + bss). A zero table moved .data -> .bss shrinks the size and leaves the footprint."""
    cells = load("m32cells")
    initialised = cells.footprint(assemble32(tmp_path, "d", ".text\nf: ret\n.data\ntab: .fill 40,1,0\n"))
    zeroed = cells.footprint(assemble32(tmp_path, "z", ".text\nf: ret\n.bss\n.lcomm tab, 40\n"))
    larger = cells.footprint(assemble32(tmp_path, "l", ".text\nf: ret\n.bss\n.lcomm tab, 60\n"))
    assert (initialised.data, initialised.bss, zeroed.data, zeroed.bss) == (40, 0, 0, 40)
    assert cells.worse(zeroed, initialised) == []
    assert cells.worse(larger, initialised) == ["static 60 vs 40"]
