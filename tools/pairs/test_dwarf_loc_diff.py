import dwarf_loc_diff as d

CODE = "0000000000000000 <f>:\n   0:\tlea    (%rdi),%eax\n   3:\tmov    %eax,0x0(%rip)        # 9 <f+0x9>\n"
SPAN = (0, 11)


def test_code_is_read_per_function_without_relocation_comments():
    assert d.code_of(CODE) == {"f": ["0 lea    (%rdi),%eax", "3 mov    %eax,0x0(%rip)"]}


def test_a_list_of_one_range_over_the_whole_function_is_its_expression():
    """`a` in the pin is an expression; ours was a one-entry list over the function: the same place, not a difference."""
    pin = {("f", "a"): ("expr", (85,))}
    ours = {("f", "a"): ("list", ((0, 11, (85,)),), SPAN)}
    assert d.compare(pin, ours) == {"same": [], "form": [("f", "a")], "different": [], "missing": []}


def test_a_range_that_ends_early_is_different_not_a_form():
    pin = {("f", "a"): ("expr", (85,))}
    ours = {("f", "a"): ("list", ((0, 9, (85,)),), SPAN)}
    assert d.compare(pin, ours)["different"] == [("f", "a")]


def test_a_variable_ours_has_no_place_for_is_missing():
    """`l` was in rax over [5,7) in the pin and had no location in ours; a list compare saw only the lists that existed."""
    pin = {("f", "l"): ("list", ((5, 7, (80,)),), SPAN)}
    assert d.compare(pin, {("f", "l"): ("none",)})["missing"] == [("f", "l")]


def test_a_global_is_the_same_place_whether_its_address_is_addr_or_addrx():
    pin = {("", "g"): ("expr", (161, 2))}
    ours = {("", "g"): ("expr", (3, 0, 0, 0, 0, 0, 0, 0, 0))}
    assert d.compare(pin, ours)["form"] == [("", "g")]
