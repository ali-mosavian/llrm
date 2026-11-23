"""A module with an empty .text is a measured 0, not a missing bar (qbbars once dropped it: 78 of 79 counted)."""
import re, subprocess, tempfile, os
src = open(os.path.join(os.path.dirname(__file__), 'qbbars.py')).read()
ns = {'subprocess': subprocess}
exec(re.search(r'def text\(o\):.*?\n(?=def one)', src, re.S).group(0), ns)

def obj(code):
    d = tempfile.mkdtemp(); o = d + '/o.o'
    subprocess.run(['gcc', '-c', '-x', 'c', '-', '-o', o], input=code, text=True, check=True)
    return o

def test_empty_text_is_zero():
    assert ns['text'](obj('int d = 1;')) == 0

def test_text_counts_function_bytes():
    assert ns['text'](obj('int f(int x){return x*3+1;}')) > 0

def test_zero_is_kept_by_the_filter():
    assert 'is not None' in src and 'if text(o) else' not in src and 'or None' not in src
