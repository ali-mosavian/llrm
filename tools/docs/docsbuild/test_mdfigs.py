"""mdfigs: figure images and fence specs become inline mermaid, once."""
from docsbuild import lint, mdfigs

SPEC = 'kind: graph\ntitle: A to B\nnode a "A" fill=a\nnode b "B"\nedge a -> b\n'


def doc_tree(tmp_path, doc, spec=SPEC):
    root = tmp_path / "repo"
    (root / "docs/assets/figures").mkdir(parents=True)
    (root / "docs/assets/figures/f.fig").write_text(spec)
    (root / "docs/assets/figures/f.png").write_bytes(b"png")
    (root / "docs/a").mkdir()
    (root / "docs/a/x.md").write_text(doc)
    return root


def convert(root):
    return mdfigs.convert_tree(root, [root / "docs/a/x.md"])


def test_image_becomes_a_fence_and_a_caption_and_the_files_are_deleted(tmp_path):
    root = doc_tree(tmp_path, "# X\n\n![alt text](../assets/figures/f.png)\n\nafter\n")
    n, problems = convert(root)
    text = (root / "docs/a/x.md").read_text()
    assert (n, problems) == (1, [])
    assert "```mermaid\n%%{init" in text and "\n```\n\n*A to B*\n\nafter\n" in text
    assert "f.png" not in text and not list((root / "docs/assets/figures").iterdir())


def test_alt_text_is_the_caption_when_the_figure_has_no_title(tmp_path):
    root = doc_tree(tmp_path, "![what it shows](../assets/figures/f.png)\n", SPEC.replace("title: A to B\n", ""))
    convert(root)
    assert "*what it shows*" in (root / "docs/a/x.md").read_text()


def test_html_image_converts_too_and_text_around_it_is_kept(tmp_path):
    root = doc_tree(tmp_path, 'before <img src="../assets/figures/f.png" alt="a"> after\n')
    convert(root)
    text = (root / "docs/a/x.md").read_text()
    assert text.startswith("before\n\n```mermaid") and text.rstrip().endswith("after")


def test_a_second_run_changes_nothing(tmp_path):
    root = doc_tree(tmp_path, "![t](../assets/figures/f.png)\n")
    convert(root)
    once = (root / "docs/a/x.md").read_text()
    assert convert(root) == (0, []) and (root / "docs/a/x.md").read_text() == once


def test_an_image_with_no_fig_beside_it_is_left_alone(tmp_path):
    root = doc_tree(tmp_path, "![shot](shot.png)\n\n```text\n![x](../assets/figures/f.png)\n```\n\n`![y](../assets/figures/f.png)`\n")
    before = (root / "docs/a/x.md").read_text()
    assert convert(root) == (0, []) and (root / "docs/a/x.md").read_text() == before
    assert (root / "docs/assets/figures/f.fig").is_file()


def test_a_fig_fence_in_a_document_is_converted_in_place(tmp_path):
    root = doc_tree(tmp_path, "# X\n```fig\n" + SPEC + "```\ntail\n")
    convert(root)
    text = (root / "docs/a/x.md").read_text()
    assert "```fig" not in text and "```mermaid" in text
    assert "\n\n```mermaid" in text and "*A to B*\n\ntail" in text


def test_a_bad_spec_is_reported_with_the_file_and_nothing_is_deleted(tmp_path):
    root = doc_tree(tmp_path, "![t](../assets/figures/f.png)\n", "kind: pie\n")
    n, problems = convert(root)
    assert n == 0 and "f.fig" in problems[0] and (root / "docs/assets/figures/f.png").is_file()


def test_converted_documents_pass_the_lint(tmp_path):
    root = doc_tree(tmp_path, "![t](../assets/figures/f.png)\n")
    convert(root)
    assert lint.check_tree(root) == []


HEADER = "//! Does a thing.\n//!\n//! ```fig\n//! kind: graph\n//! node a \"A\"\n//! ```\n//!\n//! # Runs\n//! Once.\n\nfn f() {}\n"


def test_header_fence_becomes_mermaid_with_the_prefix_kept():
    new, errors = mdfigs.convert_header(HEADER)
    assert errors == []
    lines = new.split("\n")
    assert all(l.startswith("//!") for l in lines[: lines.index("") ]) and "//! ```mermaid" in lines
    assert "//! ```fig" not in lines and new.endswith("\nfn f() {}\n")
    assert "//! # Runs" in lines


def test_header_conversion_is_idempotent_and_leaves_plain_headers_alone():
    once, _ = mdfigs.convert_header(HEADER)
    assert mdfigs.convert_header(once) == (once, [])
    plain = "//! Does a thing.\n//!\n//! # Runs\n//! Once.\n"
    assert mdfigs.convert_header(plain) == (plain, [])


def test_header_bad_spec_is_an_error():
    bad = HEADER.replace("kind: graph", "kind: pie")
    new, errors = mdfigs.convert_header(bad)
    assert new == bad and errors
