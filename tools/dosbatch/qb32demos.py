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
    "NIBBLES": ([(["space"], 10, "dies", "Sammy Dies")], {}),
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


LONGEST = 15            # the most a step waits, whatever it is waiting for
LOOKS = 3               # successive looks at a screen that must show the same picture
LOOK_EVERY = 0.03       # host seconds between looks: the guest runs between them, so looks back to back would all see one frame


def ready(session, since: int, shown: str | None) -> None:
    """Returns once the program has taken the keys and has put up what the step waits for: the text it names (a text
    screen), else the fork's `key_wait` event, which arrives when the program reads the keyboard and finds nothing there."""
    if shown:
        deadline = time.monotonic() + LONGEST
        while shown not in session.screen():
            if time.monotonic() > deadline:
                raise TimeoutError(f"{shown!r} never appeared")
        since = len(session.events)
        session.send({"cmd": "key_wait_arm"})
    session.wait_event("key_wait", since, LONGEST)


def settled(session, work: Path, name: str | None, before: bytes | None) -> None:
    """For a checkpoint, until its screen is the same picture LOOKS times running: by the hash of the adapter's memory
    on a graphics screen (no decoding), by the text on a text screen; where a box of the screen is compared, until that
    part has changed from `before`."""
    if not name:
        return
    deadline, same, last = time.monotonic() + LONGEST, 0, None
    while time.monotonic() < deadline:
        look = session.screen_hash()
        same = same + 1 if look == last else 1
        if same >= LOOKS and (before is None or look != before):
            return
        last = look
        time.sleep(LOOK_EVERY)
    raise TimeoutError(f"the screen never settled for {name}")


def play(exe: Path, work: Path, steps: list, demo: str | None = None) -> dict[str, Path]:
    """test_qbdemos.play on virtual time and without its sleeps: each step types its keys (which arms the fork's
    `key_wait` event), waits for the event or for the text it names, and a checkpoint then waits for the screen to settle
    and is taken; nothing waits for a time, and none waits longer than LONGEST."""
    work.mkdir(exist_ok=True)
    shutil.copy(exe, work / "P.EXE")
    shots: dict[str, Path] = {}
    session = qbplay.Session(work, qb32.DEMO_CONF)
    try:
        session.send({"cmd": "dos_cmd", "command": "P.EXE"}, wait=False)
        for keys, _seconds, name, shown in steps:
            began, since = time.monotonic(), len(session.events)
            before = session.screen_hash() if name else None
            if keys:
                session.type(keys, pause=0)
            else:
                session.send({"cmd": "key_wait_arm"})
            ready(session, since, shown)
            settled(session, work, name, before)
            if name:
                shots[name] = work / f"{name}.png"
                session.shot(shots[name], settle=0)
            if os.environ.get("QB32DEMO_TRACE"):
                print(f"{work.parent.parent.name}: {keys} {name} {time.monotonic() - began:.2f}s", file=sys.stderr)
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
    return hashlib.sha256(source.read_bytes() + repr(script(name)).encode() + stamp.encode() + qb32.DEMO_CONF.encode()).hexdigest()[:20]


def reference_shots(name: str, source: Path, work: Path) -> dict[str, Path]:
    """BCOM45's checkpoint screens of `name`: from the cache, or built, played and put there."""
    names = [one[2] for one in script(name) if one[2]]
    cached = CACHE / name / _key(name, source)
    if all((cached / f"{one}.png").exists() for one in names):
        return {one: cached / f"{one}.png" for one in names}
    work.mkdir(parents=True, exist_ok=True)
    (work / "build").mkdir(exist_ok=True)
    exe = qbplay.built_pairs({name: source}, work / "build")[name][0]
    shots = play(exe, work / "play", script(name), name)
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
    return play(exe, play_dir, script(name), name)


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
