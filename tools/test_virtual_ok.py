import virtual_ok as v

REFS = [(12, 90), (20, 70), (8, 100)]  # (bytes, clocks): bars (8, 70), no reference has both


def test_a_cell_no_single_reference_attains_is_virtual():
    assert v.is_virtual(REFS)


def test_a_cell_one_reference_attains_is_not_virtual():
    assert not v.is_virtual([(12, 90), (8, 70)])
    assert not v.is_virtual([(8, 70), (8, 70)])


def test_pareto_optimal_and_not_behind_the_pin_passes():
    assert v.virtual_ok((9, 80), REFS, pin=(12, 90))


def test_a_dominated_value_does_not_pass():
    assert not v.virtual_ok((9, 100), REFS + [(8, 95)], pin=(12, 120))  # ahead of the pin, so only dominance fails it


def test_a_value_behind_the_pin_on_either_axis_does_not_pass():
    assert not v.virtual_ok((13, 60), REFS, pin=(12, 90))
    assert not v.virtual_ok((9, 100), REFS, pin=(12, 90))


def test_the_rule_is_axis_count_agnostic():
    assert v.virtual_ok((1, 1, 1), [(0, 2, 2), (2, 0, 2), (2, 2, 0)], pin=(2, 2, 2))
