"""A host command stopped by the CPU time it uses, not by the wall clock.

A compile that takes a few seconds on an idle host takes ten times that when thirty gates share it: a wall-clock
limit stopped programs that were fine (a 10 s limit stopped test_loops' compile, 60 s stopped torture's) and
the failure said nothing about the compiler. `ulimit -t` counts the process's own CPU seconds, which a busy host
does not change. The wall clock stays only as a backstop for a command that waits and burns nothing.
"""

from __future__ import annotations

import signal
import subprocess

# CPU seconds a limit allows past the backstop's: a command that sleeps is stopped by this, not by `ulimit`.
BACKSTOP_SECONDS = 3600


def run(command: list, seconds: int, **kwargs) -> subprocess.CompletedProcess:
    """`subprocess.run(command, **kwargs)`, stopped after `seconds` of CPU time: raises `subprocess.TimeoutExpired`,
    the exception a wall-clock `timeout=` raised, so a caller that handled that handles this."""
    kwargs.setdefault("timeout", BACKSTOP_SECONDS)
    done = subprocess.run(["sh", "-c", f'ulimit -t {seconds}; exec "$@"', "sh", *map(str, command)], **kwargs)
    if done.returncode in (-signal.SIGXCPU, -signal.SIGKILL):
        raise subprocess.TimeoutExpired(command, seconds, output=done.stdout, stderr=done.stderr)
    return done
