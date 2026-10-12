"""Figure renderer: one test per spec primitive, layout invariants, errors, determinism."""
import re
from pathlib import Path

import pytest

from docsbuild import figures
from docsbuild.figures import FigureError, build, render

DOC = Path(__file__).resolve().parents[2] / "docs" / "working" / "figures.md"
SOLID = ("node", "cell", "span", "mark")
EPS = 0.01


def doc_figures() -> list[str]:
    folder = DOC.parents[1] / "assets" / "figures"
    return [(folder / f"docs-working-figures-{n}.fig").read_text(encoding="utf-8") for n in (1, 2, 3)]


def overlap(a, b):
    return a.x < b.x + b.w - EPS and b.x < a.x + a.w - EPS and a.y < b.y + b.h - EPS and b.y < a.y + a.h - EPS


def check_invariants(fig):
    solids = [b for b in fig.boxes if b.kind in SOLID]
    for i, a in enumerate(solids):
        for b in solids[i + 1 :]:
            assert not overlap(a, b), f"{a.kind} {a.id} overlaps {b.kind} {b.id}"
    by_id = {b.id: b for b in fig.boxes}
    vx, vy, vw, vh = fig.viewbox
    for t in fig.texts:
        assert vx <= t.x0 and t.x1 <= vx + vw and vy <= t.y0 and t.y1 <= vy + vh, f"text outside viewBox: {t.text!r}"
        if t.owner:
            o = by_id[t.owner]
            assert o.x - 0.5 <= t.x0 and t.x1 <= o.x + o.w + 0.5, f"text {t.text!r} wider than {o.id}"
            assert o.y - 0.5 <= t.y0 and t.y1 <= o.y + o.h + 0.5, f"text {t.text!r} taller than {o.id}"
    for b in fig.boxes:
        assert vx <= b.x and b.x + b.w <= vx + vw and vy <= b.y and b.y + b.h <= vy + vh
    for lab in fig.labels:
        for b in solids:
            assert not overlap(lab, b), f"label {lab.id} overlaps {b.id}"
        for g in (b for b in fig.boxes if b.kind == "group"):
            assert not figures.crosses_border(lab, g, pad=0.0), f"label {lab.id} crosses the border of group {g.id}"


def members(spec, group):
    m = re.search(rf'^group {group} "[^"]*":(.*)$', spec, re.M)
    return m.group(1).split()


def check_groups(fig, spec):
    nodes = {b.id: b for b in fig.boxes if b.kind == "node"}
    for g in (b for b in fig.boxes if b.kind == "group"):
        mem = members(spec, g.id)
        for nid, n in nodes.items():
            inside = g.x <= n.x and n.x + n.w <= g.x + g.w and g.y <= n.y and n.y + n.h <= g.y + g.h
            assert inside == (nid in mem), f"{nid} vs group {g.id}"


# ---- the figures of the docs


def test_doc_figures_exist_and_hold_invariants():
    specs = doc_figures()
    assert [s.split("\n")[0] for s in specs] == ["kind: graph", "kind: stack", "kind: lanes"]
    for s in specs:
        check_invariants(build(s))


def test_doc_pipeline_groups_hold_exactly_their_nodes():
    s = doc_figures()[0]
    check_groups(build(s), s)


# ---- primitives

GRAPH = """kind: graph
node A "alpha" fill=a
node B "beta" shape=round
node C "gamma" shape=note
edge A -> B "go"
edge B ..> C
edge A ==> C
"""


def test_node_draws_label_fill_and_shape():
    fig = build(GRAPH)
    assert {b.id for b in fig.boxes if b.kind == "node"} == {"A", "B", "C"}
    assert 'class="nd f-a"' in fig.svg and ">alpha<" in fig.svg
    assert 'rx="18"' in fig.svg  # round: radius is half the height
    assert "<path class=\"nd\"" in fig.svg  # note
    check_invariants(fig)


def test_node_label_breaks_on_backslash_n():
    fig = build('kind: graph\nnode A "one\\ntwo"')
    box = fig.boxes[0]
    assert box.h >= 2 * figures.LH and fig.svg.count("<text") == 2


def test_edge_arrow_dash_bold_and_label():
    fig = build(GRAPH)
    paths = re.findall(r'<path class="(e[^"]*)" d', fig.svg)
    assert paths == ["e", "e dash", "e bold"]
    assert fig.svg.count("marker-end") == 3
    assert [lab.id for lab in fig.labels] == ["A->B"]


def test_edge_back_edge_is_drawn_outside_all_nodes():
    fig = build('kind: graph\nnode A "a"\nnode B "b"\nedge A -> B\nedge B -> A "again"')
    right = max(b.x + b.w for b in fig.boxes if b.kind == "node")
    assert min(lab.x for lab in fig.labels) > right
    check_invariants(fig)


def test_edge_self_loop_label_clear_of_neighbours():
    fig = build('kind: graph\nnode A "a"\nnode B "b"\nnode C "c"\nedge A -> B\nedge A -> C\nedge B -> B "retry"')
    check_invariants(fig)
    assert any(lab.id == "B->B" for lab in fig.labels)


def test_edge_long_edge_routes_around_nodes_and_labels_stay_clear():
    spec = (
        'kind: graph\nnode A "a"\nnode B "b"\nnode C "c"\nnode D "d"\n'
        'edge A -> B\nedge B -> C\nedge C -> D\nedge A -> D "long edge"\n'
    )
    check_invariants(build(spec))


def test_edge_right_direction_labels_fit_between_layers():
    spec = 'kind: graph\ndir: right\nnode A "a"\nnode B "b"\nnode C "c"\nedge A -> B "a rather long label"\nedge A -> C "other long one"\nedge B -> C\n'
    check_invariants(build(spec))


def test_group_contains_members_only():
    spec = (
        'kind: graph\nnode A "a"\nnode B "b"\nnode C "c"\nnode D "d"\nnode E "e"\n'
        'edge A -> C\nedge B -> C\nedge C -> D\nedge A -> E\n'
        'group g "Group caption": A B\ngroup h "Second": C D\n'
    )
    fig = build(spec)
    check_groups(fig, spec)
    check_invariants(fig)
    assert "Group caption" in fig.svg


def test_group_long_caption_stays_inside_its_box():
    fig = build('kind: graph\nnode A "a"\ngroup g "A very long caption for one node": A')
    g = next(b for b in fig.boxes if b.kind == "group")
    t = next(t for t in fig.texts if t.owner == "g")
    assert g.x <= t.x0 and t.x1 <= g.x + g.w


STACK = """kind: stack
arrow "up" up
cell "first" off="bp+4" size="2" note="a note" fill=b
cell "second" off="bp+2"
gap "..."
cell "third"
bracket "outer" from=1 to=3
bracket "inner" from=2 to=2
"""


def test_cell_label_offset_size_note_and_fill():
    fig = build(STACK)
    cells = [b for b in fig.boxes if b.kind == "cell"]
    assert [c.id for c in cells] == ["cell1", "cell2", "cell3", "cell4"]
    assert all(a.y + a.h == b.y for a, b in zip(cells, cells[1:]))  # stacked, no gaps
    for word in ("first", "bp+4", "a note", ">2<"):
        assert word in fig.svg
    assert "nd f-b" in fig.svg
    check_invariants(fig)


def test_cell_height_follows_size_only_with_scale():
    plain = build('kind: stack\ncell "a" size="2"\ncell "b" size="40"')
    scaled = build('kind: stack\nscale: yes\ncell "a" size="2"\ncell "b" size="40"')
    h = lambda f: [b.h for b in f.boxes if b.kind == "cell"]  # noqa: E731
    assert h(plain)[0] == h(plain)[1]
    assert h(scaled)[1] > h(scaled)[0]


def test_gap_is_a_dashed_cell_with_its_text():
    fig = build(STACK)
    assert 'class="nd gap"' in fig.svg
    gap = fig.boxes[2]
    assert gap.kind == "cell" and gap.h < fig.boxes[0].h


def test_bracket_spans_its_cells_and_nests_apart():
    fig = build(STACK)
    cells = [b for b in fig.boxes if b.kind == "cell"]
    paths = re.findall(r'<path class="br" d="M([\d.]+) ([\d.]+)H[\d.]+V([\d.]+)H', fig.svg)
    assert len(paths) == 2
    (x1, ya, yb), (x2, yc, yd) = ((float(a), float(b), float(c)) for a, b, c in paths)
    assert ya == cells[0].y + 3 and yb == cells[2].y + cells[2].h - 3  # 1..3 ends at the gap row
    assert x1 != x2  # overlapping ranges use different columns
    assert yc == cells[1].y + 3 and yd == cells[1].y + cells[1].h - 3


def test_arrow_points_the_written_way():
    up = build('kind: stack\narrow "x" up\ncell "a"')
    down = build('kind: stack\narrow "x" down\ncell "a"')
    d = lambda f: re.search(r'class="e" d="M[\d.]+ ([\d.]+)V([\d.]+)"', f.svg).groups()  # noqa: E731
    assert float(d(up)[0]) > float(d(up)[1])
    assert float(d(down)[0]) < float(d(down)[1])


LANES = """kind: lanes
lane a "caller"
lane b "callee"
tick 1 "enter"
tick 2 "work"
tick 3 "leave"
span a 1 3 "frame" kind=owner
span b 2 3 "body" kind=mut
span b 1 1 "stub" kind=shared
mark a 2 "oops" kind=error
mark b 3 "fine" kind=ok
msg a b 1 "call"
msg b a 3 "return" kind=ret
"""


def test_lane_bands_stack_in_written_order():
    fig = build(LANES)
    lanes = [b for b in fig.boxes if b.kind == "lane"]
    assert [lane.id for lane in lanes] == ["a", "b"]
    assert lanes[0].y + lanes[0].h < lanes[1].y
    assert any(t.text == "caller" for t in fig.texts)


def test_tick_labels_sit_under_the_axis_at_their_column():
    fig = build(LANES)
    lanes = [b for b in fig.boxes if b.kind == "lane"]
    bottom = lanes[-1].y + lanes[-1].h
    ticks = {t.text: t for t in fig.texts if t.text in ("enter", "work", "leave")}
    assert all(t.y0 > bottom for t in ticks.values())
    centres = [(t.x0 + t.x1) / 2 for t in (ticks["enter"], ticks["work"], ticks["leave"])]
    step = centres[1] - centres[0]
    assert step > 0 and abs((centres[2] - centres[1]) - step) < 0.5


def test_span_covers_exactly_its_columns_and_overlaps_stack_in_rows():
    fig = build(LANES)
    spans = {b.id: b for b in fig.boxes if b.kind == "span"}
    lane = next(b for b in fig.boxes if b.kind == "lane" and b.id == "b")
    colw = lane.w / 3
    body = next(b for b in spans.values() if abs(b.w - (2 * colw - 6)) < 0.5)
    assert abs(body.x - (lane.x + colw + 3)) < 0.5
    two = build('kind: lanes\nlane a "a"\nspan a 1 3 "one"\nspan a 2 4 "two"')
    s = [b for b in two.boxes if b.kind == "span"]
    assert s[0].y != s[1].y
    check_invariants(two)


def test_mark_is_a_point_in_its_column_with_a_kind():
    fig = build(LANES)
    assert "mk f-e" in fig.svg and "mk f-c" in fig.svg
    marks = [b for b in fig.boxes if b.kind == "mark"]
    lane = next(b for b in fig.boxes if b.kind == "lane" and b.id == "a")
    colw = lane.w / 3
    assert abs(marks[0].x + marks[0].w / 2 - (lane.x + 1.5 * colw)) < 0.5


def test_msg_is_an_arrow_between_lanes_at_its_column():
    fig = build(LANES)
    arrows = re.findall(r'<path class="(e[^"]*)" d="M([\d.]+) ([\d.]+)V([\d.]+)"', fig.svg)
    assert len(arrows) == 2
    call, ret = arrows
    assert call[0] == "e" and ret[0] == "e dash"
    a, b = (next(x for x in fig.boxes if x.kind == "lane" and x.id == i) for i in "ab")
    assert float(call[2]) == a.y + a.h and float(call[3]) == b.y  # down, gap to gap
    assert float(ret[2]) == b.y and float(ret[3]) == a.y + a.h  # up from the callee
    assert len(fig.labels) == 2
    check_invariants(fig)


def test_title_is_the_accessible_title_and_a_caption():
    fig = build("kind: lanes\ntitle: My caption\nlane a \"a\"")
    assert "<title" in fig.svg and ">My caption</title>" in fig.svg
    cap = [t for t in fig.texts if t.text == "My caption"]
    assert cap and cap[0].y0 >= max(b.y + b.h for b in fig.boxes)


def test_no_title_gets_a_default_accessible_title_and_a_description():
    svg = render('kind: graph\nnode A "a"')
    assert re.search(r"<title[^>]*>Graph</title>", svg) and re.search(r"<desc[^>]*>Graph, 1 nodes", svg)


# ---- the output as a whole


def test_svg_scales_dark_mode_and_self_contained():
    svg = render(GRAPH)
    assert re.search(r'<svg [^>]*viewBox="[-\d. ]+"', svg)
    assert not re.search(r"<svg[^>]* (width|height)=", svg)
    assert "@media (prefers-color-scheme:dark)" in svg and "var(--ink)" in svg
    assert "http://" not in svg.replace("http://www.w3.org/2000/svg", "")
    assert "<script" not in svg and "<image" not in svg


def test_same_input_gives_same_bytes():
    for s in doc_figures() + [GRAPH, STACK, LANES]:
        assert render(s) == render(s)


def test_text_is_escaped():
    svg = render('kind: graph\nnode A "a < b & c"')
    assert "a &lt; b &amp; c" in svg


def test_dense_graph_keeps_invariants():
    ids = "ABCDEFGH"
    spec = "kind: graph\n" + "".join(f'node {i} "node {i}"\n' for i in ids)
    for a, b in ["AB", "AC", "AD", "BE", "CE", "DF", "EG", "FG", "GH", "AG", "BH", "HA"]:
        spec += f'edge {a} -> {b} "{a}{b}"\n'
    check_invariants(build(spec))


# ---- errors


@pytest.mark.parametrize(
    "spec,line,word",
    [
        ("", 1, "empty"),
        ("node A", 1, "first line"),
        ("kind: pie", 1, "unknown kind"),
        ('kind: graph\nnode 9x "a"', 2, "needs an id"),
        ('kind: graph\nnode A "a"\nnode A "b"', 3, "duplicate"),
        ('kind: graph\nnode A "a" fill=z', 2, "fill"),
        ('kind: graph\nnode A "a" shape=hex', 2, "shape"),
        ('kind: graph\nnode A "a\n', 2, "unterminated"),
        ('kind: graph\nnode A "a"\nedge A -> Z', 3, "unknown node"),
        ('kind: graph\nnode A "a"\nedge A => A', 3, "edge syntax"),
        ('kind: graph\nnode A "a"\ngroup g "c": Q', 3, "unknown node"),
        ('kind: graph\ndir: up\nnode A "a"', 2, "dir"),
        ('kind: graph\nnode A "a"\nbogus 1', 3, "unknown directive"),
        ("kind: stack\ncell a", 2, "cell syntax"),
        ('kind: stack\ncell "a"\nbracket "b" from=1 to=2', 3, "outside"),
        ('kind: stack\ncell "a"\narrow "x" sideways', 3, "arrow syntax"),
        ('kind: lanes\nlane a "a"\nspan z 1 2 "s"', 3, "unknown lane"),
        ('kind: lanes\nlane a "a"\nspan a 3 1 "s"', 3, "ends before"),
        ('kind: lanes\nlane a "a"\ntick x "t"', 3, "positive integer"),
        ('kind: lanes\nlane a "a"\nmark a 1 "m" kind=bad', 3, "mark kind"),
        ('kind: lanes\nlane a "a"\nmsg a a 1 "m"', 3, "different lanes"),
    ],
)
def test_spec_errors_name_the_line(spec, line, word):
    with pytest.raises(FigureError) as e:
        build(spec)
    assert e.value.line == line and word in e.value.message


def test_error_text_carries_the_line():
    with pytest.raises(FigureError, match=r"^line 2: "):
        build('kind: graph\nnode A "a" fill=z')


def test_a_figure_keeps_its_natural_size_so_text_stays_readable():
    """A wide stack figure scaled to a phone's width drew 4px text."""
    svg = figures.render("kind: stack\ncell \"a long label to make this wide\" off=\"bp+4\" note=\"and a long note on the right\"\n")
    width = re.search(r'viewBox="[-\d. ]+? ([\d.]+) [\d.]+"', svg).group(1)
    assert f"min-width:{width}px" in svg and f"max-width:{width}px" in svg


def test_edge_label_crossing_a_group_border_keeps_clear_of_it():
    """In the pipeline figure the label "lowering boundary" sat on the Backend group's top border."""
    spec = (
        'kind: graph\nnode A "front"\nnode B "optimizer"\nnode C "selection"\nnode D "phases"\n'
        'edge A -> B\nedge B -> C "lowering boundary"\nedge C -> D\ngroup back "Backend": C D\n'
    )
    check_invariants(build(spec))


def test_lanes_make_room_for_multiline_mark_and_message_labels():
    """A three-line mark label ran over the axis and the lane below; a four-line message label over a lane."""
    spec = (
        'kind: lanes\nlane a "a"\nlane b "b"\ntick 1 "one"\ntick 2 "two"\n'
        'mark b 1 "l1\\nl2\\nl3"\nmsg a b 2 "m1\\nm2\\nm3\\nm4" kind=ret\n'
    )
    fig = build(spec)
    lanes = {b.id: b for b in fig.boxes if b.kind == "lane"}
    for t in fig.texts:
        if t.text.startswith("l1"):
            assert t.y1 <= lanes["b"].y + lanes["b"].h
        if t.text.startswith("m1"):
            assert t.y0 >= lanes["a"].y + lanes["a"].h and t.y1 <= lanes["b"].y


def repo_figures():
    root = Path(__file__).resolve().parents[2]
    return [pytest.param(f.read_text(), id=f.name) for f in sorted((root / "docs" / "assets" / "figures").glob("*.fig"))]


@pytest.mark.parametrize("spec", repo_figures())
def test_every_figure_in_the_docs_holds_the_layout_invariants(spec):
    """A converted diagram that overlaps itself reads as a defect in the page, and nobody lays out every figure by hand."""
    check_invariants(build(spec))


def test_fan_out_of_labelled_edges_keeps_every_label_inside_the_gap():
    """Four labelled edges from one node pushed one label down onto its target node (docs/abi/c.md)."""
    spec = 'kind: graph\nnode s "source"\n' + "".join(f'node t{i} "target {i}"\nedge s -> t{i} "label number {i}"\n' for i in range(4))
    check_invariants(build(spec))


def test_a_wide_group_caption_does_not_squeeze_the_label_of_an_edge_leaving_the_group():
    """docs/architecture/split.md: the caption widened the group into the gap and the label crossed its border."""
    spec = (
        'kind: graph\ndir: right\nnode a "A"\nnode b "B"\nnode c "C"\n'
        'edge a -> c "short"\nedge b -> c "decode, lift"\n'
        'group g "A very wide caption\\nfor a narrow group": a b\ngroup h "H": c\n'
    )
    check_invariants(build(spec))
