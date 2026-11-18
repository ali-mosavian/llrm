"""libcalls.py: a cell whose columns call different runtime functions is listed, one with equal calls is not."""
import os, sys, tempfile
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import libcalls


def test_a_column_that_calls_memset_beside_one_that_inlines_it_is_listed():
    with tempfile.TemporaryDirectory() as out:
        for name, text in {'rep_count.2.ccn': '', 'rep_count.2.clangn': 'memset\n', 'sum.2.ccn': 'memcpy\n', 'sum.2.clangn': 'memcpy\n'}.items():
            open(os.path.join(out, name + '.libcalls'), 'w').write(text)
        assert [key for key, _ in libcalls.differing(libcalls.calls(out))] == [('rep_count', '2')]
