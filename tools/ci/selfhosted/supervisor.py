"""One ephemeral runner slot on tinytim, and the heartbeat the workflow reads.

    supervisor.py slot N      keep slot N's runner alive: a JIT config from the API (single use, held here, never on disk), then
                              `podman run --rm` of the runner image with it on stdin as the env file. One job, then the container is gone
                              and the slot registers a fresh runner. CPUs and memory are the slot unit's (AllowedCPUs, MemoryMax):
                              rootless podman puts the container in the unit's own cgroup and ignores --cpuset-cpus there.
    supervisor.py heartbeat   publish "<epoch> <idle runners>" as TINYTIM_HEARTBEAT each minute; remove dead registrations.
    supervisor.py cpusets     print each slot's AllowedCPUs (install.sh writes the units from it).

The PAT (/etc/ghrunner/pat) never enters a container. A dead heartbeat stops the beat; the workflow then uses hosted.
"""

import json
import os
import subprocess
import sys
import time
import urllib.request

REPO = os.environ.get("GH_REPO", "ali-mosavian/llrm")
MEMORY = os.environ.get("SLOT_MEMORY", "16g")
IMAGE = os.environ.get("RUNNER_IMAGE", "localhost/llrm-runner:latest")
# Each runner: the two SMT threads (c, c+16) of two physical cores, inside one CCD (L3: cores 0-7 and 8-15). Cores 0, 1, 8, 9 stay with
# interactive work. tinytim clocks about twice a hosted VM, so 4 threads here match its 8.
CPUSETS = ["2-3,18-19", "4-5,20-21", "6-7,22-23", "10-11,26-27", "12-13,28-29", "14-15,30-31"]
SLOTS = len(CPUSETS)
# The hosted runners' workspace is /home/runner/work: the target cache holds paths baked into binaries (LLRM_WCCQ_DIR) and is shared.
WORK = "work"
LABELS = ["self-hosted", "tinytim", "linux", "x64"]


def api(method: str, path: str, body: dict | None = None):
    pat = open(os.environ.get("PAT_FILE", "/etc/ghrunner/pat")).read().strip()
    req = urllib.request.Request(
        f"https://api.github.com/repos/{REPO}/{path}",
        data=json.dumps(body).encode() if body is not None else None,
        method=method,
        headers={"Authorization": f"Bearer {pat}", "Accept": "application/vnd.github+json"},
    )
    with urllib.request.urlopen(req, timeout=30) as r:
        data = r.read()
        return json.loads(data) if data else None


def backoff(elapsed: float) -> float:
    """A runner that exits within seconds failed to start: without a pause the slot registered a runner per spin."""
    return 30.0 if elapsed < 20 else 0.0


def container_flags() -> list[str]:
    """What every container runs with, the preflight's and each job's: one place, so no slot's environment differs from another's."""
    return ["--cgroup-manager=cgroupfs", "run", "--rm", f"--memory={MEMORY}", "--security-opt=seccomp=/opt/ghrunner/seccomp.json", "--pids-limit=8192"]


COUNTER_PROBE = ["perf", "stat", "-x,", "-e", "instructions:u", "true"]


def counters_missing(output: str) -> str | None:
    """None when `perf stat -x, -e instructions:u true` counted instructions, else why not (`<not supported>`, `<not counted>`, an error, or no line)."""
    for line in output.splitlines():
        fields = line.split(",")
        if len(fields) > 2 and fields[2].startswith("instructions"):
            return None if fields[0].strip().isdigit() and int(fields[0]) > 0 else f"instructions:u read {fields[0].strip()!r}"
    return output.strip().splitlines()[-1] if output.strip() else "perf printed nothing"


def preflight(n: int) -> bool:
    """Whether a container of this slot has the instruction counters the cost and perf jobs measure with. A slot without them would take those jobs and
    fail them (or measure nothing): it prints the reason and stays out of the pool instead."""
    done = subprocess.run(["podman", *container_flags(), IMAGE, *COUNTER_PROBE], capture_output=True, text=True, check=False)
    why = counters_missing(done.stdout + done.stderr)
    if why:
        print(f"slot {n}: PREFLIGHT FAILED, no instruction counters in the runner container: {why}", flush=True)
        return False
    print(f"slot {n}: preflight ok, instructions:u counted", flush=True)
    return True


def stale(runners: list[dict], earlier: set[str]) -> list[int]:
    """Ids of tinytim registrations offline now and at the last beat: a runner just started is offline for a moment, a dead one stays so."""
    return [r["id"] for r in runners if r["name"].startswith("tinytim-") and r["status"] == "offline" and r["name"] in earlier]


def heartbeat() -> None:
    offline: set[str] = set()
    while True:
        try:
            runners = api("GET", "actions/runners?per_page=100")["runners"]
            for rid in stale(runners, offline):
                api("DELETE", f"actions/runners/{rid}")
            offline = {r["name"] for r in runners if r["name"].startswith("tinytim-") and r["status"] == "offline"}
            idle = sum(r["name"].startswith("tinytim-") and r["status"] == "online" and not r["busy"] for r in runners)
            value = f"{int(time.time())} {idle}"
            try:
                api("PATCH", "actions/variables/TINYTIM_HEARTBEAT", {"name": "TINYTIM_HEARTBEAT", "value": value})
            except urllib.error.HTTPError as e:
                if e.code != 404:
                    raise
                api("POST", "actions/variables", {"name": "TINYTIM_HEARTBEAT", "value": value})
        except Exception as e:  # a failed beat only ages the last one
            print("heartbeat:", e, flush=True)
        time.sleep(60)


def slot(n: int) -> None:
    while True:
        if not preflight(n):
            time.sleep(300)
            continue
        name = f"tinytim-{n}-{os.urandom(3).hex()}"
        started = time.time()
        try:
            jit = api("POST", "actions/runners/generate-jitconfig", {"name": name, "runner_group_id": 1, "labels": LABELS, "work_folder": WORK})
            subprocess.run(
                ["podman", *container_flags(), "--name", name, "--env-file=/dev/stdin", IMAGE],
                input=f"ACTIONS_RUNNER_INPUT_JITCONFIG={jit['encoded_jit_config']}\n", text=True, check=False,
            )
        except Exception as e:
            print(f"slot {n}:", e, flush=True)
        time.sleep(backoff(time.time() - started))


if __name__ == "__main__":
    mode = sys.argv[1]
    if mode == "slot":
        slot(int(sys.argv[2]))
    elif mode == "heartbeat":
        heartbeat()
    else:
        print("\n".join(CPUSETS))
