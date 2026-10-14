"""Session.wait_event: a harness step ends the moment the fork's event arrives, and never waits past its timeout."""

from __future__ import annotations

import sys
import threading
import time
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import qbplay  # noqa: E402


def session_without_a_process() -> qbplay.Session:
    one = object.__new__(qbplay.Session)
    one.events, one.arrived = [], threading.Condition()
    return one


def arrive(one: qbplay.Session, event: dict) -> None:
    with one.arrived:
        one.events.append(event)
        one.arrived.notify_all()


def test_an_event_that_arrives_while_waiting_ends_the_wait_at_once():
    """The key-read check polled the keyboard buffer every 20 ms; the event is waited on."""
    one = session_without_a_process()
    threading.Timer(0.05, arrive, (one, {"event": "key_wait", "function": 0x16})).start()
    began = time.monotonic()
    assert one.wait_event("key_wait", 0, 5)["function"] == 0x16
    assert time.monotonic() - began < 1


def test_an_event_from_before_the_step_does_not_count():
    """A key_wait left over from an earlier step must not end the next one."""
    one = session_without_a_process()
    arrive(one, {"event": "key_wait"})
    with pytest.raises(TimeoutError):
        one.wait_event("key_wait", len(one.events), 0.1)
    arrive(one, {"event": "other"})
    with pytest.raises(TimeoutError):
        one.wait_event("key_wait", 1, 0.1)


def test_a_text_step_waits_for_a_key_read_after_the_text_not_before(monkeypatch):
    """GORILLA's 'Press any key' is on screen before the program flushes the keyboard and reads it: a key_wait from
    before the text ended the step, the space was flushed, and 'Name of Player 1' never came (rates below 2000)."""
    import qb32demos

    one = session_without_a_process()
    one.screen = lambda: "Press any key to continue"
    one.send = lambda command, wait=True: None
    arrive(one, {"event": "key_wait"})
    monkeypatch.setattr(qb32demos, "LONGEST", 0.2)
    with pytest.raises(TimeoutError):
        qb32demos.ready(one, 0, "Press any key")
    one.send = lambda command, wait=True: threading.Timer(0.05, arrive, (one, {"event": "key_wait"})).start()
    qb32demos.ready(one, 0, "Press any key")


def test_a_text_step_gives_the_emulator_cpu_seconds_not_wall_seconds(monkeypatch):
    """At host load 150 'Press any key' needed more than 15 wall seconds of a guest that had used 3 of CPU, and NIBBLES and GORILLA failed
    (#1570). An emulator that uses no CPU is not late however long the wall runs."""
    import subprocess
    import time

    import qb32demos
    import qbplay

    one = session_without_a_process()
    one.limit = lambda seconds: qbplay.Session.limit(one, seconds)
    one.process = subprocess.Popen(["sleep", "30"])
    shown = []
    one.screen = lambda: "Press any key" if shown else "loading"
    one.send = lambda command, wait=True: arrive(one, {"event": "key_wait"})
    monkeypatch.setattr(qb32demos, "LONGEST", 0.3)
    outcome = []

    def step():
        try:
            qb32demos.ready(one, 0, "Press any key")
            outcome.append("done")
        except TimeoutError as error:
            outcome.append(str(error))

    thread = threading.Thread(target=step)
    try:
        thread.start()
        time.sleep(1.5)  # five times LONGEST on the wall; the emulator used no CPU
        assert outcome == [], outcome
        shown.append(1)
        thread.join(10)
        assert outcome == ["done"]
    finally:
        one.process.kill()
