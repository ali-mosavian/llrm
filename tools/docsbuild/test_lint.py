"""The structural lint of mermaid fences, and the optional render."""
import pytest
from docsbuild import lint, tomermaid as tm

GOOD = "\n".join([
    tm.INIT, "flowchart TB", "  classDef opt fill:#ea580c,stroke:#9a3412,color:#fff",
    '  a["<b>A</b>"]:::mid', '  b["<b>B</b><br/><small>x</small>"]:::opt',
    '  subgraph g_x["GROUP"]', '    c["C"]:::src', "  end",
    '  a -->|"label"| b', "  b ==> g_x", "  b -.-> c",
])


def swap(old, new, body=GOOD):
    assert old in body
    return body.replace(old, new)


def test_a_converted_figure_is_clean():
    assert lint.check(GOOD) == []


def test_the_first_line_must_be_the_shared_init():
    assert lint.check(swap(tm.INIT, tm.INIT.replace("15px", "16px")))
    assert lint.check(swap(tm.INIT, tm.BLOCK_INIT))


def test_the_diagram_type_must_be_known():
    assert lint.check(swap("flowchart TB", "pie"))


def test_unbalanced_quote_and_bracket_are_reported():
    assert any("quote" in e for e in lint.check(swap('a["<b>A</b>"]', 'a["<b>A</b>]')))
    assert any("[]" in e for e in lint.check(swap('c["C"]', 'c["C"')))


def test_raw_angle_bracket_in_a_label_is_reported_and_a_quoted_semicolon_is_not():
    assert any("angle" in e for e in lint.check(swap('"C"', '"a < b"')))
    assert lint.check(swap('"C"', '"a #lt; b#59; c"')) == []
    assert lint.check(swap('"C"', '"C; D"')) == []


def test_a_bare_semicolon_outside_quotes_is_reported():
    """`;` ends a statement in mermaid: a bare one splits the line."""
    assert any("';'" in e for e in lint.check(swap("b -.-> c", "b -.-> c; a --> b")))
    assert any("';'" in e for e in lint.check(swap("b -.-> c", 'b -.-> c; x["a;b"]')))


def test_an_undefined_class_is_reported_and_a_defined_one_is_not():
    assert any("'nope'" in e for e in lint.check(swap(":::src", ":::nope")))
    assert lint.check(swap(":::src", ":::mine", GOOD + "\n  classDef mine fill:#000")) == []


def test_an_edge_to_an_undeclared_node_is_reported():
    errs = lint.check(swap("b -.-> c", "b -.-> zz"))
    assert any("'zz'" in e for e in errs)


def test_an_edge_to_a_subgraph_id_is_fine():
    assert lint.check(GOOD) == []  # b ==> g_x


BLOCK = [tm.BLOCK_INIT, "block-beta", "  columns 4", '  a["x"]:3 b["y"]', '  space:1 c["z"]:3', "  classDef off fill:#000", "  class a,b off"]


def test_block_rows_must_fit_the_columns():
    assert lint.check("\n".join(BLOCK)) == []
    wide = "\n".join(BLOCK).replace('b["y"]', 'b["y"]:2')
    assert any("5 blocks in 4" in e or "row of" in e for e in lint.check(wide))
    assert any("columns" in e for e in lint.check("\n".join(BLOCK).replace("  columns 4\n", "")))


def test_sequence_endpoints_must_be_participants():
    seq = [tm.SEQUENCE_INIT, "sequenceDiagram", "  participant a as A", "  participant b as B", "  a->>b: hi"]
    assert lint.check("\n".join(seq)) == []
    assert lint.check("\n".join(seq).replace("a->>b", "a->>q"))


def test_gantt_passes_with_its_own_init():
    g = [tm.GANTT_INIT, "gantt", "  dateFormat X", "  section s", "  t :t1, 1, 2"]
    assert lint.check("\n".join(g)) == []


def test_fences_finds_languages_and_skips_text_blocks(tmp_path):
    doc = "x\n```mermaid\nA\n```\n\n````text\n```mermaid\nB\n```\n````\n"
    found = lint.fences(doc)
    assert [(l, lang) for l, lang, _ in found] == [(2, "mermaid"), (6, "text")]


def test_tree_check_reads_markdown_and_headers_and_rejects_fig_fences(tmp_path):
    root = tmp_path
    (root / "docs").mkdir()
    (root / "docs/a.md").write_text("```mermaid\n" + GOOD.replace("flowchart TB", "pie") + "\n```\n\n```fig\nkind: graph\n```\n")
    (root / "crates/x/src").mkdir(parents=True)
    (root / "crates/x/src/m.rs").write_text("//! S.\n//!\n//! ```fig\n//! kind: graph\n//! ```\n\nfn f() {}\n")
    (root / "crates/x/src/n.rs").write_text("//! S.\n//!\n//! ```mermaid\n//! " + "\n//! ".join(GOOD.replace("::: src", "").split("\n")) + "\n//! ```\n")
    errs = lint.check_tree(root)
    assert any(e.startswith("docs/a.md:1:") and "second line" in e for e in errs)
    assert any(e.startswith("docs/a.md:") and "legacy" in e for e in errs)
    assert any(e.startswith("crates/x/src/m.rs:3:") and "legacy" in e for e in errs)
    assert not any("n.rs" in e for e in errs)


def test_the_repository_figures_are_well_formed():
    assert lint.check_tree(lint.HERE.parent.parent) == []


def test_render_is_skipped_with_a_message_when_mmdc_is_absent(tmp_path, monkeypatch, capsys):
    monkeypatch.delenv("MMDC", raising=False)
    monkeypatch.setenv("PATH", str(tmp_path))
    assert lint.render_errors(tmp_path) == []
    assert "skipped" in capsys.readouterr().err


def test_render_finds_mmdc_only_through_the_environment(monkeypatch, tmp_path):
    monkeypatch.setenv("MMDC", "/x/mmdc")
    assert lint.find_mmdc() == "/x/mmdc"
    monkeypatch.delenv("MMDC")
    monkeypatch.setenv("PATH", str(tmp_path))
    assert lint.find_mmdc() is None


@pytest.mark.skipif(lint.find_mmdc() is None, reason="mmdc is not available")
def test_render_accepts_a_good_figure_and_flags_a_bad_one(tmp_path):
    (tmp_path / "docs").mkdir()
    (tmp_path / "docs/a.md").write_text("```mermaid\n" + GOOD.replace("b ==> g_x\n", "") + "\n```\n")
    assert lint.render_errors(tmp_path) == []
    (tmp_path / "docs/a.md").write_text("```mermaid\n" + tm.INIT + "\nflowchart TB\n  a -->\n```\n")
    assert lint.render_errors(tmp_path)


def flow(*edges):
    return "\n".join([tm.INIT, "flowchart TB", '  a["A"]', '  b["B"]', '  c["C"]', *("  " + e for e in edges)])


@pytest.mark.parametrize("edge", [
    "a -->|no| b", "a -- keep it --> b", "a ==>|x| b", "a -.->|x| b", "a -- text --- b",
    "a -->|x| b -->|y| c", "a -- p --> b -- q --> c", "a ==>|x| b ==>|y| c", "a -->|no keep| b & c",
])
def test_edge_labels_are_not_nodes(edge):
    """`|no| keep` was reported as an undeclared node."""
    assert lint.check(flow(edge)) == []


def test_an_undeclared_node_in_a_labelled_chain_is_still_reported():
    errs = lint.check(flow("a -->|x| b -->|y| zz"))
    assert any("'zz'" in e for e in errs)


def test_inline_declared_ids_and_subgraph_ids_count_as_declared():
    body = "\n".join([
        tm.INIT, "flowchart TB", "  classDef k fill:#000",
        '  subgraph s1 ["Group"]', '    b2["x"]:::k --> b3["y;z"]:::k', "  end",
        "  subgraph s2", "    q1((c))", "  end", "  b3 -->|go| s2", "  s1 --> q1",
    ])
    assert lint.check(body) == []


def test_fix_style_normalizes_the_figures_of_a_markdown_text_and_nothing_else():
    old = "%%{init: {'theme':'base'}}%%"
    text = f"a\n```mermaid\n{old}\nflowchart LR\n  a --> b\n```\n\n```text\n{old}\nflowchart LR\n```\n"
    fixed = lint.fix_style(text)
    assert fixed == text.replace(old, tm.INIT, 1)
    assert lint.fix_style(fixed) == fixed


def test_fix_style_rewrites_only_tracked_docs_pages(tmp_path):
    old = "%%{init: {'theme':'base'}}%%"
    fig = f"```mermaid\n{old}\nflowchart LR\n  a --> b\n```\n"
    (tmp_path / "docs").mkdir()
    (tmp_path / "docs/a.md").write_text(fig)
    (tmp_path / "README.md").write_text(fig)
    assert lint.fix_style_tree(tmp_path) == ["docs/a.md"]
    assert tm.INIT in (tmp_path / "docs/a.md").read_text()
    assert (tmp_path / "README.md").read_text() == fig
    assert lint.fix_style_tree(tmp_path) == []


OLD_DUMP = "_f@3: 10 instructions\n  block 0x0000\n    0x0000  v2 := mov 0x1  ; inserted\n    0x0003  jmp   ; inserted"
NEW_DUMP = "\n".join([
    "\u2550\u2550\u2550 _f@3 \u00b7 RegAlloc \u00b7 m16 \u2550\u2550\u2550", "fn _f@3(y4.1@ax) -> @eax", "frame  slot0 [bp-2] 2 B", "",
    "B0  entry  \u2192 B1  \u00d71.0", "     #2  bx         = mov.i16     1", "     ; ...", "    B1  \u2190 B0 B3  \u2192 B3 B2  loop",
    "         #8  ax         = sub.i16     ax, 1",
])


def test_a_lir_fence_in_the_old_dump_format_is_rejected():
    """The excerpts of ~30 headers kept `0x0003  v2 := mov 0x1  ; inserted` after the printer changed; they read as current."""
    errors = lint.lir_errors("a.md", 3, OLD_DUMP)
    assert len(errors) == 4 and "a.md:4" in errors[0]


def test_a_lir_fence_in_the_current_format_is_clean():
    assert lint.lir_errors("a.md", 3, NEW_DUMP) == []


def test_tree_check_reads_lir_fences_of_markdown_and_headers(tmp_path):
    (tmp_path / "docs").mkdir()
    (tmp_path / "docs/a.md").write_text("```lir\n" + OLD_DUMP + "\n```\n")
    (tmp_path / "crates/x/src").mkdir(parents=True)
    (tmp_path / "crates/x/src/m.rs").write_text("//! S.\n//!\n//! ```lir\n//! " + OLD_DUMP.replace("\n", "\n//! ") + "\n//! ```\n\nfn f() {}\n")
    errors = lint.check_tree(tmp_path)
    assert sum("LIR text format" in e for e in errors) == 8
