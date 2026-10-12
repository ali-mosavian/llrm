"""Site build: the repository converts clean; links, anchors and navigation."""
import re
from pathlib import Path

import pytest

from docsbuild import build

ROOT = Path(__file__).resolve().parents[2]


def make(tmp_path, files):
    for name, text in files.items():
        p = tmp_path / "repo" / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8")
    return tmp_path / "repo"


BASE = {
    "readme.md": "# Home\n\n[docs](docs/readme.md) ![pic](docs/a/pic.svg)\n",
    "agents.md": "# Rules\n",
    "docs/readme.md": "# Docs\n\n| Section | Holds |\n|---|---|\n| [a](a/readme.md) | x |\n| [root](../readme.md) | y |\n",
    "docs/a/readme.md": "# A\n\n[one](one.md#part) [up](../readme.md)\n",
    "docs/a/one.md": "# One\n\n## Part\n\n[self](#part)\n",
    "docs/a/pic.svg": "<svg xmlns='http://www.w3.org/2000/svg'/>",
}


def run(tmp_path, files):
    root = make(tmp_path, files)
    out = tmp_path / "out"
    return build.build(root, out), out


def test_repository_builds_with_no_problem(tmp_path):
    errors = build.build(ROOT, tmp_path / "site")
    assert errors == []
    site = tmp_path / "site"
    assert (site / ".nojekyll").exists() and (site / "index.html").exists()
    assert (site / "optimizations/reference/passes.html").exists()
    assert (site / "architecture/pipeline.svg").exists()


def test_every_markdown_file_gets_a_page(tmp_path):
    build.build(ROOT, tmp_path / "site")
    md = [p.relative_to(ROOT / "docs").with_suffix(".html") for p in (ROOT / "docs").rglob("*.md")]
    assert md and all((tmp_path / "site" / m).exists() for m in md)


def test_links_are_relative_to_the_new_page_and_keep_anchors(tmp_path):
    errors, out = run(tmp_path, BASE)
    assert errors == []
    assert 'href="one.html#part"' in (out / "a/readme.html").read_text()
    assert 'href="../index.html"' in (out / "a/readme.html").read_text()  # root readme is index.html
    assert 'src="a/pic.svg"' in (out / "index.html").read_text() and (out / "a/pic.svg").exists()


def test_unresolved_link_and_missing_anchor_are_reported_with_lines(tmp_path):
    files = dict(BASE, **{"docs/a/one.md": "# One\n\n[x](gone.md)\n\n[y](../readme.md#nope)\n"})
    errors, _ = run(tmp_path, files)
    assert any(e.startswith("docs/a/one.md:3:") and "does not resolve" in e for e in errors)
    assert any(e.startswith("docs/a/one.md:5:") and "#nope" in e for e in errors)


def test_figure_error_and_unconverted_construct_fail_the_build(tmp_path):
    files = dict(BASE, **{"docs/a/one.md": "# One\n\n```fig\nkind: graph\n```\n\n<blink>\n"})
    errors, _ = run(tmp_path, files)
    assert any("figure:" in e for e in errors) and any("unconverted construct" in e for e in errors)


def test_links_to_files_outside_the_site_point_at_the_repository(tmp_path):
    files = dict(BASE, **{"docs/a/one.md": "# One\n\n## Part\n\n[c](../../crates/x.rs#L3) [d](../../crates)\n", "crates/x.rs": "", "crates/y": ""})
    errors, out = run(tmp_path, files)
    page = (out / "a/one.html").read_text()
    assert errors == []
    assert "/blob/main/crates/x.rs#L3" in page and "/tree/main/crates" in page


def test_navigation_follows_the_section_readmes_and_marks_the_page(tmp_path):
    _, out = run(tmp_path, BASE)
    nav = re.search(r"<nav.*?</nav>", (out / "a/one.html").read_text(), re.S).group(0)
    assert nav.index(">Docs<") < nav.index(">A<") < nav.index(">One<")
    assert 'aria-current="page">One<' in nav


def test_unlinked_page_is_still_in_the_navigation(tmp_path):
    _, out = run(tmp_path, dict(BASE, **{"docs/a/lost.md": "# Lost\n"}))
    assert ">Lost<" in (out / "index.html").read_text()


def test_same_input_same_files(tmp_path):
    build.build(ROOT, tmp_path / "one")
    build.build(ROOT, tmp_path / "two")
    names = sorted(p.relative_to(tmp_path / "one") for p in (tmp_path / "one").rglob("*") if p.is_file())
    assert names == sorted(p.relative_to(tmp_path / "two") for p in (tmp_path / "two").rglob("*") if p.is_file())
    assert all((tmp_path / "one" / n).read_bytes() == (tmp_path / "two" / n).read_bytes() for n in names)


def test_check_exits_nonzero_on_a_problem(tmp_path, monkeypatch, capsys):
    root = make(tmp_path, dict(BASE, **{"docs/a/one.md": "# One\n\n[x](gone.md)\n"}))
    monkeypatch.setattr(build, "ROOT", root)
    assert build.main(["--check"]) == 1
    assert "does not resolve" in capsys.readouterr().out


def test_check_passes_on_the_repository(capsys):
    assert build.main(["--check"]) == 0
