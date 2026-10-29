"""CPU of a process and everything under it, from /proc: user+sys seconds of the process, its descendants, and the children they reaped."""
import os

TICKS = os.sysconf("SC_CLK_TCK")


def _stats() -> dict[int, tuple[int, int]]:
    """pid -> (parent, ticks) for every process: utime, stime and the reaped children's cutime, cstime."""
    out = {}
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        try:
            with open(f"/proc/{entry}/stat") as handle:
                text = handle.read()
        except OSError:
            continue
        # The command is in parentheses and may hold spaces or parentheses: the fields follow its last one.
        fields = text.rpartition(")")[2].split()
        out[int(entry)] = (int(fields[1]), sum(int(fields[i]) for i in (11, 12, 13, 14)))
    return out


def descendants(pid: int) -> list[int]:
    stats = _stats()
    children: dict[int, list[int]] = {}
    for child, (parent, _) in stats.items():
        children.setdefault(parent, []).append(child)
    found, todo = [], [pid]
    while todo:
        one = todo.pop()
        for child in children.get(one, []):
            found.append(child)
            todo.append(child)
    return found


def tree_cpu(pid: int) -> float:
    """CPU seconds `pid` and its descendants have used, running or reaped."""
    stats = _stats()
    children: dict[int, list[int]] = {}
    for child, (parent, _) in stats.items():
        children.setdefault(parent, []).append(child)
    total, todo = 0, [pid]
    while todo:
        one = todo.pop()
        total += stats.get(one, (0, 0))[1]
        todo.extend(children.get(one, []))
    return total / TICKS


def describe(pid: int) -> list[str]:
    """One line per process under `pid` (itself included): pid, state, CPU seconds, what it waits on, its command. What a killed step was doing."""
    lines = []
    for one in [pid, *descendants(pid)]:
        try:
            with open(f"/proc/{one}/stat") as handle:
                state = handle.read().rpartition(")")[2].split()[0]
            with open(f"/proc/{one}/cmdline", "rb") as handle:
                command = handle.read().replace(b"\0", b" ").decode(errors="replace").strip()
            with open(f"/proc/{one}/wchan") as handle:
                waits = handle.read().strip() or "-"
        except OSError:
            continue
        lines.append(f"  {one} {state} {tree_cpu(one):.1f}s wchan={waits} {command[:200]}")
    return lines
