import debug_identity as d


def test_a_debug_section_is_not_a_difference():
    off = {".text": b"\x90", ".data": b"1"}
    on = {".text": b"\x90", ".data": b"1", ".debug_info": b"x", ".rela.debug_info": b"r"}
    assert d.differences(off, on) == []


def test_changed_code_bytes_are_a_difference():
    """A mark that moved an instruction changed `.text`; only the debug sections were allowed to."""
    assert d.differences({".text": b"\x90\x90"}, {".text": b"\x90"}) == [".text"]


def test_a_section_only_one_side_has_is_a_difference():
    assert d.differences({".text": b"a"}, {".text": b"a", ".extra": b"b"}) == [".extra"]


def test_the_positive_control_needs_debug_sections_in_the_g_object():
    """A -g object with no .debug_* section proves nothing about the code: the leg did not run -g."""
    assert d.has_debug({".text": b"a", ".debug_info": b"x"})
    assert not d.has_debug({".text": b"a"})
