"""--sync: one page per documented header, the indexes, links, incremental keys, and --check."""
import passdocs
from docsbuild import build, lint, sync

ANALYSIS = "crates/opt/llrm-analysis/src"
PASSES = "crates/opt/llrm-transforms/src"
BACKEND = "crates/codegen/llrm-codegen/src/backend"
OUT = "docs/optimizations"


def header(summary, runs="Once.", extra="", example="No honest example: a helper.\n"):
    tail = f"//!\n//! # Example\n//! {example}" if example else ""
    return f"//! {summary}\n//!\n//! # How it works\n//! Steps.\n//!\n{extra}//! # Provides\n//! {runs}\n{tail}"


def repo(tmp_path, files):
    root = tmp_path / "repo"
    for name, text in files.items():
        (root / name).parent.mkdir(parents=True, exist_ok=True)
        (root / name).write_text(text, encoding="utf-8")
    return root


def synced(root, **kw):
    p = sync.plan(root, **kw)
    assert not p.errors, p.errors
    sync.apply(root, p)
    return p


BASE = {
    f"{ANALYSIS}/walk.rs": header("Walks the thing. It is long.", "`Walk` and a table | with a bar."),
    f"{ANALYSIS}/other.rs": header("Another one."),
    f"{ANALYSIS}/bare.rs": "fn f() {}\n",
    f"{ANALYSIS}/lib.rs": "//! skipped\n",
    f"{ANALYSIS}/walk_tests.rs": "//! skipped\n",
    f"{PASSES}/fold.rs": header("Folds."),
}


def test_a_documented_file_gets_a_page_with_banner_title_and_header(tmp_path):
    root = repo(tmp_path, BASE)
    synced(root)
    page = (root / OUT / "analyses/walk.md").read_text().split("\n")
    assert page[0] == (
        "> Generated from the `//!` header of "
        "[`crates/opt/llrm-analysis/src/walk.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/walk.rs); "
        "edit the header, not this file."
    )
    assert page[1].startswith("<!-- src: crates/opt/llrm-analysis/src/walk.rs sha256:")
    assert page[3:6] == ["# walk", "", "Walks the thing. It is long."]


def test_headings_are_demoted_one_level_outside_fences(tmp_path):
    body = header("S.", extra="//! ```text\n//! # not a heading\n//! ```\n//!\n//! ## Deeper\n//! x\n//!\n")
    root = repo(tmp_path, {f"{ANALYSIS}/a.rs": body})
    synced(root)
    page = (root / OUT / "analyses/a.md").read_text()
    assert "\n## How it works\n" in page and "\n## Provides\n" in page and "\n### Deeper\n" in page
    assert "\n# not a heading\n" in page


def test_mermaid_and_text_fences_are_copied_verbatim(tmp_path):
    fig = "//! ```mermaid\n//! flowchart TB\n//!   a --> b\n//! ```\n//!\n"
    root = repo(tmp_path, {f"{ANALYSIS}/a.rs": header("S.", extra=fig)})
    synced(root)
    assert "```mermaid\nflowchart TB\n  a --> b\n```" in (root / OUT / "analyses/a.md").read_text()


def test_rustdoc_links_become_links_to_the_sibling_page(tmp_path):
    files = dict(BASE, **{
        f"{ANALYSIS}/a.rs": header("Uses [`crate::walk`] and [`walk`] and [`crate::other::Item`] and [`crate::fold`].", "x"),
    })
    root = repo(tmp_path, files)
    synced(root)
    page = (root / OUT / "analyses/a.md").read_text()
    assert "[`crate::walk`](walk.md)" in page and "[`walk`](walk.md)" in page
    assert "[`crate::other::Item`](other.md)" in page


def test_a_link_to_a_page_of_another_directory_is_relative_and_one_to_a_bare_file_goes_to_github(tmp_path):
    files = dict(BASE, **{
        f"{ANALYSIS}/a.rs": header("See [`crate::fold`] and [`bare`] and [`Nothing`] and [`w`](http://x).", "x"),
        "crates/core/src/driver.rs": "//! d\n",
    })
    root = repo(tmp_path, files)
    synced(root)
    page = (root / OUT / "analyses/a.md").read_text()
    assert "[`crate::fold`](../passes/fold.md)" in page
    assert f"[`bare`](https://github.com/ali-mosavian/llrm/blob/main/{ANALYSIS}/bare.rs)" in page
    assert "`Nothing`" in page and "[`Nothing`]" not in page and "[`w`](http://x)" in page


def test_a_link_in_code_is_left_alone(tmp_path):
    body = header("S.", extra="//! ```text\n//! [`crate::walk`]\n//! ```\n//!\n")
    root = repo(tmp_path, dict(BASE, **{f"{ANALYSIS}/a.rs": body}))
    synced(root)
    assert "\n[`crate::walk`]\n" in (root / OUT / "analyses/a.md").read_text()


def test_index_is_a_sorted_table_with_summary_section_line_and_page_link(tmp_path):
    root = repo(tmp_path, BASE)
    synced(root)
    rows = (root / OUT / "analyses/readme.md").read_text().split("\n")
    table = [r for r in rows if r.startswith("| ") and not r.startswith("| Name")]
    assert [r.split("|")[1].strip() for r in table] == ["bare", "other", "walk"]
    assert "| bare | undocumented | | [source](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/bare.rs) |" in rows
    assert "| walk | Walks the thing. | `Walk` and a table \\| with a bar. | [walk](walk.md) |" in rows
    assert "| Name | Summary | Provides | Page |" in rows
    assert "2 documented, 1 undocumented" in "\n".join(rows)
    assert not (root / OUT / "analyses/bare.md").exists()


def test_the_other_two_directories_get_pages_and_indexes_even_when_empty(tmp_path):
    root = repo(tmp_path, BASE)
    synced(root)
    assert (root / OUT / "passes/fold.md").is_file() and (root / OUT / "passes/readme.md").is_file()
    empty = (root / OUT / "backend/readme.md").read_text()
    assert "0 documented, 0 undocumented" in empty and "| Name | Summary | Runs | Page |" in empty


def test_pipeline_header_is_a_page_linked_at_the_top_of_the_pass_index(tmp_path):
    files = dict(BASE, **{f"{PASSES}/pipeline.rs": header("The order.")})
    root = repo(tmp_path, files)
    synced(root)
    index = (root / OUT / "passes/readme.md").read_text()
    assert (root / OUT / "passes/pipeline.md").is_file()
    assert "[pipeline](pipeline.md), The order." in index.split("| Name")[0]
    assert "| pipeline |" not in index


def test_a_page_whose_source_is_gone_is_deleted_and_hand_files_are_not_touched(tmp_path):
    root = repo(tmp_path, BASE)
    synced(root)
    (root / ANALYSIS / "other.rs").unlink()
    (root / OUT / "passes.md").write_text("hand\n")
    p = synced(root)
    assert f"{OUT}/analyses/other.md" in p.deletes and not (root / OUT / "analyses/other.md").exists()
    assert (root / OUT / "passes.md").read_text() == "hand\n"


def test_a_file_that_loses_its_header_becomes_undocumented_and_loses_its_page(tmp_path):
    root = repo(tmp_path, BASE)
    synced(root)
    (root / ANALYSIS / "other.rs").write_text("fn f() {}\n")
    synced(root)
    assert not (root / OUT / "analyses/other.md").exists()
    assert "| other | undocumented |" in (root / OUT / "analyses/readme.md").read_text()


# ---- incremental

def tree_of(root):
    return {p.relative_to(root).as_posix(): p.read_bytes() for p in sorted(root.rglob("*")) if p.is_file()}


class Counter:
    def __init__(self, monkeypatch, module, name):
        self.calls = 0
        real = getattr(module, name)

        def counted(*a, **k):
            self.calls += 1
            return real(*a, **k)

        monkeypatch.setattr(module, name, counted)


def test_an_untouched_tree_writes_and_renders_nothing(tmp_path, monkeypatch):
    root = repo(tmp_path, BASE)
    synced(root)
    pages = Counter(monkeypatch, sync, "page_text")
    p = sync.plan(root)
    assert p.drift == [] and pages.calls == 0


def test_editing_one_header_rewrites_its_page_and_the_index_only(tmp_path, monkeypatch):
    root = repo(tmp_path, BASE)
    synced(root)
    (root / ANALYSIS / "other.rs").write_text(header("Changed."))
    pages = Counter(monkeypatch, sync, "page_text")
    p = sync.plan(root)
    assert pages.calls == 1
    assert sorted(p.writes) == [f"{OUT}/analyses/other.md", f"{OUT}/analyses/readme.md"]


def test_adding_a_file_rewrites_its_own_page_and_index_only(tmp_path, monkeypatch):
    """Adding one source file rewrote every generated page: the digest covered the names of all pages and all crate files."""
    root = repo(tmp_path, BASE)
    synced(root)
    (root / ANALYSIS / "fresh.rs").write_text(header("Fresh."))
    pages = Counter(monkeypatch, sync, "page_text")
    p = sync.plan(root)
    assert pages.calls == 1
    assert sorted(p.writes) == [f"{OUT}/analyses/fresh.md", f"{OUT}/analyses/readme.md"]


def test_a_source_file_outside_the_documented_directories_writes_nothing(tmp_path, monkeypatch):
    root = repo(tmp_path, BASE)
    synced(root)
    (root / "crates/frontends/x/src").mkdir(parents=True)
    (root / "crates/frontends/x/src/new.rs").write_text("fn f() {}\n")
    pages = Counter(monkeypatch, sync, "page_text")
    p = sync.plan(root)
    assert pages.calls == 0 and p.writes == {} and p.drift == []


def test_a_link_to_a_stem_that_gains_a_second_file_rewrites_that_page_only(tmp_path, monkeypatch):
    """A rustdoc link resolved to the one file of that name; a second file of the name makes it plain code."""
    files = dict(BASE, **{f"{ANALYSIS}/a.rs": header("Uses [`crate::util`].", "x"), "crates/x/src/util.rs": "fn f() {}\n"})
    root = repo(tmp_path, files)
    synced(root)
    assert "blob/main/crates/x/src/util.rs" in (root / OUT / "analyses/a.md").read_text()
    (root / "crates/y/src").mkdir(parents=True)
    (root / "crates/y/src/util.rs").write_text("fn g() {}\n")
    pages = Counter(monkeypatch, sync, "page_text")
    p = sync.plan(root)
    assert pages.calls == 1 and sorted(p.writes) == [f"{OUT}/analyses/a.md"]


def test_incremental_output_equals_a_cold_run(tmp_path):
    root = repo(tmp_path, BASE)
    synced(root)
    (root / ANALYSIS / "other.rs").write_text(header("Changed."))
    synced(root)
    cold = repo(tmp_path / "cold", dict(BASE, **{f"{ANALYSIS}/other.rs": header("Changed.")}))
    synced(cold)
    assert tree_of(root / OUT) == tree_of(cold / OUT)


def test_a_page_is_rewritten_when_a_name_it_links_to_appears(tmp_path):
    """A link to a page that did not exist yet stayed plain code after the page was added."""
    files = dict(BASE, **{f"{ANALYSIS}/a.rs": header("Uses [`crate::late`].", "x")})
    root = repo(tmp_path, files)
    synced(root)
    assert "[`crate::late`]" not in (root / OUT / "analyses/a.md").read_text()
    (root / ANALYSIS / "late.rs").write_text(header("Late."))
    synced(root)
    assert "[`crate::late`](late.md)" in (root / OUT / "analyses/a.md").read_text()


def test_a_changed_tool_digest_rewrites_every_page_and_force_does_too(tmp_path, monkeypatch):
    root = repo(tmp_path, BASE)
    synced(root)
    assert sync.plan(root, force=True).drift == []  # same bytes: nothing to write
    monkeypatch.setattr(sync, "tool_digest", lambda: "other")
    pages = Counter(monkeypatch, sync, "page_text")
    p = sync.plan(root)
    assert pages.calls == 3 and len(p.writes) == 3


# ---- --check

def test_check_names_the_command_on_drift(tmp_path, monkeypatch, capsys):
    root = repo(tmp_path, BASE)
    monkeypatch.setattr(build, "ROOT", root)
    assert build.main(["--check"]) == 1
    assert "build.py --sync" in capsys.readouterr().out
    build.main(["--sync"])
    assert build.main(["--check"]) == 0


def test_check_fails_on_a_malformed_figure_and_a_legacy_fence_in_a_header(tmp_path, monkeypatch, capsys):
    fig = "//! ```fig\n//! kind: graph\n//! ```\n//!\n"
    root = repo(tmp_path, dict(BASE, **{f"{ANALYSIS}/a.rs": header("S.", extra=fig)}))
    monkeypatch.setattr(build, "ROOT", root)
    build.main(["--sync"])
    capsys.readouterr()
    assert build.main(["--check"]) == 1
    assert "legacy" in capsys.readouterr().out


def test_check_lists_every_generated_page_without_an_example_heading(tmp_path, monkeypatch, capsys):
    """Pages shipped with no `## Example`; --check said nothing."""
    files = dict(BASE, **{f"{ANALYSIS}/a.rs": header("S.", example=""), f"{PASSES}/b.rs": header("T.", example="")})
    root = repo(tmp_path, files)
    monkeypatch.setattr(build, "ROOT", root)
    build.main(["--sync"])
    capsys.readouterr()
    assert build.main(["--check"]) == 1
    out = capsys.readouterr().out
    assert f"{ANALYSIS}/a.rs" in out and f"{PASSES}/b.rs" in out and "fold.rs" not in out
    assert sync.missing_example(root) == [f"{PASSES}/b.rs: no # Example", f"{ANALYSIS}/a.rs: no # Example"]


def test_check_with_render_runs_without_mmdc(tmp_path, monkeypatch, capsys):
    root = repo(tmp_path, BASE)
    monkeypatch.setattr(build, "ROOT", root)
    monkeypatch.delenv("MMDC", raising=False)
    monkeypatch.setenv("PATH", str(tmp_path))
    build.main(["--sync"])
    assert build.main(["--check", "--render"]) == 0
    assert "skipped" in capsys.readouterr().err


# ---- the repository itself

def test_repository_is_in_sync():
    p = sync.plan(build.ROOT)
    assert p.errors == [] and p.drift == [], sync.drift_message(p)


def test_repository_figures_are_well_formed():
    assert lint.check_tree(build.ROOT) == []


# Directories whose every file has the header.
HEADERS_REQUIRED = ["crates/opt/llrm-analysis/src", "crates/opt/llrm-transforms/src", "crates/codegen/llrm-codegen/src/backend"]


def test_every_file_of_a_required_directory_has_a_header():
    """A pass without its `//!` summary, `# How it works` and `# Runs`/`# Provides` is missing from the reference."""
    assert passdocs.check(HEADERS_REQUIRED, build.ROOT) == []


def test_docs_name_a_pass_through_its_page_not_its_source_file():
    """Docs linked `fold.rs` on GitHub where a page explains the pass; a permalink to a line is a code citation and stays."""
    import re
    stems = {f"{d}/{p.stem}" for d in ("passes", "analyses", "backend") for p in (build.ROOT / sync.OUT / d).glob("*.md")}
    pat = re.compile(r"\[`?(\w+)\.rs`?\]\(https://github\.com/[^)]*/crates/(?:opt/llrm-transforms/src|opt/llrm-analysis/src|codegen/llrm-codegen/src/backend)/\1\.rs\)")
    bad = []
    for md in sorted((build.ROOT / "docs").rglob("*.md")):
        if "/optimizations/" in md.as_posix() and md.parent.name in ("passes", "analyses", "backend"):
            continue
        for m in pat.finditer(md.read_text()):
            if any(s.endswith("/" + m.group(1)) for s in stems):
                bad.append(f"{md}: {m.group(0)}")
    assert bad == []


def test_the_page_format_is_pinned():
    """The key of a page holds `sync.FORMAT`, not the bytes of the tool, so moving or editing the tool rewrites no page (it once rewrote 417). A change to the layout
    below fails here until `FORMAT` is bumped."""
    text = sync.page_text("c/x/src/ab.rs", ["Sum.", "", "# How it works", "Body."], "passes", sync.Names({}, {}), "KEY")
    assert text == (
        f"> Generated from the `//!` header of [`c/x/src/ab.rs`]({sync.URL}/c/x/src/ab.rs); edit the header, not this file.\n"
        "<!-- src: c/x/src/ab.rs sha256:KEY -->\n\n# ab\n\nSum.\n\n## How it works\nBody.\n"
    )
    assert sync.tool_digest() == sync.FORMAT
