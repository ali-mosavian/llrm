import shutil
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import mkci  # noqa: E402


@pytest.mark.skipif(shutil.which("gcc") is None, reason="no gcc")
def test_the_host_oracle_reproduces_the_committed_full_output_on_a_corpus_prefix_it_is_given(tmp_path):
    """The .ci.out is gcc's answer, trusted only because the same harness gives grep.out on the whole corpus (mkci.main checks)."""
    out = mkci.host_output("grep", b"Oliver Twist met Scrooge and a ghost.\n" * 3, "bench_grep", tmp_path)
    assert out.split()[0] == "3"


@pytest.mark.skipif(shutil.which("gcc") is None, reason="no gcc")
def test_a_program_that_reads_a_file_by_name_gets_a_host_oracle(tmp_path):
    """stft_fixed (bench_io.h: SIGNAL read, SPECTRA written) had no .ci.out: mkci knew only input_read programs, so its counted cells ran the whole
    64 KB corpus and ow, tc and tcpp stopped at the 3e8-instruction limit: no bar for the Nib cell. gcc on the first 8192 bytes of SIGNAL prints this."""
    import corpus

    signal = corpus.path("signal").read_bytes()
    out = mkci.host_output("stft_fixed", signal[:8192], "bench_stft_fixed", tmp_path, "SIGNAL")
    assert out.split() == ["453552588"]
