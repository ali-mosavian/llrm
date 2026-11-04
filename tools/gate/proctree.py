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
    """`ps -o pid,ppid,stat,wchan:20,etime,args` of `pid` and everything under it: what a killed step was doing and waiting on."""
    import subprocess

    pids = ",".join(str(one) for one in [pid, *descendants(pid)])
    done = subprocess.run(["ps", "-o", "pid,ppid,stat,wchan:20,etime,args", "-p", pids, "--cols", "300"], capture_output=True, text=True)
    return [f"  {line}" for line in done.stdout.splitlines()] or ["  (ps printed nothing)"]
