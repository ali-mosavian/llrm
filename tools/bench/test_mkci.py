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
