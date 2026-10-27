"""passdocs.py reads the `//!` headers and reports the ones that lack a required section."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import passdocs  # noqa: E402

GOOD = "//! Does a thing.\n//!\n//! # How it works\n//! Steps.\n//!\n//! # Runs\n//! Once.\n//!\n//! # Example\n//! None.\n"


def tree(tmp_path, files):
    for name, text in files.items():
        path = tmp_path / "src" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    return tmp_path


def test_a_header_with_a_missing_section_is_reported(tmp_path, monkeypatch):
    """A header with no `# Runs` passed unnoticed, so a pass shipped without saying where it runs."""
    root = tree(tmp_path, {"a.rs": GOOD, "b.rs": "//! Does b.\n//!\n//! # How it works\n//! x\n//!\n//! # Example\n//! y\n", "c.rs": "fn f() {}\n"})
    monkeypatch.setattr(passdocs, "PAGES", [("p", "P", "src", "Runs")])
    assert passdocs.check(["src"], root) == ["src/b.rs: missing # Runs (or # Used by)", "src/c.rs: missing summary, # How it works, # Example, # Runs (or # Used by)"]


def test_a_header_needs_one_of_runs_used_by_or_provides():
    """Helpers are not pipeline phases: `# Used by` satisfies the section; a header with none of the three is reported."""
    lines = [line[4:] for line in GOOD.splitlines()]
    assert passdocs.missing([x for x in lines if "Runs" not in x], "Runs") == ["# Runs (or # Used by)"]
    for name in ("Provides", "Used by"):
        assert passdocs.missing([x.replace("Runs", name) for x in lines], "Runs") == []


def test_a_header_without_an_example_section_is_reported(tmp_path, monkeypatch):
    """Pages shipped with no `# Example`: the generated page showed the pass on no input at all."""
    no_example = GOOD.split("//!\n//! # Example")[0]
    root = tree(tmp_path, {"a.rs": no_example, "b.rs": GOOD})
    monkeypatch.setattr(passdocs, "PAGES", [("p", "P", "src", "Runs")])
    assert passdocs.check(["src"], root) == ["src/a.rs: missing # Example"]










def test_the_summary_is_the_first_sentence_unwrapped():
    """A wrapped first sentence listed as its first line only, cut mid-phrase in the table of contents."""
    assert passdocs.summary(["Does a", "thing well. Then more.", "", "Next."]) == "Does a thing well."


def test_section_line_is_the_first_line_under_the_first_of_the_headings():
    lines = ["S.", "# Used by", "", "the loop passes", "more", "# Runs", "once"]
    assert passdocs.section_line(lines, ("Runs", "Used by")) == "once"
    assert passdocs.section_line(lines, ("Used by",)) == "the loop passes"
    assert passdocs.section_line(lines, ("Provides",)) == ""
    assert passdocs.section_line(["# Runs", "```text", "# Provides", "```", "x"], ("Runs",)) == "x"


def test_a_file_named_by_others_has_a_generated_used_by_and_a_prose_paragraph_explains_it(tmp_path):
    """78 files of the pass directories failed the header check: they had no `# Runs` or `# How it works` heading, though other files named them and their headers explained the pass."""
    src = tmp_path / "crates/x/src"
    src.mkdir(parents=True)
    header = "//! A pass that does one thing.\n//!\n//! It looks at every branch in a function and rewrites the ones whose condition it has already seen.\n//!\n//! # Example\n//! `br %c` is `br true`.\n"
    (src / "pass.rs").write_text(header + "pub fn run() {}\n")
    (src / "driver.rs").write_text("//! d\nuse crate::pass; fn f() { pass::run() }\n")
    users = passdocs.users_of(src, tmp_path)
    assert users["pass"] == ["driver"]
    lines = passdocs.header(src / "pass.rs")
    assert passdocs.missing(lines, "Runs") == ["# Runs (or # Used by)"]
    assert passdocs.missing(lines, "Runs", users["pass"]) == []
    assert passdocs.with_users(lines, users["pass"])[-1] == "Named by `driver`."
    assert passdocs.with_users(lines, []) == lines
    bare = passdocs.header_of("//! A pass.\n//!\n//! # Example\n//! x\n")
    assert "# How it works" in passdocs.missing(bare, "Runs", ["driver"])
