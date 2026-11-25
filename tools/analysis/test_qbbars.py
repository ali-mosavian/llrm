"""A module with an empty .text is a measured 0, not a missing bar (qbbars once dropped it: 78 of 79 counted)."""
import re, subprocess, tempfile, os, shutil
src = open(os.path.join(os.path.dirname(__file__), 'qbbars.py')).read()
ns = {'subprocess': subprocess, 're': re}
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


def stub_make(tmp_path, mods):
    """A `make` that prints the compile lines of `mods` [(source, own flags)], as `make -n -B` does, and builds nothing."""
    (tmp_path / 'bin').mkdir(exist_ok=True)
    body = ''.join(f'echo "cc -std=c23 -O1 -g -Wall {own} -MMD -MP -MF x.d -MT x.o -c {src} -o x.o"\n' for src, own in mods)
    (tmp_path / 'bin/make').write_text('#!/bin/sh\n' + body); (tmp_path / 'bin/make').chmod(0o755)
    return f'{tmp_path / "bin"}:' + os.environ['PATH']

def test_a_refused_module_is_listed_as_failed_not_compared(tmp_path):
    """A module llrm-cc refuses must read as failed, never as a (gcc-built or missing) bar compared against gcc: the QB wrapper once fell back to gcc and a refused module read 'unchanged'."""
    tree, dist, pin = tmp_path / 'tree', tmp_path / 'dist', tmp_path / 'pin'
    (tree / 'src/core').mkdir(parents=True); dist.mkdir(); pin.mkdir()
    for n in ('a', 'b'): (tree / f'src/core/{n}.c').write_text(f'int {n}(int x){{return x*3+1;}}')
    (dist / 'llrm-cc').write_text('#!/bin/sh\n' + 'case "$*" in *a.c*) exit 1;; esac\n' + GCC)
    (pin / 'clang').write_text('#!/bin/sh\n' + GCC)
    for p in (dist / 'llrm-cc', pin / 'clang'): p.chmod(0o755)
    env = dict(os.environ, QB_TREE=str(tree), QB_SCRATCH=str(tmp_path), PIN_BIN=str(pin), PATH=stub_make(tmp_path, [('src/core/a.c', ''), ('src/core/b.c', '')]))
    out = subprocess.run(['python3', os.path.join(os.path.dirname(__file__), 'qbbars.py'), 'm64', 'Os', str(dist)], capture_output=True, text=True, env=env, cwd=tree).stdout
    assert '1/2 files with all three' in out and "failed: ['src/core/a.c']" in out, out


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


def test_the_module_set_and_defines_come_from_the_product_build(tmp_path):
    """w11's and my censuses disagreed on the same 77 modules in all three columns: hand-written module lists and defines (mine had -DEXEC_COUNT, the perf build has none). The set and each file's own -D/-I come from `make -n -B`."""
    tree, dist, pin, log = tmp_path / 'tree', tmp_path / 'dist', tmp_path / 'pin', tmp_path / 'log'
    (tree / 'src/core').mkdir(parents=True); (tree / 'src/plat').mkdir(); dist.mkdir(); pin.mkdir()
    for p in ('src/core/a.c', 'src/plat/b.c', 'src/core/c.c'): (tree / p).write_text('int f(int x){return x*3+1;}')
    real = shutil.which('gcc')
    spy = f'#!/bin/sh\ncase " $* " in *" -c "*) echo "$0 $*" >> {log};; esac\n' + GCC.replace('exec gcc', f'exec {real}')
    stub = stub_make(tmp_path, [('src/core/a.c', '-DFOO=1 -Iinc -isystem sys'), ('src/plat/b.c', '-DBAR')])
    (tmp_path / 'bin/gcc-15').write_text(spy); (dist / 'llrm-cc').write_text(spy); (pin / 'clang').write_text(spy)
    for p in (dist / 'llrm-cc', pin / 'clang', tmp_path / 'bin/gcc-15'): p.chmod(0o755)
    env = dict(os.environ, QB_TREE=str(tree), QB_SCRATCH=str(tmp_path), PIN_BIN=str(pin), PATH=stub)
    out = subprocess.run(['python3', os.path.join(os.path.dirname(__file__), 'qbbars.py'), 'm64', 'Os', str(dist)], capture_output=True, text=True, env=env, cwd=tree).stdout
    assert '2/2 files with all three' in out, out
    lines = log.read_text().splitlines()
    assert len(lines) == 6 and not any('c.c ' in l.split('-c')[1] for l in lines), lines
    for line in lines:
        words = line.split()
        if 'src/core/a.c' in words:
            assert '-DFOO=1' in words and '-Iinc' in words and ('-isystem sys' in line), line
            assert '-DBAR' not in words and '-DEXEC_COUNT' not in words, line
        else:
            assert '-DBAR' in words and '-DFOO=1' not in words, line


def test_every_bar_carries_the_shared_external_flags(tmp_path):
    """Bars were built with the distro's defaults (frame pointer, endbr64, canary, PIE, i686): the bar is the compiler's own code, so gcc and clang get
    tools/external_flags.py's set, `-fPIE` at m64, `-fno-pic -fno-pie` and `-march=i486` at m32."""
    tree, dist, pin, log = tmp_path / 'tree', tmp_path / 'dist', tmp_path / 'pin', tmp_path / 'log'
    (tree / 'src/core').mkdir(parents=True); dist.mkdir(); pin.mkdir()
    (tree / 'src/core/a.c').write_text('int a(int x){return x*3+1;}')
    real = shutil.which('gcc')
    spy = f'#!/bin/sh\ncase " $* " in *" -c "*) echo "$0 $*" >> {log};; esac\n' + GCC.replace('exec gcc', f'exec {real}')
    (tmp_path / 'bin').mkdir(); (tmp_path / 'bin/gcc-15').write_text(spy)
    (dist / 'llrm-cc').write_text(spy); (pin / 'clang').write_text(spy)
    for p in (dist / 'llrm-cc', pin / 'clang', tmp_path / 'bin/gcc-15'): p.chmod(0o755)
    want = {'m64': ['-fomit-frame-pointer', '-fno-stack-protector', '-fcf-protection=none', '-fno-stack-clash-protection', '-fPIE'],
            'm32': ['-fomit-frame-pointer', '-fno-stack-protector', '-fcf-protection=none', '-fno-pic', '-fno-pie', '-march=i486']}
    for arch, flags in want.items():
        log.write_text('')
        env = dict(os.environ, QB_TREE=str(tree), QB_SCRATCH=str(tmp_path), PIN_BIN=str(pin), PATH=stub_make(tmp_path, [('src/core/a.c', '')]))
        subprocess.run(['python3', os.path.join(os.path.dirname(__file__), 'qbbars.py'), arch, 'Os', str(dist)], capture_output=True, text=True, env=env, cwd=tree)
        lines = log.read_text().splitlines()
        assert len(lines) == 3, lines
        for line in lines:
            for flag in flags:
                assert flag in line.split(), f'{arch}: {flag} missing in {line}'
        assert (arch != 'm32') or any('-fexcess-precision=fast' in l.split() for l in lines), 'gcc -m32 cannot compile pfpu.c without it (the module was dropped)'
        assert any('-D__cdecl=' in l.split() for l in lines), 'gcc cannot compile qbrt.c without -D__cdecl= (the column dropped the module)'


def test_code_in_any_executable_section_is_counted():
    """A bar of `.text*` names misses code a compiler puts elsewhere (a named hot section, .init): one side then reads smaller than it is. Every SHF_EXECINSTR section counts, a data section does not."""
    def build(asm):
        d = tempfile.mkdtemp(); o = d + '/o.o'
        subprocess.run(['gcc', '-c', '-x', 'assembler', '-', '-o', o], input=asm, text=True, check=True)
        return o
    plain = build('.text\nf: ret\n')
    split = build('.text\nf: ret\n.section .text.unlikely,"ax",@progbits\ng: .zero 16\n.section .myhot,"ax",@progbits\nh: .zero 32\n.section .data.tbl,"aw",@progbits\nd: .zero 64\n')
    assert ns['text'](split) - ns['text'](plain) == 48, (ns['text'](split), ns['text'](plain))
    assert ns['whole'](split) - ns['whole'](plain) == 48 + 64


def run_arch(tmp_path, arch, gcc_name, with_clang=True, env_extra=None):
    """qbbars on one module with every compiler a logging stub; returns (stdout, stderr, logged compile lines)."""
    tree, dist, pin, log = tmp_path / 'tree', tmp_path / 'dist', tmp_path / 'pin', tmp_path / 'log'
    (tree / 'src/core').mkdir(parents=True, exist_ok=True); dist.mkdir(exist_ok=True); pin.mkdir(exist_ok=True)
    (tree / 'src/core/a.c').write_text('int a(int x){return x*3+1;}'); log.write_text('')
    spy = f'#!/bin/sh\ncase " $* " in *" -c "*) echo "$0 $*" >> {log};; esac\n' + GCC.replace('exec gcc', f'exec {shutil.which("gcc")}')
    stub = stub_make(tmp_path, [('src/core/a.c', '')])
    (tmp_path / 'bin' / gcc_name).write_text(spy)
    (dist / 'llrm-cc').write_text(spy); (pin / 'clang').write_text(spy)
    for p in (dist / 'llrm-cc', pin / 'clang', tmp_path / 'bin' / gcc_name): p.chmod(0o755)
    env = dict(os.environ, QB_TREE=str(tree), QB_SCRATCH=str(tmp_path), PATH=stub, **(env_extra or {}))
    if with_clang: env['PIN_BIN'] = str(pin)
    else: env.pop('PIN_BIN', None)
    r = subprocess.run(['python3', os.path.join(os.path.dirname(__file__), 'qbbars.py'), arch, 'Os', str(dist)], capture_output=True, text=True, env=env, cwd=tree)
    return r.stdout, r.stderr, log.read_text().splitlines()


def test_arm64_bars_are_the_arm64_compilers_with_the_arm64_flags(tmp_path):
    """The arm64 table row: aarch64 gcc, clang told its target, no x86 words (-m64, -fcf-protection) and PAC/BTI off."""
    out, err, lines = run_arch(tmp_path, 'arm64', 'aarch64-linux-gnu-gcc-15', env_extra={'LLRM_ARM64_SYSROOT': '/sys/root'})
    assert len(lines) == 3, (lines, err)
    ours, gcc, clang = lines
    assert 'aarch64-linux-gnu-gcc-15' in gcc and '--target=aarch64-linux-gnu' in clang.split() and '--target=aarch64-linux-gnu' not in gcc.split()
    w = clang.split()
    assert '-nostdinc' in w and w[w.index('-idirafter') + 1] == '/sys/root/usr/aarch64-linux-gnu/include' and '-isystem' not in w[w.index('-idirafter') - 1:w.index('-idirafter')] and '/sys/root/usr/aarch64-linux-gnu/include' not in ours, clang
    for line in lines:
        w = line.split()
        assert '-mbranch-protection=none' in w and '-fPIE' in w, line
        assert not {'-m64', '-m32', '-fcf-protection=none', '-march=i486'} & set(w), line


def test_arm64_needs_its_glibc_root_not_a_guessed_path(tmp_path):
    """QB is hosted: arm64 builds on the arm64-glibc row, whose headers come from $LLRM_ARM64_SYSROOT. Unset must say so (the freestanding row once measured QB against the host's x86 headers)."""
    out, err, lines = run_arch(tmp_path, 'arm64', 'aarch64-linux-gnu-gcc-15', env_extra={'LLRM_ARM64_SYSROOT': ''})
    assert 'LLRM_ARM64_SYSROOT' in err and not lines, err


def test_m68k_is_a_gcc_only_bar_at_the_named_cpu_with_soft_float(tmp_path):
    """m68k: no clang column (no pin clang is needed, PIN_BIN unset is fine); gcc gets -m<cpu> -msoft-float (gcc's 68020 default is the FPU, which ours does not use: a 332 B low bar); ours is told the target."""
    for cpu in ('68000', '68020'):
        out, err, lines = run_arch(tmp_path, 'm68k-' + cpu, 'm68k-linux-gnu-gcc', with_clang=False)
        assert len(lines) == 2, (lines, err)
        ours, gcc = lines
        assert f'-m{cpu}' in gcc.split() and '-msoft-float' in gcc.split() and '-std=gnu2x' in gcc.split(), gcc
        assert '-msoft-float' not in ours.split() and 'm68k-linux-gnu-gcc' not in ours
        assert 'total .text ours' in out and 'clang' not in out, out


def test_an_unknown_arch_lists_the_known_ones(tmp_path):
    out, err, _ = run_arch(tmp_path, 'riscv', 'gcc-15')
    assert 'm64' in err and 'arm64' in err and 'm68k-68020' in err, err


def test_the_report_has_no_clang_column_for_a_gcc_only_arch_and_ranks_by_gcc():
    """report() over fake per-module sizes: m68k has gcc alone, so `best` is gcc and a module larger than it is worse."""
    ns2 = {'__name__': 'qbbars_under_test', '__file__': os.path.join(os.path.dirname(__file__), 'qbbars.py')}
    import sys as _s
    old = _s.argv; _s.argv = ['qbbars.py', 'm68k-68020', 'Os', '.']
    os.environ.setdefault('QB_TREE', '.')
    try: exec(compile(src, 'qbbars.py', 'exec'), ns2)
    finally: _s.argv = old
    res = [('big.c', {'ours': 120, 'ours_whole': 130, 'gcc': 100, 'gcc_whole': 105}), ('small.c', {'ours': 50, 'ours_whole': 50, 'gcc': 60, 'gcc_whole': 60})]
    lines = ns2['report'](res)
    assert lines[0].startswith('m68k-68020 -Os: 2/2 files with all two')
    assert 'total whole ours 180 gcc 165 sum(best) 165' in lines[2], lines
    assert lines[3].startswith('  +25 whole (  +20 text) big.c ours 120/130 gcc 100/105') and 'clang' not in ' '.join(lines)
    assert lines[-1] == 'worse 1 of 2 sum worse 25', lines
