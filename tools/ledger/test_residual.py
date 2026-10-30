import residual


INC = {"root": 1000, "glue": 300, "pass": 600, "alloc": 620}
INC2 = {"root": 1000, "a": 500, "b": 500, "shared": 700}


def test_the_walk_stops_at_a_listed_pass_and_keeps_the_rest():
    edges = {"root": [("pass", 600), ("glue", 300)], "glue": [("alloc", 120)], "pass": [("alloc", 500)]}
    own = residual.attribute(edges, INC, {"pass"}, "root")
    assert own["root"] == 100 and own["glue"] == 180 and own["alloc"] == 120 and "pass" not in own
    assert sum(own.values()) == 400


def test_a_shared_callee_is_charged_by_the_edge_not_twice():
    edges = {"root": [("a", 500), ("b", 500)], "a": [("shared", 400)], "b": [("shared", 300)]}
    own = residual.attribute(edges, INC2, set(), "root")
    assert own["shared"] == 700 and sum(own.values()) == 1000
