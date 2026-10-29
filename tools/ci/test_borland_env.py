"""The gate's private jobs give llrm-cc's x86_16-bcc row its Borland headers and library.

Gate run 2 (reference group): qcport and cv4 failed with 'LLRM_BORLAND_INC is not set' / 'the target needs the environment variable
LLRM_BORLAND_INC': the private checkout's variables did not include them, and nothing made the lower-case mirror the row reads."""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools/gate"))
import gate  # noqa: E402

CI = (ROOT / ".github/workflows/ci.yml").read_text()


def test_the_private_environment_names_the_borland_headers_mirror_and_library(tmp_path):
    env = gate.private_env(str(tmp_path))
    assert env["LLRM_BORLAND_INC"] == str(tmp_path / "bcinc") and env["LLRM_BORLAND_LIB"] == str(tmp_path / "bcpp31/lib")


def test_the_job_makes_the_mirror_from_qcports_headers_before_it_exports_the_environment():
    make = 'sh tools/lowercase-includes.sh "$dir/qctc/inc" "$dir/bcinc"'
    assert make in CI and CI.index(make) < CI.index('python3 tools/gate/gate.py private-env "$dir"')
