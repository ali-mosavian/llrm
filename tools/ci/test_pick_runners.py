import json

from pick_runners import HOSTED, TINYTIM, idle_runners, pick

NOW = 1_000_000.0
GROUPS = ["unit", "python", "tools", "cost", "reference", "run"]


def test_no_heartbeat_sends_every_job_to_hosted():
    """A tinytim that never wrote (or lost) its heartbeat must not queue a job on a runner that is not there."""
    for beat in ("", "garbage", "1 x"):
        got = pick(GROUPS, beat, True, NOW)
        assert got["build"] == HOSTED and set(got["groups"].values()) == {HOSTED}


def test_a_stale_heartbeat_is_a_dead_tinytim():
    assert idle_runners(f"{NOW - 181} 3", NOW) == 0
    assert idle_runners(f"{NOW - 10} 3", NOW) == 3


def test_an_ineligible_event_never_gets_tinytim():
    got = pick(GROUPS, f"{NOW} 3", False, NOW)
    assert got["build"] == HOSTED and set(got["groups"].values()) == {HOSTED}


def test_heavy_groups_take_the_idle_runners_and_the_rest_stay_hosted():
    got = pick(GROUPS, f"{NOW} 2", True, NOW)
    assert got["build"] == TINYTIM
    assert sum(v == TINYTIM for v in got["groups"].values()) == 2
    assert got["groups"]["unit"] == got["groups"]["run"] == HOSTED


def test_output_is_json_for_fromjson():
    assert json.loads(json.dumps(pick(GROUPS, "", True, NOW)))["groups"]["unit"] == HOSTED


def test_the_summary_names_the_runner_of_every_job():
    got = pick(["unit", "python"], f"{NOW} 2", True, NOW)
    assert got["summary"] == "tinytim: build python; hosted: unit"
