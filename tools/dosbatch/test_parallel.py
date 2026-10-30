"""run_parallel: how many launches share the jobs, and that the results of the shares come back together."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import dosbatch  # noqa: E402


def test_few_jobs_share_one_launch_and_many_are_capped(monkeypatch):
    monkeypatch.delenv("LLRM_DOS_WORKERS", raising=False)
    assert dosbatch.workers_for(3) == 1
    assert dosbatch.workers_for(40) == 4
    monkeypatch.setenv("LLRM_DOS_WORKERS", "99")
    assert dosbatch.workers_for(400) == 6


def test_the_shares_run_in_launches_of_their_own_and_their_results_are_joined(tmp_path, monkeypatch):
    seen = {}

    def fake_run(jobs, work, **options):
        seen[work.name] = [job.stem for job in jobs]
        return {job.stem: dosbatch.Result("ok") for job in jobs}

    monkeypatch.setattr(dosbatch, "run", fake_run)
    monkeypatch.setenv("LLRM_DOS_WORKERS", "2")
    jobs = [dosbatch.Job(f"C{at:03d}", "obj", tmp_path / "x.obj") for at in range(8)]
    done = dosbatch.run_parallel(jobs, tmp_path / "w")
    assert sorted(done) == sorted(job.stem for job in jobs)
    assert seen == {"launch0": ["C000", "C002", "C004", "C006"], "launch1": ["C001", "C003", "C005", "C007"]}
