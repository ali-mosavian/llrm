"""A host command stopped by the CPU time it uses, not by the wall clock.

A compile that takes a few seconds on an idle host takes ten times that when thirty gates share it: a wall-clock
limit stopped programs that were fine (a 10 s limit stopped test_loops' compile, 60 s stopped torture's) and
the failure said nothing about the compiler. `ulimit -t` counts the process's own CPU seconds, which a busy host
does not change. The wall clock stays only as a backstop for a command that waits and burns nothing.
"""

from __future__ import annotations

import os
import signal
import subprocess

# CPU seconds a limit allows past the backstop's: a command that sleeps is stopped by this, not by `ulimit`.
BACKSTOP_SECONDS = 3600


# Address space (KB) a command may map: a runaway compile died at 50 GB RSS (datetime.c under the translated JumpThreading) before a timeout stopped it.
MEMORY_KB = int(os.environ.get("LLRM_MEMORY_LIMIT_KB", 8_000_000))
OUT_OF_MEMORY = ("memory allocation of", "Cannot allocate memory", "MemoryError", "out of memory")


class MemoryLimit(subprocess.SubprocessError):
    """The command was stopped by the address-space limit (`ulimit -v`)."""


def run(command: list, seconds: int, memory_kb: int | None = None, **kwargs) -> subprocess.CompletedProcess:
    """`subprocess.run(command, **kwargs)`, stopped after `seconds` of CPU time: raises `subprocess.TimeoutExpired`,
    the exception a wall-clock `timeout=` raised, so a caller that handled that handles this."""
    kwargs.setdefault("timeout", BACKSTOP_SECONDS)
    limit = MEMORY_KB if memory_kb is None else memory_kb
    done = subprocess.run(["sh", "-c", f'ulimit -t {seconds}; ulimit -v {limit}; exec "$@"', "sh", *map(str, command)], **kwargs)
    if done.returncode in (-signal.SIGXCPU, -signal.SIGKILL):
        raise subprocess.TimeoutExpired(command, seconds, output=done.stdout, stderr=done.stderr)
    text = done.stderr if isinstance(done.stderr, str) else (done.stderr or b"").decode("latin-1")
    if done.returncode and any(mark in text for mark in OUT_OF_MEMORY):
        raise MemoryLimit(f"memory limit ({limit // 1024} MB): {' '.join(map(str, command[:3]))}")
    return done
