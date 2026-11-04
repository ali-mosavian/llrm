"""Lockstep waits (qbplay.Session, qb32demos): in emulated time, so a slow host takes the same steps."""

from __future__ import annotations

import sys
import time
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import qbplay  # noqa: E402


class Guest:
    """A lockstep session without an emulator: the guest is a function of the emulated clock, and the host may take as long as it likes."""

    run_until = qbplay.Session.run_until

    def __init__(self, text=lambda ms: "", picture=lambda ms: "still", host_delay: float = 0.0):
        self.ms, self.calls, self.text, self.picture, self.host_delay = 0, [], text, picture, host_delay

    def resume(self, ms=None):
        time.sleep(self.host_delay)
        self.ms += ms
        self.calls.append(("resume", ms))
        return {"event": "run_limit"}

    def run_to_key(self, ms):
        self.calls.append(("run_to_key", ms))

    def send(self, command, wait=True):
        self.calls.append(("send", command["cmd"]))

    def screen(self):
        time.sleep(self.host_delay)
        return self.text(self.ms)

    def screen_hash(self):
        time.sleep(self.host_delay)
        return self.picture(self.ms)


@pytest.mark.parametrize("host_delay", [0.0, 0.01])
def test_a_text_step_waits_in_emulated_slices_and_a_slow_host_takes_the_same_steps(host_delay):
    """The waits counted the emulator's CPU seconds and the wall, so how many looks a step took, and when its keys went in, was the host's
    load (GORILLA's prompt3 and #1570's 'Press any key'). The same guest now gives the same calls at any host speed."""
    import qb32demos

    one = Guest(text=lambda ms: "Press any key" if ms >= 3000 else "loading", host_delay=host_delay)
    qb32demos.ready(one, "Press any key", typed=False)
    assert one.calls == [("resume", 100)] * 30 + [("send", "key_wait_arm"), ("run_to_key", qb32demos.LONGEST_MS)]


def test_a_text_step_arms_the_key_read_after_the_text_and_not_before():
    """GORILLA's 'Press any key' is on screen before the program flushes the keyboard and reads it: a key_wait from before the text ended
    the step, the space was flushed, and 'Name of Player 1' never came."""
    import qb32demos

    one = Guest(text=lambda ms: "Press any key" if ms >= 200 else "")
    qb32demos.ready(one, "Press any key", typed=True)
    assert [c[0] for c in one.calls] == ["resume", "resume", "send", "run_to_key"]


def test_a_step_with_keys_and_no_text_ends_where_the_last_key_was_read():
    """Arming again after the last key ran the program on to a later read: a prompt that polls was left behind."""
    import qb32demos

    one = Guest()
    qb32demos.ready(one, None, typed=True)
    assert one.calls == []


def test_a_step_that_never_shows_its_text_fails_after_the_same_emulated_time_at_any_speed():
    import qb32demos

    one = Guest(text=lambda ms: "loading")
    with pytest.raises(TimeoutError, match=rf"never came in {qb32demos.LONGEST_MS} emulated ms"):
        qb32demos.ready(one, "Press any key", typed=False)
    assert one.ms == qb32demos.LONGEST_MS


@pytest.mark.parametrize("host_delay", [0.0, 0.01])
def test_a_checkpoint_settles_after_the_same_emulated_looks_at_any_host_speed(host_delay):
    """GORILLA's prompt3 differed by 56 pixels from BCOM45's whenever the emulator's core was starved: three looks 0.03 s apart on the wall
    saw one half-drawn frame three times. A look now follows LOOK_MS of guest time, whatever the host did in between."""
    import qb32demos

    one = Guest(picture=lambda ms: f"frame{min(ms, 400)}", host_delay=host_delay)
    qb32demos.settled(one, "prompt3", "before")
    looks = [c for c in one.calls if c[0] == "resume"]
    assert one.ms == 400 + qb32demos.LOOK_MS * (qb32demos.LOOKS - 1) and set(looks) == {("resume", qb32demos.LOOK_MS)}


def test_a_checkpoint_that_never_settles_says_how_it_looked():
    """CI's GORILLA failed `the screen never settled for prompt3` on a hosted runner and nothing said whether the picture moved for ever,
    never left the one before the step, or was never looked at."""
    import qb32demos

    one = Guest(picture=lambda ms: f"frame{ms}")
    with pytest.raises(TimeoutError, match=r"never settled for prompt3: \d+ looks in 60000 emulated ms, \d+ distinct pictures, last "):
        qb32demos.settled(one, "prompt3", "before")
    stuck = Guest(picture=lambda ms: "before")
    with pytest.raises(TimeoutError, match="still the one before the step"):
        qb32demos.settled(stuck, "prompt3", "before")


def test_the_keys_go_in_one_at_a_time_each_where_the_program_asks_for_it():
    import qbplay

    sent = []
    one = object.__new__(qbplay.Session)
    one.now_us = 0
    one.send = lambda command, wait=True: sent.append(("send", command["key"], command["stop"]))
    one.run_to_key = lambda ms: sent.append(("run", ms))
    one.keys_in_lockstep(["4", "5", "enter"], 7)
    assert sent == [("send", "4", True), ("run", 7), ("send", "5", True), ("run", 7), ("send", "enter", True), ("run", 7)]


def test_keys_wait_for_a_program_that_asks_not_for_one_that_flushes_the_keyboard():
    """GORILLA's velocity prompt flushes the keyboard before it asks: a key typed at the first empty read was read by the flush and thrown
    away, and the game sat at `Velocity: _`. `waiting` empty reads in a row with the picture the same is a program that asks."""
    import qbplay

    pictures = iter(["flush", "draw", "ask", "ask", "ask"])
    runs, one = [], object.__new__(qbplay.Session)
    one.now_us = 0
    one.send = lambda command, wait=True: runs.append(command["cmd"])
    one.run_to_key = lambda ms: None
    one.screen_hash = lambda: next(pictures)
    one.keys_in_lockstep(["7"], 100, waiting=3)
    assert runs == ["key"] + ["key_wait_arm"] * 4  # the reads after the flush: drawing, then asking three times


def test_an_emulator_that_cannot_park_the_guest_is_refused_not_waited_on():
    import qbplay

    one = object.__new__(qbplay.Session)
    one.send = lambda command, wait=True: {"status": "ok", "on": True}
    one.close = lambda: None
    with pytest.raises(RuntimeError, match="cannot run in lockstep"):
        one.require_lockstep()


def test_a_wait_for_a_key_that_ends_at_the_limit_is_a_timeout():
    import qbplay

    one = object.__new__(qbplay.Session)
    one.resume = lambda ms: {"event": "run_limit"}
    with pytest.raises(TimeoutError, match="read no key in 7 emulated ms"):
        one.run_to_key(7)


def test_a_demo_that_cannot_be_played_fails_its_own_case_and_not_the_others(monkeypatch, tmp_path):
    """One module fixture played both demos and re-raised the first exception: GORILLA's `never settled for prompt3` was recorded for
    NIBBLES as well, and a reader of the report looked for a cause the demos share."""
    import qb32demos
    import qbbuild

    monkeypatch.setattr(qbbuild, "runtimes", lambda work: {})

    def build(source, work, objects):
        if "GORILLA" in str(source):
            raise TimeoutError("the screen never settled for prompt3")
        return {}

    monkeypatch.setattr(qbbuild, "build", build)
    monkeypatch.setattr(qb32demos, "reference_shots", lambda name, source, work, conf=None: {})
    # run() reads NIBBLES's source to seed it, so the demos are files (a host without the private demos has none)
    nibbles, gorilla = tmp_path / "NIBBLES.BAS", tmp_path / "GORILLA.BAS"
    nibbles.write_bytes(b"RANDOMIZE TIMER\r\n")
    gorilla.write_bytes(b"PRINT 1\r\n")
    results = qb32demos.run({"NIBBLES": nibbles, "GORILLA": gorilla}, tmp_path / "work")
    assert qb32demos.of(results, "NIBBLES") == {}
    with pytest.raises(TimeoutError, match="never settled for prompt3"):
        qb32demos.of(results, "GORILLA")
