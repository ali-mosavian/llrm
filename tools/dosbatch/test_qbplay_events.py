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
