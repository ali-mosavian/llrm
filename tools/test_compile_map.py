import compile_map as m
import pinned_tests

STEPS = """[instr] by own work, in Minstr
[instr]      418.833 Minstr own      418.833 Minstr total     376x split placed
[instr]      366.213 Minstr own      563.906 Minstr total      46x mir hoist
[instr] total 7058.573 Minstr, outermost steps 7025.274 Minstr, outside every step 33.300 Minstr
"""

GCC = """Time variable                                   usr           sys          wall           GGC
 phase opt and generate             :   0.08 (100%)   0.01 (100%)   0.09 ( 82%)  4344k ( 67%)
 callgraph ipa passes               :   0.01 ( 12%)   0.00 (  0%)   0.01 (  9%)   582k (  9%)
 tree CCP                           :   0.01 ( 12%)   0.00 (  0%)   0.01 (  9%)  8264  (  0%)
 integrated RA                      :   0.02 ( 25%)   0.00 (  0%)   0.01 (  9%)   699k ( 11%)
 TOTAL                              :   0.08          0.01          0.11         6458k
"""


def test_the_steps_and_the_total_are_read_from_the_time_report():
    rows, total = m.parse_steps(STEPS)
    assert rows[0] == (376, 418.833, 418.833, "split placed") and rows[1][3] == "mir hoist"
    assert total == (7058.573, 7025.274, 33.3)


def test_gcc_containers_are_not_counted_with_what_they_contain():
    """The phase rows and `callgraph ipa passes` hold other timers: counted too, the rows summed to 112% of the total."""
    rows, total = m.parse_gcc(GCC)
    assert total == 0.08 and [name for name, _ in rows] == ["tree CCP", "integrated RA"]


def test_a_clang_event_is_charged_for_the_time_not_spent_in_the_events_inside_it():
    events = [
        {"ph": "X", "name": "OptFunction", "ts": 0, "dur": 100},
        {"ph": "X", "name": "RunPass", "ts": 10, "dur": 30, "args": {"detail": "InstCombine"}},
        {"ph": "X", "name": "RunPass", "ts": 50, "dur": 20, "args": {"detail": "SROA"}},
        {"ph": "X", "name": "Total OptFunction", "ts": 0, "dur": 100},
    ]
    own = m.self_times(events)
    assert own["OptFunction"] == 50 and own["RunPass InstCombine"] == 30 and own["RunPass SROA"] == 20 and "Total OptFunction" not in own


def test_every_step_has_an_owner_or_is_flagged_by_the_table():
    assert m.owner_of("analysis points-to") == ("analysis: pointers", "points-to")
    assert m.owner_of("regalloc spill") == ("regalloc", "allocator")
    assert m.owner_of("lir peephole") == ("lir passes", None)
    assert m.owner_of("mir hoist") == ("mir passes", None)


def test_the_map_names_its_commit_and_commands_and_flags_unowned_rows():
    pinned_tests.require_pin()
    result = {
        "header": {"commit": "abc test", "llrm": "llrm-c -m32 -O1 f.c", "gcc": "gcc", "clang": "clang"},
        "total_M": 1000.0, "instrumented_M": 1040.0, "rows_sum_M": 1030.0, "outside_every_step_M": 10.0, "scale": 1000 / 1040,
        "gcc_total_M": 100.0, "clang_total_M": 80.0,
        "rows": [{"name": "mir hoist", "count": 1, "own_M": 100.0, "total_M": 100.0, "category": "mir passes", "owner": None},
                 {"name": "regalloc spill", "count": 1, "own_M": 500.0, "total_M": 500.0, "category": "regalloc", "owner": "allocator"}],
        "categories": {"mir passes": {"llrm_M": 96.0, "gcc_M": 10.0, "clang_M": 8.0}, "regalloc": {"llrm_M": 480.0, "gcc_M": 40.0, "clang_M": 10.0}},
    }
    text = m.show(result)
    assert "abc test" in text and "llrm-c -m32 -O1 f.c" in text
    assert "UNOWNED" in text and "1 unowned row(s)" in text and "mir hoist (9.6%)" in text
    assert "WARNING" not in text
    result["rows_sum_M"] = 900.0
    assert "WARNING" in m.show(result)


def test_counters_are_read_by_event_and_the_best_cycle_count_is_the_figure(monkeypatch):
    """perf's `cycles:u` suffix once hid the event: the cycles read as zero and IPC as infinite."""
    out = ["1000,,cycles:u,1,100.00,,\n2000,,instructions:u,1,100.00,,\n", "1500,,cycles:u,1,100.00,,\n2000,,instructions:u,1,100.00,,\n"]
    runs = iter(out)
    monkeypatch.setattr(m, "run", lambda *a, **k: type("D", (), {"stderr": next(runs)})())
    got = m.cycles(["x"], runs=2)
    assert got["cycles_M"] == 0.001 and got["spread_pct"] == 50 and got["ipc"] == 2
