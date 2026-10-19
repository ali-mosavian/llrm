"""`stage(name)`: one flushed, timestamped line at the START of a stage, so a batch log carries stage times.

The squash and the smoke printed their log at the end of the run: the 16:16 batch showed every stage at 16:23:48.
"""
from __future__ import annotations

import time

PREFIX = "STAGE"


def stage(name: str) -> None:
    print(f"{time.strftime('%H:%M:%S')} {PREFIX} {name}", flush=True)
