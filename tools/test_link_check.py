"""The link layer: a batch that touches what programs link against builds and links a QB and a C program."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import link_check as lc  # noqa: E402


def test_a_program_that_does_not_link_is_named_with_its_reason(tmp_path):
    """qbrt arena_type/bc_draw/color_masks died in jwlink on `memcpy_` at 50387f114 and the landing gate was green: nothing linked the QB runtime."""
    qb = lambda name, work: "jwlink: Error! E2028: memcpy_ is an undefined reference" if name == "arena_type" else None  # noqa: E731
    failures = lc.link_failures(tmp_path, qb=qb, c=lambda name, work: None)
    assert failures == ["qb arena_type: jwlink: Error! E2028: memcpy_ is an undefined reference"]
    assert lc.link_failures(tmp_path, qb=lambda n, w: None, c=lambda n, w: "x" if n == "memfuncs_runtime" else None) == ["c memfuncs_runtime: x"]


def test_only_a_change_to_what_programs_link_against_or_a_target_fact_asks_for_the_link_layer():
    assert lc.touches_runtime(["runtime/c/x86-m32/c.toml"])
    assert lc.touches_runtime(["runtime/qb/dev/dos/m32/gdscan.asm"])
    assert lc.touches_runtime(["crates/target/llrm-x86-m32/src/machines/calling.toml"])
    assert not lc.touches_runtime(["docs/working/readme.md", "crates/codegen/llrm-codegen/src/backend/memcpycall.rs", "tools/parity_squash.py"])


def test_the_landing_layer_links_for_a_runtime_change_and_refuses_what_does_not_link(capsys):
    import parity_squash as ps

    built = []
    bad = ["qb arena_type: jwlink: Error! E2028: memcpy_ is an undefined reference"]
    assert ps.new_link_failures(["docs/x.md"], build=lambda: built.append(1), check=lambda: bad) == []
    assert not built, "a docs change builds nothing"
    assert ps.new_link_failures(["runtime/c/x86-m32/c.toml"], build=lambda: built.append(1), check=lambda: bad) == bad
    assert built and "link: 3 QB + 2 C programs, 1 do not link" in capsys.readouterr().out


def test_a_dangling_link_is_refused_by_name_before_anything_is_built(tmp_path):
    """The #4972 batch crashed in the link stage on two links the re-org left dangling: no refusal, no name."""
    (tmp_path / "tests/qbrt").mkdir(parents=True)
    for name in lc.QB_PROGRAMS:
        (tmp_path / "tests/qbrt" / f"{name}.bas").write_text("")
    (tmp_path / "gone.txt").symlink_to("../old/gone.txt")
    listing = ["120000 aa 0\tgone.txt"]
    assert lc.unreadable_inputs(tmp_path, listing) == ["dangling link gone.txt -> ../old/gone.txt"]
    import parity_squash as ps

    built = []
    why = ps.new_link_failures(["runtime/c/x86-m32/c.toml"], build=lambda: built.append(1), check=lambda: [], inputs=lambda: lc.unreadable_inputs(tmp_path, listing))
    assert why == ["dangling link gone.txt -> ../old/gone.txt"] and not built
