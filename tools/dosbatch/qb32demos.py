"""NIBBLES and GORILLA on dos32 played with typed keys and compared with BCOM45's screens, in one playthrough each.

BCOM45's screens are cached (~/.cache/llrm/qb32-demo-refs, keyed by the source, the keys and BCOM45's linker), so
only the first run, or one after a change to the script, builds and plays BCOM45's; the dos32 side is built and
played every time, the two demos at once.

NIBBLES's snake runs into the wall (the delay loop is calibrated on TIMER, which once ran backwards on dos32 and sent
the snake across the arena in two seconds), and GORILLA's players throw bananas (the font of the graphics screen once
came back from a protected-mode interrupt as noise). The skyline, the sparkles and where the number lies are random,
so a screen is compared where nothing is: whole where it holds none, else the box of a prompt.
"""

from __future__ import annotations

import hashlib
import os
import shutil
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32  # noqa: E402
import qbplay  # noqa: E402
import qbruntime  # noqa: E402
import test_qbdemos  # noqa: E402

CACHE = Path.home() / ".cache" / "llrm" / "qb32-demo-refs"

# The keys after the introduction's, and the checkpoints that are compared in a box (x0, y0, x1, y1 of a 640 x 350
# screen) rather than whole.
PLAYED = {
    "NIBBLES": ([(["space"], 10, "dies", None)], {}),
    "GORILLA": (
        [
            (["p"], 5, "prompt1", None),
            (["4", "5", "enter"], 1, "prompt2", None),
            (["5", "0", "enter"], 8, "prompt3", None),
        ],
        {"prompt1": (0, 0, 150, 30), "prompt2": (400, 0, 640, 30), "prompt3": (0, 0, 150, 30)},
    ),
}


STABLE_FOR = 3          # successive looks at the screen that must be the same picture
LOOK_EVERY = 0.3        # seconds between looks
SETTLE = 0.5            # seconds after text a step waited for appears


KEY_HEAD, KEY_TAIL = 0x41A, 0x41C       # the BIOS keyboard buffer's ends: equal when no key waits


def keys_taken(session, seconds: float = 5) -> bool:
    """Whether the program has read every key typed: the BIOS keyboard buffer is empty."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        reply = session.send({"cmd": "mem_read", "addr": KEY_HEAD, "len": 4})
        data = str(reply.get("data", ""))
        if len(data) == 8 and data[:4] == data[4:]:
            return True
        time.sleep(0.05)
    return False


def play(exe: Path, work: Path, steps: list) -> dict[str, Path]:
    """test_qbdemos.play, but a step does not sleep its whole time: it goes on once the program has read the keys, and
    then once the text it waited for is there, or else once the screen has been the same picture (the PNG's bytes)
    for a second. The step's seconds are the longest it waits."""
    work.mkdir(exist_ok=True)
    shutil.copy(exe, work / "P.EXE")
    shots: dict[str, Path] = {}
    session = qbplay.Session(work)
    try:
        session.send({"cmd": "dos_cmd", "command": "P.EXE"}, wait=False)
        for keys, longest, name, shown in steps:
            began = time.monotonic()
            session.type(keys)
            if keys:
                keys_taken(session)
            if shown:
                session.wait_for(shown, 60)
                time.sleep(min(SETTLE, longest))
            else:
                deadline, same, last, at = time.monotonic() + longest, 0, None, 0
                while time.monotonic() < deadline:
                    now = work / f"look{at % 2}.png"
                    session.shot(now)
                    same = same + 1 if last is not None and last.read_bytes() == now.read_bytes() else 0
                    if same >= STABLE_FOR:
                        break
                    last, at = now, at + 1
                    time.sleep(LOOK_EVERY)
            if name:
                shots[name] = work / f"{name}.png"
                session.shot(shots[name])
            if os.environ.get("QB32DEMO_TRACE"):
                print(f"{work.parent.parent.name}: {keys} {name} {time.monotonic() - began:.1f}s", file=sys.stderr)
    finally:
        session.close()
    return shots


def script(name: str) -> list:
    return test_qbdemos.SCRIPTS[name] + PLAYED[name][0]


def region_difference(want: Path, got: Path, box: tuple[int, int, int, int]) -> int:
    (width, _, left), (_, _, right) = qbplay.png_pixels(want), qbplay.png_pixels(got)
    scale = width // 640
    x0, y0, x1, y1 = (v * scale for v in box)

    def pixel(data: bytes, x: int, y: int) -> bytes:
        return data[(y * width + x) * 3 : (y * width + x) * 3 + 3]

    return sum(pixel(left, x, y) != pixel(right, x, y) for y in range(y0, y1) for x in range(x0, x1))


def differences(name: str, want: dict[str, Path], got: dict[str, Path]) -> list[str]:
    """The checkpoints of `name` whose screens differ by more than a blinking cursor."""
    boxes, bad = PLAYED[name][1], []
    for checkpoint in want:
        box = boxes.get(checkpoint)
        count = region_difference(want[checkpoint], got[checkpoint], box) if box else qbplay.differing_pixels(want[checkpoint], got[checkpoint])
        if count > test_qbdemos.CURSOR_PIXELS:
            bad.append(f"{checkpoint}: {count} pixels")
    return bad


def _key(name: str, source: Path) -> str:
    linker = dosbatch.QB45 / "LINK.EXE"
    stamp = f"{linker.stat().st_size}:{int(linker.stat().st_mtime)}" if linker.exists() else ""
    return hashlib.sha256(source.read_bytes() + repr(script(name)).encode() + stamp.encode()).hexdigest()[:20]


def reference_shots(name: str, source: Path, work: Path) -> dict[str, Path]:
    """BCOM45's checkpoint screens of `name`: from the cache, or built, played and put there."""
    names = [one[2] for one in script(name) if one[2]]
    cached = CACHE / name / _key(name, source)
    if all((cached / f"{one}.png").exists() for one in names):
        return {one: cached / f"{one}.png" for one in names}
    work.mkdir(parents=True, exist_ok=True)
    (work / "build").mkdir(exist_ok=True)
    exe = qbplay.built_pairs({name: source}, work / "build")[name][0]
    shots = play(exe, work / "play", script(name))
    cached.mkdir(parents=True, exist_ok=True)
    for one, path in shots.items():
        shutil.copy(path, cached / f"{one}.png")
    return {one: cached / f"{one}.png" for one in names}


def candidate_shots(name: str, source: Path, runtime: dict[str, Path], work: Path) -> dict[str, Path]:
    """The checkpoint screens of the dos32 build of `name`."""
    work.mkdir(parents=True, exist_ok=True)
    obj, exe = work / f"{name}.obj", work / f"{name}.exe"
    if reason := qb32.compile_basic(source, obj):
        raise RuntimeError(f"compile {name}: {reason}")
    play_dir = work / "play"
    play_dir.mkdir(exist_ok=True)
    for loader in qb32.link(obj, runtime, exe, work):
        shutil.copy(loader, play_dir / loader.name)
    return play(exe, play_dir, script(name))


def run(sources: dict[str, Path], work: Path) -> dict[str, list[str]]:
    """For each demo, the checkpoints that differ from BCOM45's: [] where it looks the same."""
    runtime = qb32.build(work / "runtime")

    def one(name: str) -> list[str]:
        with ThreadPoolExecutor(2) as pool:
            reference = pool.submit(reference_shots, name, sources[name], work / name / "ref")
            candidate = pool.submit(candidate_shots, name, sources[name], runtime, work / name / "cand")
            return differences(name, reference.result(), candidate.result())

    with ThreadPoolExecutor(len(sources)) as pool:
        futures = {name: pool.submit(one, name) for name in sources}
        return {name: future.result() for name, future in futures.items()}
