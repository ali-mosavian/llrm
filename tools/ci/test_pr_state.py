import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import pr_state  # noqa: E402


def test_the_api_answer_wins_over_the_events_payload(monkeypatch):
    """#1918: a run that started after the ready mark still carried draft: true, and no gate ran for a ready pull request."""
    import io

    monkeypatch.setattr(pr_state.urllib.request, "urlopen", lambda request, timeout=0: io.StringIO('{"draft": false}'))
    assert pr_state.draft("o/r", "1", "true") == "false"


def test_when_the_api_cannot_be_read_the_payload_answers(monkeypatch):
    def down(request, timeout=0):
        raise OSError("offline")

    monkeypatch.setattr(pr_state.urllib.request, "urlopen", down)
    assert pr_state.draft("o/r", "1", "true") == "true" and pr_state.draft("o/r", "1", "") == "false"
