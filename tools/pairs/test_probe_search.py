"""A probe that does not build is unknown: never good, never bad, never narrowed through."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import probe_search as p  # noqa: E402

COMMITS = [f"c{n:04d}" for n in range(1679)]


def builder(unbuildable: range, value):
    """A fake probe: `value(index)` for the commits that build, a failure for the rest; counts how often each is asked."""
    asked = []

    def probe(commit):
        index = COMMITS.index(commit)
        asked.append(index)
        if index in unbuildable:
            return "failed", "cargo: error"
        return "ok", {"n": value(index)}

    return probe, asked


def test_a_range_that_does_not_build_is_undetermined_not_good():
    """2026-10-24: every commit before index 646 failed to build, the old search took each for good, and named index 646, the first that built, as the commit where
    scroll/nib went bad. The value was 170,762 at index -1 (main) and 1,794,354 just before 646, so the step was somewhere in the unbuilt range."""
    probe, _ = builder(range(1, 646), lambda i: 700_660 if i else 5)
    found = p.first_bad(COMMITS, probe, good=0, bad=646, is_bad=lambda counts: counts["n"] > 400_000)
    assert found.undetermined and not found.exact, found
    assert found.last_good == 0, "the last good commit is one that was measured"


def test_a_failed_neighbour_is_stood_in_for_by_the_next_commit_that_builds():
    probe, _ = builder(range(100, 102), lambda i: 10 if i < 120 else 99)
    found = p.first_bad(COMMITS, probe, good=0, bad=200, is_bad=lambda counts: counts["n"] > 50)
    assert found.exact and (found.last_good, found.first_bad) == (119, 120)


def test_the_step_is_named_only_between_two_measured_commits():
    probe, _ = builder(range(0, 0), lambda i: 5 if i < 777 else 500)
    found = p.first_bad(COMMITS, probe, good=0, bad=1678, is_bad=lambda counts: counts["n"] > 100)
    assert found.exact and found.first_bad == 777 and found.last_good == 776
