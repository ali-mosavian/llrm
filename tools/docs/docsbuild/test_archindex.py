"""docs/architecture/readme.md's Contents list is built from the pages, so a docs PR never edits the shared line."""
from __future__ import annotations

from pathlib import Path

from docsbuild import archindex

ROOT = Path(__file__).resolve().parents[3]


def tree(tmp_path: Path, pages: dict[str, str]) -> Path:
    (tmp_path / "docs/architecture").mkdir(parents=True)
    (tmp_path / "docs/architecture/readme.md").write_text(f"# Architecture\n\n{archindex.START}\nstale\n{archindex.END}\n\nbody\n")
    for name, text in pages.items():
        (tmp_path / "docs/architecture" / name).write_text(text)
    return tmp_path


def page(text: str, order: int) -> str:
    return f"---\narchitecture-index: {text}\narchitecture-order: {order}\n---\n# Title\n"


def test_the_list_follows_the_pages_order_then_path_and_links_relative_to_the_readme(tmp_path):
    root = tree(tmp_path, {"b.md": page("B: second", 20), "a.md": page("A", 20), "c.md": page("C", 10), "none.md": "# no front matter\n"})
    assert archindex.render(root).startswith("Contents: [C](c.md), [A](a.md), [B: second](b.md)")
    assert "none.md" not in archindex.render(root)


def test_a_new_page_appears_in_the_list_without_editing_the_readme_line(tmp_path):
    """Two docs PRs each added their page to the one Contents line and conflicted on it every time."""
    root = tree(tmp_path, {"a.md": page("A", 10)})
    archindex.sync(root)
    first = (root / archindex.README).read_text()
    (root / "docs/architecture/new.md").write_text(page("New", 30))
    assert archindex.drift(root), "the readme does not list the new page: stale"
    archindex.sync(root)
    assert "[New](new.md)" in (root / archindex.README).read_text() and not archindex.drift(root)
    assert first.count("[A](a.md)") == 1


def test_the_committed_readme_is_current():
    """A page added or retitled without `build.py --sync` leaves the list stale: the check fails."""
    assert not archindex.drift(ROOT), "run python3 tools/docs/docsbuild/build.py --sync"


def test_front_matter_holds_only_a_block_that_opens_the_file():
    assert archindex.front_matter("# Title\n---\narchitecture-index: x\n---\n") == {}
    assert archindex.front_matter("---\narchitecture-index: a: b\n---\n") == {"architecture-index": "a: b"}
