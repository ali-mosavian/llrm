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

GCC = 'for a; do shift; case "$a" in -std=c23) a=-std=c2x;; esac; set -- "$@" "$a"; done\nexec gcc "$@"\n'

def test_a_refused_module_is_listed_as_failed_not_compared(tmp_path):
    """A module llrm-cc refuses must read as failed, never as a (gcc-built or missing) bar compared against gcc: the QB wrapper once fell back to gcc and a refused module read 'unchanged'."""
    tree, dist, pin = tmp_path / 'tree', tmp_path / 'dist', tmp_path / 'pin'
    (tree / 'src/core').mkdir(parents=True); dist.mkdir(); pin.mkdir()
    for n in ('a', 'b'): (tree / f'src/core/{n}.c').write_text(f'int {n}(int x){{return x*3+1;}}')
    (dist / 'llrm-cc').write_text('#!/bin/sh\n' + 'case "$*" in *a.c*) exit 1;; esac\n' + GCC)
    (pin / 'clang').write_text('#!/bin/sh\n' + GCC)
    for p in (dist / 'llrm-cc', pin / 'clang'): p.chmod(0o755)
    env = dict(os.environ, QB_TREE=str(tree), QB_SCRATCH=str(tmp_path), PIN_BIN=str(pin))
    out = subprocess.run(['python3', os.path.join(os.path.dirname(__file__), 'qbbars.py'), 'm64', 'Os', str(dist)], capture_output=True, text=True, env=env, cwd=tree).stdout
    assert '1/2 files with all three' in out and "failed: ['a.c']" in out, out
