"""Keeps SLOTS ephemeral runners alive on tinytim and publishes the heartbeat the workflow reads.

Each runner: a JIT config from the API (single use, held here, never on disk), then `podman run --rm` of the runner image with
that config on stdin as the env file. One job, then the container is gone and the slot registers a fresh runner.
The PAT (/etc/ghrunner/pat) never enters a container. A dead supervisor stops the heartbeat; the workflow then uses hosted.
"""

import json
import os
import subprocess
import threading
import time
import urllib.request

REPO = os.environ.get("GH_REPO", "ali-mosavian/llrm")
SLOTS = int(os.environ.get("SLOTS", "3"))
CPUS = int(os.environ.get("SLOT_CPUS", "8"))
FIRST_CPU = int(os.environ.get("FIRST_CPU", "8"))  # cores 0-7 stay with interactive work
MEMORY = os.environ.get("SLOT_MEMORY", "64g")
IMAGE = os.environ.get("RUNNER_IMAGE", "localhost/llrm-runner:latest")
PAT = open(os.environ.get("PAT_FILE", "/etc/ghrunner/pat")).read().strip()
# The hosted runners' workspace is /home/runner/work: the target cache holds paths baked into binaries (LLRM_WCCQ_DIR) and is shared.
WORK = "work"
LABELS = ["self-hosted", "tinytim", "linux", "x64"]


def api(method: str, path: str, body: dict | None = None):
    req = urllib.request.Request(
        f"https://api.github.com/repos/{REPO}/{path}",
        data=json.dumps(body).encode() if body is not None else None,
        method=method,
        headers={"Authorization": f"Bearer {PAT}", "Accept": "application/vnd.github+json"},
    )
    with urllib.request.urlopen(req, timeout=30) as r:
        data = r.read()
        return json.loads(data) if data else None


LIVE: set[str] = set()


def backoff(elapsed: float) -> float:
    """A runner that exits within seconds failed to start: without a pause the slot registered a runner per spin."""
    return 30.0 if elapsed < 20 else 0.0


def stale(runners: list[dict], live: set[str]) -> list[int]:
    """Ids of tinytim registrations that are offline and belong to no container of this process."""
    return [r["id"] for r in runners if r["name"].startswith("tinytim-") and r["status"] == "offline" and r["name"] not in live]


def idle_count() -> int:
    runners = api("GET", "actions/runners?per_page=100")["runners"]
    for rid in stale(runners, LIVE):
        api("DELETE", f"actions/runners/{rid}")
    return sum(r["name"].startswith("tinytim-") and r["status"] == "online" and not r["busy"] for r in runners)


def heartbeat() -> None:
    while True:
        try:
            value = f"{int(time.time())} {idle_count()}"
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
        name = f"tinytim-{n}-{os.urandom(3).hex()}"
        try:
            jit = api("POST", "actions/runners/generate-jitconfig", {"name": name, "runner_group_id": 1, "labels": LABELS, "work_folder": WORK})
            LIVE.add(name)
            started = time.time()
            subprocess.run(
                ["podman", "--cgroup-manager=cgroupfs", "run", "--rm", "--name", name, f"--cpuset-cpus={FIRST_CPU + (n - 1) * CPUS}-{FIRST_CPU + n * CPUS - 1}", f"--memory={MEMORY}", "--security-opt=seccomp=/opt/ghrunner/seccomp.json", "--pids-limit=8192",
                 "--env-file=/dev/stdin", IMAGE],
                input=f"ACTIONS_RUNNER_INPUT_JITCONFIG={jit['encoded_jit_config']}\n", text=True, check=False,
            )
            LIVE.discard(name)
            time.sleep(backoff(time.time() - started))
        except Exception as e:
            print(f"slot {n}:", e, flush=True)
            time.sleep(30)


if __name__ == "__main__":
    threading.Thread(target=heartbeat, daemon=True).start()
    ts = [threading.Thread(target=slot, args=(n,)) for n in range(1, SLOTS + 1)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
