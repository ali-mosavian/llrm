"""A module with an empty .text is a measured 0, not a missing bar (qbbars once dropped it: 78 of 79 counted)."""
import re, subprocess, tempfile, os
src = open(os.path.join(os.path.dirname(__file__), 'qbbars.py')).read()
ns = {'subprocess': subprocess}
exec(re.search(r'def sizes\(o,prefixes\):.*?\n(?=def one)', src, re.S).group(0), ns)

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


def test_unset_pin_bin_is_a_message_not_a_home_path(tmp_path):
    """qbbars and pinfn32.sh defaulted to a build directory under the home folder: a path of one machine in the repo. Unset PIN_BIN must say so."""
    env = {k: v for k, v in os.environ.items() if k != 'PIN_BIN'}
    env.update(QB_TREE=str(tmp_path), D=str(tmp_path))
    d = os.path.dirname(__file__)
    q = subprocess.run(['python3', os.path.join(d, 'qbbars.py'), 'm64', 'Os', str(tmp_path)], capture_output=True, text=True, env=env)
    p = subprocess.run(['sh', os.path.join(d, 'pinfn32.sh'), 'x.c'], capture_output=True, text=True, env=env)
    for r in (q, p):
        assert r.returncode != 0 and 'PIN_BIN' in r.stderr and 'llvm-build' not in r.stderr, r.stderr
    for name in ('qbbars.py', 'pinfn32.sh', 'spillcensus.py'):
        assert not re.search('|'.join(['scr' + 'atch', '/ho' + 'me/', '/var/' + 'tmp', r'\$HO' + 'ME']), open(os.path.join(d, name)).read()), name


def test_a_table_in_data_ranks_equal_to_one_in_rodata():
    """Ours put jump tables in .data, gcc and clang in .rodata: `.text` alone ranked a module 462 B worse (exec.o m64) though the object was the same size."""
    asm = lambda sec: f'.text\nf: ret\n.section {sec},"aw",@progbits\nt: .zero 64\n'
    def build(sec):
        d = tempfile.mkdtemp(); o = d + '/o.o'
        subprocess.run(['gcc', '-c', '-x', 'assembler', '-', '-o', o], input=asm(sec), text=True, check=True); return o
    a, b = build('.data'), build('.rodata')
    assert ns['whole'](a) == ns['whole'](b) == ns['text'](a) + 64


def test_extra_modules_outside_core_are_counted(tmp_path):
    """insns.c and os.c are outside src/core: the m32 census counted 79 modules and never the two the coordinator asked for."""
    tree, dist, pin = tmp_path / 'tree', tmp_path / 'dist', tmp_path / 'pin'
    (tree / 'src/core').mkdir(parents=True); (tree / 'src/plat').mkdir(); dist.mkdir(); pin.mkdir()
    (tree / 'src/core/a.c').write_text('int a(int x){return x*3+1;}')
    (tree / 'src/plat/b.c').write_text('int b(int x){return x*5+1;}')
    (dist / 'llrm-cc').write_text('#!/bin/sh\n' + GCC); (pin / 'clang').write_text('#!/bin/sh\n' + GCC)
    for p in (dist / 'llrm-cc', pin / 'clang'): p.chmod(0o755)
    env = dict(os.environ, QB_TREE=str(tree), QB_SCRATCH=str(tmp_path), PIN_BIN=str(pin), QB_EXTRA='src/plat/b.c')
    out = subprocess.run(['python3', os.path.join(os.path.dirname(__file__), 'qbbars.py'), 'm64', 'Os', str(dist)], capture_output=True, text=True, env=env, cwd=tree).stdout
    assert '2/2 files with all three' in out, out
