"""Markdown converter: constructs, and the defects the tree once had."""
from docsbuild.mdhtml import convert, slugify


def html(text, **kw):
    return convert(text, **kw).html


def test_headings_get_stable_unique_ids():
    r = convert("# Title `x`\n\n## A b\n\n## A b\n")
    assert [h[2] for h in r.headings] == ["title-x", "a-b", "a-b-1"]
    assert '<h2 id="a-b-1">' in r.html


def test_slug_drops_punctuation_keeps_hyphen_and_underscore():
    assert slugify("1. The `foo_bar` -- API (v2)!") == "1-the-foo_bar----api-v2"


def test_paragraph_joins_lines_and_hard_break():
    assert html("a\nb  \nc\\\nd") == "<p>a\nb<br>\nc<br>\nd</p>"


def test_bullets_numbers_and_one_nested_level():
    h = html("- a\n  - b\n  - c\n- d\n\n3. x\n4. y\n")
    assert "<ul>\n<li>a\n<ul>\n<li>b</li>\n<li>c</li>\n</ul></li>\n<li>d</li>\n</ul>" in h
    assert '<ol start="3">' in h


def test_list_item_continuation_line_stays_in_the_item():
    assert html("1. one\n   still one\n2. two\n") == "<ol>\n<li>one\nstill one</li>\n<li>two</li>\n</ol>"


def test_loose_list_wraps_items_in_paragraphs():
    assert "<li><p>a</p></li>" in html("- a\n\n- b\n")


def test_list_item_holds_a_code_fence():
    assert "<pre><code>x\n</code></pre>" in html("- a\n\n  ```\n  x\n  ```\n")


def test_fence_has_language_class_and_escapes():
    assert html("```asm\nmov <ax>\n```") == '<pre><code class="language-asm">mov &lt;ax&gt;\n</code></pre>'


def test_fig_fence_renders_inline_svg():
    r = convert('```fig\nkind: graph\nnode A "a"\n```')
    assert "<figure" in r.html and "<svg" in r.html and r.figures == 1 and not r.figure_errors


def test_bad_fig_fence_is_reported_with_the_document_line():
    r = convert('text\n\n```fig\nkind: graph\nnode A "a"\nedge A -> Z\n```')
    assert r.figure_errors == [(3 + 3, "unknown node 'Z'")] and "figerr" in r.html


def test_table_alignment_and_escaped_pipe():
    h = html("| a | b | c |\n|:--|:-:|--:|\n| 1 | `x\\|y` | 3 |\n")
    assert 'text-align:left' in h and 'text-align:center' in h and 'text-align:right' in h
    assert "<code>x|y</code>" in h


def test_blockquote():
    assert html("> a\n> b") == "<blockquote>\n<p>a\nb</p>\n</blockquote>"


def test_inline_code_links_emphasis_images():
    h = html("`a*b` [t `c`](x.md#y) **s** *e* _u_ snake_case ![alt](p.png)")
    assert "<code>a*b</code>" in h and '<a href="x.md#y">t <code>c</code></a>' in h
    assert "<strong>s</strong> <em>e</em> <em>u</em> snake_case" in h
    assert '<img src="p.png" alt="alt" loading="lazy">' in h


def test_link_hook_rewrites_hrefs_and_links_are_listed():
    r = convert("[a](x.md) ![i](y.png)", link_hook=lambda h: "R/" + h)
    assert 'href="R/x.md"' in r.html and 'src="R/y.png"' in r.html
    assert [h for _, h in r.links] == ["x.md", "y.png"]


def test_bare_url_and_autolink():
    h = html("see https://a.example/x). and <https://b.example>")
    assert '<a href="https://a.example/x">https://a.example/x</a>).' in h and 'href="https://b.example"' in h


def test_br_passes_through_other_html_is_unrecognised():
    r = convert("a<br>b <foo> c")
    assert "a<br>b" in r.html and "&lt;foo&gt;" in r.html
    assert [w for _, w in r.unrecognised] == ["raw HTML <foo>"]


def test_indented_code_block():
    assert "<pre><code>cargo test &lt;x&gt;\n</code></pre>" in html("## T\n\n    cargo test <x>\n")


def test_escapes_are_literal():
    assert html("\\*not em\\*") == "<p>*not em*</p>"


# The three defects below were in the tree and rendered wrongly without an error.


def test_closing_fence_with_text_is_flagged_not_swallowed():
    r = convert("```text\na\n``` Prose after the fence\nmore\n```\n")
    assert any("fence line with text" in w for _, w in r.unrecognised)


def test_unclosed_fence_is_flagged():
    assert any("never closed" in w for _, w in convert("```\nabc\n").unrecognised)


def test_table_row_with_extra_cell_is_flagged():
    r = convert("| a | b |\n|---|---|\n| 1 | 2 | 3 |\n")
    assert any("cells" in w for _, w in r.unrecognised)


def test_pipe_in_code_splits_a_table_cell_unless_escaped():
    r = convert("| a | b |\n|---|---|\n| `x|y` | 2 |\n")
    assert any("cells" in w for _, w in r.unrecognised)
    assert not convert("| a | b |\n|---|---|\n| `x\\|y` | 2 |\n").unrecognised
