"""`uv run --project tools python -m pytest tools/test_envflag.py`"""
import envflag


def test_an_off_word_is_off_and_anything_else_is_on():
    """LLRM_REQUIRE_BINARIES=0 made four tests fail on a missing binary instead of skipping: `os.environ.get(name)` is true of "0" (#2279)."""
    for off in [None, "", "0", "false", "FALSE", "off", "Off"]:
        assert not envflag.on(off), off
    for on in ["1", "yes", "true", "on", "00"]:
        assert envflag.on(on), on
    assert not envflag.flag("LLRM_X", {"LLRM_X": "0"}) and envflag.flag("LLRM_X", {"LLRM_X": "1"}) and not envflag.flag("LLRM_X", {})
