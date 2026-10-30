import importlib.util
import os
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).parent


def load(tmp_path, monkeypatch):
    work = tmp_path / "w"
    for name in ("base", "main"):
        (work / name).mkdir(parents=True)
    monkeypatch.setenv("ROWS_WORK", str(work))
    return work


def row(prog, variant, clocks, code=10):
    import json

    return json.dumps({"prog": prog, "variant": variant, "ins": clocks, "mem": 1, "misaligned": 0, "clocks": clocks, "code": code, "ok": True}) + "\n"


def test_a_row_crossing_min_of_gcc_and_clang_is_reported_newly_below(tmp_path, monkeypatch):
    """A row that was above the cheaper of gcc and clang and is now below it is named; one that only moved is not."""
    work = load(tmp_path, monkeypatch)
    levels = ("O1", "O2", "O3", "Os")
    for name, clocks in (("base", 120), ("main", 90)):
        lines = ""
        for level in levels:
            llrm = "llrm" if level == "O2" else "llrm" + level
            lines += row("p", llrm, clocks) + row("p", "gcc" + level, 100) + row("p", "clang" + level, 110)
        (work / name / "results.jsonl").write_text(lines)
    import json

    for name in ("base", "main"):
        (work / f"df-{name}.json").write_text(
            "".join(row("dfaces", v, 100) for v in ("llrm", "llrmO1", "llrmO3", "llrmOs", "gccO1", "gccO2", "gccO3", "gccOs", "clangO1", "clangO2", "clangO3", "clangOs"))
        )
    (work / "qcport.json").write_text(json.dumps([[n, "m", level, 5] for n in ("base", "main") for level in ("-O1", "-O2", "-O3", "-Os")]))
    spec = importlib.util.spec_from_file_location("analyse", HERE / "analyse.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    text = "\n".join(module.out)
    assert "newly below (better): ['p 1.200->0.900']" in text
    assert "newly above (worse): none" in text


def test_a_counter_that_was_zero_in_the_base_does_not_stop_the_table(tmp_path, monkeypatch):
    """A program with no memory operands has mem 0 in the base; the table divided by it and crashed on the ablation of `annotated`."""
    work = load(tmp_path, monkeypatch)
    import json

    levels = ("O1", "O2", "O3", "Os")
    for name, mem in (("base", 0), ("main", 7)):
        lines = ""
        for level in levels:
            llrm = "llrm" if level == "O2" else "llrm" + level
            for variant, memory in ((llrm, mem), ("gcc" + level, 1), ("clang" + level, 1)):
                lines += json.dumps({"prog": "p", "variant": variant, "ins": 5, "mem": memory, "clocks": 9, "code": 3, "ok": True}) + "\n"
        (work / name / "results.jsonl").write_text(lines)
    for name in ("base", "main"):
        (work / f"df-{name}.json").write_text(
            "".join(row("dfaces", v, 100) for v in ("llrm", "llrmO1", "llrmO3", "llrmOs", "gccO1", "gccO2", "gccO3", "gccOs", "clangO1", "clangO2", "clangO3", "clangOs"))
        )
    (work / "qcport.json").write_text(json.dumps([[n, "m", level, 5] for n in ("base", "main") for level in ("-O1", "-O2", "-O3", "-Os")]))
    spec = importlib.util.spec_from_file_location("analyse", HERE / "analyse.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    assert "p 0->7" in "\n".join(module.out)


def test_a_failed_row_stops_the_table_and_is_named_instead_of_leaving_the_geomean(tmp_path, monkeypatch):
    """33 wrong-output rows under LLRM_GREEDY=1 never appeared: the summary was computed over the rows that have counters."""
    import json

    work = load(tmp_path, monkeypatch)
    for name in ("base", "main"):
        lines = ""
        for level in ("O1", "O2", "O3", "Os"):
            llrm = "llrm" if level == "O2" else "llrm" + level
            for variant, prog in [(v, q) for v in (llrm, "gcc" + level, "clang" + level) for q in ("p", "r")]:
                bad = name == "main" and variant == "llrmO3" and prog == "p"
                lines += json.dumps({"prog": prog, "variant": variant, "ins": 5, "mem": 1, "clocks": 9, "code": 3, "ok": not bad, **({"error": "prints 3, want 4"} if bad else {})}) + "\n"
        (work / name / "results.jsonl").write_text(lines)
        (work / f"df-{name}.json").write_text("".join(row("dfaces", v, 100) for v in ("llrm", "llrmO1", "llrmO3", "llrmOs", "gccO1", "gccO2", "gccO3", "gccOs", "clangO1", "clangO2", "clangO3", "clangOs")))
    (work / "qcport.json").write_text(json.dumps([[n, "m", level, 5] for n in ("base", "main") for level in ("-O1", "-O2", "-O3", "-Os")]))
    spec = importlib.util.spec_from_file_location("analyse", HERE / "analyse.py")
    module = importlib.util.module_from_spec(spec)
    with pytest.raises(SystemExit, match=r"1 rows failed, 0 missing\nrows total 24 / with counters 47 of 48 / failed 1 / missing 0(.|\n)*p/llrmO3 \(failed in new: prints 3, want 4\)"):
        spec.loader.exec_module(module)
    monkeypatch.setattr("sys.argv", ["analyse.py", "--allow-failed"])
    spec.loader.exec_module(module)
    assert module.out[1].startswith("1 rows failed") and module.out[2].startswith("rows total 24") and "allowed: p/llrmO3" in module.out[3]


def object_with_comment(path, comment):
    import shutil
    import subprocess

    source = path.with_suffix(".c")
    source.write_text("int f(void) { return 1; }\n")
    subprocess.run(["gcc", "-c", str(source), "-o", str(path)], check=True)
    note = path.with_suffix(".txt")
    note.write_bytes(comment.encode() + b"\0")
    subprocess.run(["objcopy", "--update-section", f".comment={note}", str(path)], check=True)
    assert shutil.which("readelf")


def test_a_clang_object_built_by_the_distro_clang_stops_the_rows(tmp_path):
    """rows/main's *.clangO1.o were built by Ubuntu clang 20.1.8 (tools/rows build.sh called `clang` from PATH): nothing in the table said so."""
    import sys

    sys.path.insert(0, str(HERE))
    import rowcheck

    object_with_comment(tmp_path / "p.clangO1.o", "Ubuntu clang version 20.1.8 (0ubuntu4)")
    with pytest.raises(SystemExit, match=r"1 of 1 clang objects .* not built by the pinned clang.*p.clangO1.o: Ubuntu clang version 20.1.8"):
        rowcheck.compiler_premise(tmp_path)
    good = tmp_path / "good"
    good.mkdir()
    object_with_comment(good / "p.clangO1.o", "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)")
    assert "all built by the pinned clang" in rowcheck.compiler_premise(good)
    assert "premise unchecked" in rowcheck.compiler_premise(tmp_path / "empty")


def test_a_wrong_output_row_is_named_wrong_output_and_stays_out_of_the_geomean(tmp_path, monkeypatch):
    """x_gcd and x_list printed 0 (ok false, no error) under PHIElim: the census called them 'sized only, never run', and --allow-failed kept their
    2x and 7x 'wins' in the clocks geomean, which went out as a gain."""
    import json

    work = load(tmp_path, monkeypatch)
    for name in ("base", "main"):
        lines = ""
        for prog in ("p", "q"):
            for level in ("O1", "O2", "O3", "Os"):
                llrm = "llrm" if level == "O2" else "llrm" + level
                for variant in (llrm, "gcc" + level, "clang" + level):
                    bad = name == "main" and prog == "q" and variant == "llrmO3"
                    lines += json.dumps({"prog": prog, "variant": variant, "ins": 5, "mem": 1, "clocks": 1 if bad else 9, "code": 3, "ok": not bad, **({"got": [0]} if bad else {})}) + "\n"
        (work / name / "results.jsonl").write_text(lines)
        (work / f"df-{name}.json").write_text("".join(row("dfaces", v, 100) for v in ("llrm", "llrmO1", "llrmO3", "llrmOs", "gccO1", "gccO2", "gccO3", "gccOs", "clangO1", "clangO2", "clangO3", "clangOs")))
    (work / "qcport.json").write_text(json.dumps([[n, "m", level, 5] for n in ("base", "main") for level in ("-O1", "-O2", "-O3", "-Os")]))
    monkeypatch.setattr("sys.argv", ["analyse.py", "--allow-failed"])
    spec = importlib.util.spec_from_file_location("analyse", HERE / "analyse.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    text = "\n".join(module.out)
    assert "q/llrmO3 (failed in new: wrong output: got [0])" in text
    assert "| -O3 | 1.0000 | 1.0000 |" in text


def test_a_wrong_reference_row_is_failed_like_a_wrong_llrm_row():
    """The census reads `ok` of every variant: a gcc or clang binary that prints the wrong value must not become the min(gcc, clang) of a row."""
    sys.path.insert(0, str(HERE))
    import rowcheck

    rows = {"p/gccO2": {"ok": False, "got": [7], "ins": 1, "mem": 1, "clocks": 1, "code": 1}, "p/clangO2": {"ok": True, "ins": 1, "mem": 1, "clocks": 1, "code": 1}}
    line, bad = rowcheck.census({"base": rows})
    assert bad == ["p/gccO2 (failed in base: wrong output: got [7])"] and "failed 1" in line


def test_a_side_whose_d_faces_did_not_compile_is_named_and_the_report_still_comes(tmp_path, monkeypatch):
    """The #2773 pair's new side panicked compiling d_faces: df-main.json was never written and analyse.py died with FileNotFoundError, losing every table."""
    work = load(tmp_path, monkeypatch)
    import json

    for name in ("base", "main"):
        lines = ""
        for level in ("O1", "O2", "O3", "Os"):
            llrm = "llrm" if level == "O2" else "llrm" + level
            for variant in (llrm, "gcc" + level, "clang" + level):
                lines += json.dumps({"prog": "p", "variant": variant, "ins": 5, "mem": 1, "clocks": 9, "code": 3, "ok": True}) + "\n"
        (work / name / "results.jsonl").write_text(lines)
    (work / "df-base.json").write_text("".join(row("dfaces", v, 100) for v in ("llrm", "llrmO1", "llrmO3", "llrmOs", "gccO1", "gccO2", "gccO3", "gccOs", "clangO1", "clangO2", "clangO3", "clangOs")))
    (work / "df-main.log").write_text("internal error: a SCEVable type, not Float(Float)\n")
    (work / "qcport.json").write_text(json.dumps([[n, "m", level, 5] for n in ("base", "main") for level in ("-O1", "-O2", "-O3", "-Os")]))
    spec = importlib.util.spec_from_file_location("analyse", HERE / "analyse.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    text = "\n".join(module.out)
    assert "d_faces: NOT MEASURED, no result on main (internal error: a SCEVable type, not Float(Float))" in text and "rows total 12" in text


def test_the_second_arm_is_called_new_in_every_printed_line_and_the_arms_are_stated(tmp_path, monkeypatch):
    """The arm directory is named `main` but holds the NEW binary: 'failed in main' read as main failing, and a wrong-output verdict was written from it."""
    import json

    work = load(tmp_path, monkeypatch)
    (work / "arms.txt").write_text("arms: base=sbp-base llrm-c md5 aaaa exit 0; new=sbp-new llrm-c md5 bbbb exit 0")
    levels = ("O1", "O2", "O3", "Os")
    for name in ("base", "main"):
        lines = ""
        for level in levels:
            llrm = "llrm" if level == "O2" else "llrm" + level
            for variant in (llrm, "gcc" + level, "clang" + level):
                bad = name == "main" and variant == llrm
                for prog in ("p", "q"):
                    lines += json.dumps({"prog": prog, "variant": variant, "ins": 5, "mem": 1, "misaligned": 0, "clocks": 9, "code": 3, "ok": not (bad and prog == "p"), "got": [1]}) + "\n"
        (work / name / "results.jsonl").write_text(lines)
        (work / f"df-{name}.json").write_text("".join(row("dfaces", v, 100) for v in ("llrm", "llrmO1", "llrmO3", "llrmOs", "gccO1", "gccO2", "gccO3", "gccOs", "clangO1", "clangO2", "clangO3", "clangOs")))
    (work / "qcport.json").write_text(json.dumps([[n, "m", level, 5] for n in ("base", "main") for level in ("-O1", "-O2", "-O3", "-Os")]))
    monkeypatch.setattr(sys, "argv", ["analyse.py", "--allow-failed"])
    spec = importlib.util.spec_from_file_location("analyse", HERE / "analyse.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    text = "\n".join(module.out)
    assert "failed in new" in text and "failed in main" not in text
    assert text.startswith("arms: base=sbp-base llrm-c md5 aaaa exit 0; new=sbp-new llrm-c md5 bbbb exit 0")
