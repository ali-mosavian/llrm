"""An arm64 timed-only program reads the cut of its corpus its bench.toml names, as the DOS bench does."""
import os, subprocess, sys, tempfile

import pytest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import count

HERE = os.path.dirname(os.path.abspath(__file__))
BENCH = os.path.join(HERE, '..', '..', 'bench')
CLANG = os.path.join(os.environ.get('LLVM_BIN', os.path.expanduser('~/scratch/llvm-build-aarch64/bin')), 'clang')


@pytest.mark.skipif(not os.path.exists(CLANG), reason='needs the pin clang for aarch64 (LLVM_BIN)')
def test_stft_fixed_prints_its_ci_output_from_the_cut_of_its_signal():
    """rt.c cut only grep's corpus, at a CI_BYTES it hard-coded: stft_fixed (ci_bytes 16384 since #10685) read all of SIGNAL and printed
    30825279 where its .ci.out says 374810179."""
    with tempfile.TemporaryDirectory() as out:
        subprocess.run(['bash', os.path.join(HERE, 'build2.sh'), 'stft_fixed', '2', 'clang'], env={**os.environ, 'OUT': out}, check=True)
        value, _, error = count.run(os.path.join(out, 'stft_fixed.2.clang.elf'))
    want = open(os.path.join(BENCH, 'stft_fixed', 'stft_fixed.ci.out')).read().split()[-1]
    assert not error and str(value).split(':')[0] == want, (value, error)  # the report, then a hash of the files written
