"""Whether a pull request is a draft now, from the API.

    pr_state.py REPO NUMBER PAYLOAD    prints true or false; PAYLOAD (the event's) when the API cannot be read

The event's payload is the state when the event was sent. A run that starts after the pull request was marked ready can still carry
`draft: true`, and then no gate runs for a ready pull request (#1918).
"""

import os
import sys
import json
import urllib.request


def draft(repo: str, number: str, payload: str, token: str = "") -> str:
    try:
        request = urllib.request.Request(f"https://api.github.com/repos/{repo}/pulls/{number}", headers={"Authorization": f"Bearer {token}"} if token else {})
        return str(json.load(urllib.request.urlopen(request, timeout=20))["draft"]).lower()
    except Exception:  # noqa: BLE001: no answer from the API is the payload's answer
        return payload.lower() if payload.lower() in ("true", "false") else "false"


if __name__ == "__main__":
    print(draft(sys.argv[1], sys.argv[2], sys.argv[3], os.environ.get("GH_TOKEN", "")))
