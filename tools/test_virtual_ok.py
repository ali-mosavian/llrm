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


# x87's real FPU cells (clocks, bytes): ours / gcc / pin clang; the same rows are pinned in vsgcc/test_fputable.py (#8509)
def fpu(ours, gcc, clang):
    return v.virtual_ok(ours, [gcc, clang], pin=clang)


def test_x_iir_is_virtual_ok():
    assert fpu((130528, 115), (60034, 172), (130542, 126))


def test_x_jpeg_single_is_worse_as_gcc_has_both_axes():
    assert not v.is_virtual([(78258540, 15886), (85045697, 21184)])
    assert not fpu((75281236, 17524), (78258540, 15886), (85045697, 21184))


def test_x_matmul_f_is_worse_as_the_pin_is_11_bytes_smaller():
    assert not fpu((67111, 246), (68373, 296), (71203, 235))


def test_matmul_is_worse_as_gcc_dominates_it():
    assert not fpu((18734, 307), (18060, 296), (19336, 303))


def test_gcc_equal_to_clang_on_both_axes_is_never_virtual():
    assert not v.is_virtual([(100, 10), (100, 10)])
    assert not fpu((90, 12), (100, 10), (100, 10))
