import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / "selfhosted"))
os.environ["PAT_FILE"] = str(Path(__file__))  # any readable file: the module reads the token at import

import supervisor  # noqa: E402


def test_a_runner_that_dies_at_once_makes_the_slot_wait():
    """A bad podman flag made every start fail in milliseconds; the slot registered a runner per spin (30 offline ones in a minute)."""
    assert supervisor.backoff(0.2) > 0
    assert supervisor.backoff(3600) == 0


def test_only_dead_registrations_of_ours_are_reaped():
    runners = [
        {"id": 1, "name": "tinytim-1-aa", "status": "offline"},
        {"id": 2, "name": "tinytim-2-bb", "status": "offline"},
        {"id": 3, "name": "tinytim-3-cc", "status": "online"},
        {"id": 4, "name": "someone-else", "status": "offline"},
    ]
    assert supervisor.stale(runners, {"tinytim-2-bb"}) == [1]


def test_every_slot_gets_its_own_cores():
    """nproc inside a container reported 32 under --cpus: JOBS=$(nproc) oversubscribed. A cpuset of 8 makes it 8."""
    src = Path(supervisor.__file__).read_text()
    assert "--cpuset-cpus" in src and "--cpus=" not in src


def test_the_workspace_is_the_hosted_one():
    """The target cache from a hosted build holds /home/runner/work paths in llrm-c; on tinytim's /home/runner/_work every wccq spawn failed
    "No such file or directory" (30 python-group test files red)."""
    assert supervisor.WORK == "work"
