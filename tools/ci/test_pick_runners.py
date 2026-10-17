import json

from pick_runners import HOSTED, TINYTIM, idle_runners, pick

NOW = 1_000_000.0
GROUPS = ["unit", "python", "tools", "cost", "qcport", "run"]


def test_no_heartbeat_sends_the_jobs_that_do_not_need_counters_to_hosted():
    """A tinytim that never wrote (or lost) its heartbeat must not queue an ordinary job on a runner that is not there."""
    for beat in ("", "garbage", "1 x"):
        got = pick(["unit", "python", "tools", "run"], beat, True, NOW)
        assert got["build"] == HOSTED and set(got["groups"].values()) == {HOSTED} and got["small"] == HOSTED


def test_a_stale_heartbeat_is_a_dead_tinytim():
    assert idle_runners(f"{NOW - 181} 3", NOW) == 0
    assert idle_runners(f"{NOW - 10} 3", NOW) == 3


def test_an_ineligible_event_never_gets_tinytim():
    got = pick(GROUPS, f"{NOW} 3", False, NOW)
    assert got["build"] == HOSTED and set(got["groups"].values()) == {HOSTED}


def test_only_the_jobs_that_need_perf_counters_take_tinytim_and_the_rest_stay_hosted():
    """The same build ran 82-103 s hosted and 243-669 s on tinytim (shared with every session's builds): CI does not depend on its load."""
    got = pick(GROUPS + ["perf", "bench-1"], f"{NOW} 5", True, NOW)
    assert got["build"] == HOSTED
    assert {g for g, v in got["groups"].items() if v == TINYTIM} == {"perf", "cost"}


def test_a_compiler_change_always_runs_its_perf_jobs_on_tinytim_even_with_none_idle():
    """#1892 failed by design ('re-run when tinytim is idle') and a re-run reused the hosted decision. A queued tinytim job is the retry."""
    for beat in ("", f"{NOW} 0"):
        got = pick(["perf", "cost", "unit"], beat, True, NOW)
        assert got["groups"]["perf"] == got["groups"]["cost"] == TINYTIM and got["groups"]["unit"] == HOSTED and got["perf_hosted"] == []


def test_a_change_that_leaves_the_compiler_alone_makes_no_perf_job_and_a_fork_gets_none_on_tinytim():
    assert pick(["perf", "cost", "unit"], f"{NOW} 3", True, NOW, compiler=False)["run"] == ["unit"]
    fork = pick(["perf", "unit"], f"{NOW} 3", False, NOW)
    assert fork["groups"]["perf"] == HOSTED and fork["perf_hosted"] == ["perf"], "the ci job fails a compiler change on this"


def test_the_tiny_serial_jobs_run_hosted_unless_hosted_jobs_wait_and_a_tinytim_runner_is_idle():
    """On tinytim bench-merge and ci shared a queue with 12-minute cost jobs: a 5-second ci job waited 36 minutes."""
    assert pick(["unit"], f"{NOW} 2", True, NOW)["small"] == HOSTED
    assert pick(["unit"], f"{NOW} 2", True, NOW, backlog=20)["small"] == TINYTIM
    assert pick(["perf", "cost"], f"{NOW} 2", True, NOW, backlog=20)["small"] == HOSTED, "the perf jobs took the idle runners"
    assert pick(["unit"], "", True, NOW, backlog=20)["small"] == HOSTED and pick(["unit"], f"{NOW} 2", False, NOW, backlog=20)["small"] == HOSTED


def test_output_is_json_for_fromjson():
    assert json.loads(json.dumps(pick(GROUPS, "", True, NOW)))["groups"]["unit"] == HOSTED


def test_the_summary_names_the_runner_of_every_job():
    got = pick(["unit", "perf"], f"{NOW} 2", True, NOW)
    assert got["summary"] == "tinytim: perf; hosted: build unit"


def test_when_hosted_jobs_are_waiting_the_other_jobs_overflow_to_idle_tinytim():
    """20 hosted jobs running and 80 queued: a job waited 36 minutes for a hosted slot."""
    groups = ["perf", "python", "unit", "bench-1"]
    quiet = pick(groups, f"{NOW} 3", True, NOW, backlog=1)["groups"]
    busy = pick(groups, f"{NOW} 3", True, NOW, backlog=20)["groups"]
    assert quiet["python"] == quiet["unit"] == HOSTED
    assert busy["perf"] == busy["python"] == busy["unit"] == TINYTIM and busy["bench-1"] == HOSTED, "perf first, then by length, while runners are idle"
    assert pick(groups, "", True, NOW, backlog=20)["groups"]["python"] == HOSTED, "no tinytim, no overflow"


def test_the_hosted_backlog_counts_only_queued_jobs_that_wait_for_a_hosted_runner():
    """12 queued runs left hosted idle (a job started in 3 s): runs ahead overflowed jobs to the slower tinytim for nothing."""
    from hosted_backlog import waiting

    jobs = [{"status": "queued", "labels": ["ubuntu-24.04"]}, {"status": "queued", "labels": ["self-hosted", "tinytim"]}, {"status": "in_progress", "labels": ["ubuntu-24.04"]}, {"status": "queued"}]
    assert waiting(jobs) == 2 and waiting([]) == 0
