"""passdocs.py extracts the `//!` headers into pages and reports headers that lack a required section."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import passdocs  # noqa: E402

GOOD = "//! Does a thing.\n//!\n//! # How it works\n//! Steps.\n//!\n//! # Runs\n//! Once.\n"


def tree(tmp_path, files):
    for name, text in files.items():
        path = tmp_path / "src" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    return tmp_path


def test_a_header_with_a_missing_section_is_reported(tmp_path, monkeypatch):
    """A header with no `# Runs` passed unnoticed, so a pass shipped without saying where it runs."""
    root = tree(tmp_path, {"a.rs": GOOD, "b.rs": "//! Does b.\n//!\n//! # How it works\n//! x\n", "c.rs": "fn f() {}\n"})
    monkeypatch.setattr(passdocs, "PAGES", [("p", "P", "src", "Runs")])
    assert passdocs.check(["src"], root) == ["src/b.rs: missing # Runs", "src/c.rs: missing summary, # How it works, # Runs"]


def test_analyses_need_provides_not_runs():
    lines = [line[4:] for line in GOOD.splitlines()]
    assert passdocs.missing(lines, "Provides") == ["# Provides"]
    assert passdocs.missing([x.replace("Runs", "Provides") for x in lines], "Provides") == []


def test_headings_are_demoted_two_levels_outside_fences():
    out = passdocs.render(["S.", "# How it works", "```text", "# not a heading", "```"])
    assert out == ["S.", "### How it works", "```text", "# not a heading", "```"]


def test_intra_doc_links_become_plain_code_and_real_links_stay():
    out = passdocs.render(["See [`crate::x`] and [`y`][z] and [`w`](http://e)."])
    assert out == ["See `crate::x` and `y` and [`w`](http://e)."]


def test_an_undocumented_file_is_listed_and_a_documented_one_has_a_section(tmp_path):
    root = tree(tmp_path, {"a.rs": GOOD, "bare.rs": "fn f() {}\n", "lib.rs": "", "a_tests.rs": "", "testing.rs": ""})
    text, done, bare = passdocs.page("T", "src", "Runs", root, "abc")
    assert (done, bare) == (1, 1)
    assert "## a\n" in text and "- [a](#a): Does a thing." in text
    assert "## Undocumented" in text and "src/bare.rs" in text and "blob/abc/src/a.rs" in text
    assert "lib.rs" not in text


def test_write_makes_the_index_and_three_pages(tmp_path):
    out = tmp_path / "out"
    passdocs.write(out, passdocs.ROOT, "main")
    assert sorted(p.name for p in out.iterdir()) == ["analyses.md", "backend.md", "index.md", "passes.md"]


def test_the_summary_is_the_first_sentence_unwrapped():
    """A wrapped first sentence listed as its first line only, cut mid-phrase in the table of contents."""
    assert passdocs.summary(["Does a", "thing well. Then more.", "", "Next."]) == "Does a thing well."
