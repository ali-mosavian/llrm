"""The spec-to-mermaid converter: one test per primitive."""
import pytest
from docsbuild import lint, tomermaid as tm


def conv(*lines):
    return tm.convert("\n".join(lines))


def text(*lines):
    return conv(*lines).text


def graph(*lines):
    return text("kind: graph", *lines)


def test_node_has_a_bold_first_line_and_small_rest_and_its_class():
    out = graph('node a "Title\\nline two\\nline three" fill=b')
    assert 'a["<b>Title</b><br/><small>line two<br/>line three</small>"]:::opt' in out


@pytest.mark.parametrize("fill,cls", [("a", "src"), ("b", "opt"), ("c", "out"), ("d", "data"), ("e", "warn")])
def test_fill_letters_name_classes(fill, cls):
    assert f":::{cls}" in graph(f'node a "x" fill={fill}')


def test_a_node_without_fill_is_mid_and_the_round_shape_is_a_stadium():
    out = graph('node a "x" shape=round')
    assert 'a(["<b>x</b>"]):::mid' in out


def test_note_shape_has_no_box_and_no_bold():
    out = graph('node a "free text" shape=note')
    assert 'a["free text"]:::note' in out and "classDef note" in out


def test_label_characters_that_break_mermaid_are_escaped():
    """A quote, a semicolon or a brace in a label closed the label early or ended the statement."""
    out = graph('node a "say \\"x\\"; #1 <T> {p} a|b"')
    assert "say #quot;x#quot;#59; #35;1 #lt;T#gt; #123;p#125; a#124;b" in out
    assert lint.check(out) == []


def test_reserved_word_and_dotted_ids_are_renamed():
    out = graph('node end "x"', 'node a.b "y"', "edge end -> a.b")
    assert "end_[" in out and "a_b[" in out and "end_ --> a_b" in out


@pytest.mark.parametrize("arrow,mm", [("->", "-->"), ("..>", "-.->"), ("==>", "==>")])
def test_edge_kinds(arrow, mm):
    assert f"  a {mm} b" in graph('node a "a"', 'node b "b"', f"edge a {arrow} b")


def test_edge_label_is_quoted_between_bars_and_keeps_line_breaks():
    out = graph('node a "a"', 'node b "b"', 'edge a -> b "two\\nlines"')
    assert 'a -->|"two<br/>lines"| b' in out


def test_direction_down_is_tb_and_right_is_lr():
    assert "flowchart TB" in graph('node a "a"')
    assert "flowchart LR" in text("kind: graph", "dir: right", 'node a "a"')


def test_group_is_a_subgraph_tinted_by_its_commonest_class():
    out = graph('node a "a" fill=a', 'node b "b" fill=a', 'node c "c"', 'group g "Front ends": a b c')
    assert 'subgraph g_g["FRONT ENDS"]' in out
    assert f"style g_g fill:{tm.TINT['src']}12,stroke:{tm.TINT['src']}" in out


def test_group_of_plain_nodes_is_grey_and_groups_nest():
    out = graph('node a "a"', 'node b "b"', 'group o "Outer": a b', 'group i "Inner": a')
    assert f"style g_o fill:{tm.TINT['mid']}12" in out
    assert out.index("subgraph g_o") < out.index("subgraph g_i") < out.index("end")


def test_a_fan_of_edges_into_a_group_ends_at_its_border():
    """Six edges into one group each crossed its caption; they now end at the group."""
    lines = ['node o "o"', *[f'node w{i} "w{i}"' for i in range(4)], 'group g "Writers": w0 w1 w2 w3']
    lines += [f"edge o -> w{i}" for i in range(4)]
    out = graph(*lines)
    assert out.count("o --> g_g") == 1 and "o --> w0" not in out


def test_two_edges_into_a_group_keep_their_nodes():
    out = graph('node o "o"', 'node a "a"', 'node b "b"', 'group g "G": a b', "edge o -> a", "edge o -> b")
    assert "o --> a" in out and "o --> b" in out


def test_title_is_the_caption_not_part_of_the_diagram():
    r = conv("kind: graph", "title: From source to object", 'node a "a"')
    assert r.caption() == "*From source to object*" and "From source" not in r.text


def test_only_the_classes_used_are_defined():
    out = graph('node a "a" fill=c')
    assert "classDef out" in out and "classDef src" not in out


def test_unknown_node_in_an_edge_names_the_line():
    with pytest.raises(tm.FigureError) as e:
        graph('node a "a"', "edge a -> z")
    assert e.value.line == 3 and "z" in e.value.message


# ---- stack

STACK = [
    "kind: stack",
    'arrow "higher" up',
    'cell "arg" off="bp+4" size="2" note="the argument"',
    'cell "ret" off="bp+2" size="2" fill=a',
    'gap "..."',
    'cell "locals" size="8"',
    'arrow "grows down" down',
]


def test_cell_is_offset_cell_size_note_in_eight_columns():
    out = text(*STACK)
    assert "block-beta" in out and "columns 8" in out
    assert 'o1["bp+4"] c1["arg"]:3 s1["2"] n1["the argument"]:3' in out
    assert lint.check(out) == []


def classes_of(out):
    """class name -> block ids, from the `class a,b name` lines."""
    return {l.split()[2]: l.split()[1].split(",") for l in out.split("\n") if l.strip().startswith("class ")}


def test_cell_fill_and_gap_have_their_classes():
    out = text(*STACK)
    cl = classes_of(out)
    assert "c2" in cl["src"] and "c1" in cl["cell"] and "c3" in cl["gap"]
    assert 'c3["..."]:3' in out


def test_arrows_are_caption_rows_with_a_triangle():
    out = text(*STACK)
    assert "▲ higher" in out and "▼ grows down" in out


def test_a_cell_without_a_size_has_no_dark_box():
    """An empty size cell drew as a dark block beside the cell."""
    cl = classes_of(text(*STACK))
    assert "s1" in cl["size"] and "s3" in cl["off"] and "s3" not in cl["size"]


def test_bracket_adds_a_column_and_puts_its_label_in_the_middle_notes():
    out = text("kind: stack", 'cell "a"', 'cell "b"', 'cell "c"', 'bracket "pushed by the caller" from=1 to=3')
    assert "columns 9" in out
    assert "<b>pushed by the caller</b>" in out.split("n2[")[1].split("\n")[0]
    assert "┐" in out and "│" in out and "┘" in out
    assert lint.check(out) == []


def test_bracket_outside_the_cells_is_an_error():
    with pytest.raises(tm.FigureError):
        text("kind: stack", 'cell "a"', 'bracket "x" from=1 to=2')


# ---- lanes

LANES = [
    "kind: lanes",
    "title: Borrow",
    'lane v "v (owner)"',
    'lane r "r = &v[0]"',
    'tick 1 "let"',
    'tick 2 "2"',
    'tick 3 "use"',
    'span v 1 3 "owner" kind=owner',
    'span r 2 3 "shared" kind=shared',
    'span r 1 2 "mutable" kind=mut',
    'span r 2 3 "plain" kind=info',
    'mark v 3 "refused: later" kind=error',
    'mark v 1 "fine" kind=ok',
]


def test_lanes_without_msg_are_a_gantt_with_a_section_per_lane():
    out = text(*LANES)
    assert out.split("\n")[1] == "gantt" and "section v (owner)" in out and "section r = &v[0]" in out
    assert "dateFormat X" in out and "tickInterval 1second" in out


def test_span_kinds_are_task_tags():
    out = text(*LANES)
    assert "owner :done, t8, 1, 3" in out
    assert "shared :active, t9, 2, 3" in out
    assert "mutable :crit, t10, 1, 2" in out
    assert "plain :t11, 2, 3" in out


def test_marks_are_milestones_and_a_colon_in_a_task_name_is_replaced():
    """A colon in a task name ended the name early and shifted the dates."""
    out = text(*LANES)
    assert "✗ refused∶ later :milestone, crit, t12, 3, 3" in out
    assert "✓ fine :milestone, done, t13, 1, 1" in out


def test_tick_labels_that_are_not_numbers_go_in_the_caption():
    r = conv(*LANES)
    assert "Axis: 1 let; 3 use." in r.caption() and r.title == "Borrow"


def test_a_one_column_span_is_one_unit_wide():
    out = text("kind: lanes", 'lane a "a"', 'span a 4 4 "x"')
    assert "x :t3, 4, 5" in out


SEQ = [
    "kind: lanes",
    'lane c "caller"',
    'lane f "callee"',
    'mark c 1 "push args" kind=ok',
    'msg c f 2 "call f" kind=call',
    'msg f c 4 "ret; done" kind=ret',
    'mark f 3 "body"',
]


def test_lanes_with_msg_are_a_sequence_in_column_order():
    out = text(*SEQ).split("\n")
    assert out[1] == "sequenceDiagram"
    assert "  participant c as caller" in out and "  participant f as callee" in out
    body = [x for x in out[4:]]
    assert body == [
        "  Note over c: ✓ push args",
        "  c->>f: call f",
        "  Note over f: body",
        "  f-->>c: ret#59; done",
    ]


def test_msg_between_one_lane_and_itself_is_an_error():
    with pytest.raises(tm.FigureError):
        text("kind: lanes", 'lane a "a"', 'msg a a 1 "x"')


@pytest.mark.parametrize("spec", [LANES, SEQ, STACK])
def test_every_converted_kind_passes_the_lint(spec):
    assert lint.check(text(*spec)) == []


def test_unknown_kind_and_missing_kind_are_errors():
    with pytest.raises(tm.FigureError):
        tm.convert("kind: pie")
    with pytest.raises(tm.FigureError):
        tm.convert('node a "x"')


OLD = "%%{init: {'theme':'base','themeVariables':{'fontSize':'15px'}}}%%"


@pytest.mark.parametrize("kind", list(tm.INITS))
def test_normalize_init_swaps_an_old_init_for_the_shared_one(kind):
    """Hand-written figures carried an older init line the lint rejected."""
    out = tm.normalize_init(f"{OLD}\n{kind}\n  x")
    assert out == f"{tm.INITS[kind]}\n{kind}\n  x"
    assert tm.normalize_init(out) == out


def test_normalize_init_leaves_other_types_and_initless_figures_alone():
    for text in (f"{OLD}\npie\n  a", "flowchart LR\n  a --> b"):
        assert tm.normalize_init(text) == text


def test_normalize_init_takes_the_type_from_the_line_after_the_init():
    assert tm.normalize_init(f"{OLD}\n\nflowchart TB\n  a").startswith(tm.INIT + "\n")
