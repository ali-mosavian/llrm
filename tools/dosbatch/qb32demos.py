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


KEY_HEAD = 0x41A        # the BIOS keyboard buffer's head and tail words: equal when no key waits
POLL = 0.02             # seconds between looks at what the program is doing
LOOKS = 3               # successive looks at a graphics screen that must show the same picture
LONGEST = 15            # the most a step waits, whatever it is waiting for


def keys_taken(session) -> None:
    """Returns once the program has read every key typed: the BIOS keyboard buffer is empty."""
    deadline = time.monotonic() + LONGEST
    while time.monotonic() < deadline:
        data = str(session.send({"cmd": "mem_read", "addr": KEY_HEAD, "len": 4}).get("data", ""))
        if len(data) == 8 and data[:4] == data[4:]:
            return
        time.sleep(POLL)
    raise TimeoutError("the program never read the keys")


def raw_picture(path: Path, rows: tuple[int, int] | None = None) -> bytes:
    """A PNG's scanlines, inflated and not unfiltered (two screens that differ by a cursor differ in a few bytes of it);
    only the scanlines `rows` (first, last), where given."""
    import struct
    import zlib

    data, at, idat, head = path.read_bytes(), 8, [], b""
    while at < len(data):
        size, kind = struct.unpack(">I4s", data[at : at + 8])
        if kind == b"IHDR":
            head = data[at + 8 : at + 8 + size]
        elif kind == b"IDAT":
            idat.append(data[at + 8 : at + 8 + size])
        at += 12 + size
    raw = zlib.decompress(b"".join(idat))
    if rows is None:
        return raw
    width, _height, _depth, color = struct.unpack(">IIBB", head[:10])
    stride = 1 + width * (3 if color == 2 else 4)
    scale = width // 640
    return raw[rows[0] * scale * stride : (rows[1] * scale) * stride]


def same_picture(a: bytes, b: bytes) -> bool:
    """Whether two screens differ by no more than a blinking cursor."""
    if len(a) != len(b):
        return False
    different = (int.from_bytes(a, "little") ^ int.from_bytes(b, "little")).to_bytes(len(a), "little")
    return len(different.replace(b"\0", b"")) <= 3 * test_qbdemos.CURSOR_PIXELS


def settled(session, work: Path, shown: str | None, watch: str | None, name: str | None, before: bytes | None) -> None:
    """Returns once the screen shows what the step wants.  A step with text to wait for waits for it.  A step that
    names a checkpoint then waits for its screen to be the same picture LOOKS times running, and, where the checkpoint is
    a box of the screen (its first and last rows), for that part to have changed from `before` and then stopped."""
    box = PLAYED.get(watch, ({}, {}))[1].get(name) if watch else None
    rows = (box[1], box[3]) if box else None
    deadline, same, last = time.monotonic() + LONGEST, 0, None
    while time.monotonic() < deadline:
        if shown and shown not in session.screen():
            time.sleep(POLL)
            continue
        if not name:
            return
        now = work / f"look{same % 2}.png"
        session.shot(now, settle=0)
        picture = raw_picture(now, rows)
        if rows and before is not None and same_picture(picture, before):
            time.sleep(POLL)
            continue
        same = same + 1 if last is not None and same_picture(picture, last) else 1
        if same >= LOOKS:
            return
        last = picture
        time.sleep(POLL)
    raise TimeoutError(f"the screen never settled{f' on {shown!r}' if shown else ''}")


def play(exe: Path, work: Path, steps: list, demo: str | None = None) -> dict[str, Path]:
    """test_qbdemos.play without its fixed waits: each step types its keys, waits until the program has read them, and
    then until the screen shows what the step wants; none waits longer than LONGEST."""
    work.mkdir(exist_ok=True)
    shutil.copy(exe, work / "P.EXE")
    shots: dict[str, Path] = {}
    session = qbplay.Session(work)
    try:
        session.send({"cmd": "dos_cmd", "command": "P.EXE"}, wait=False)
        for keys, _seconds, name, shown in steps:
            began = time.monotonic()
            box = PLAYED.get(demo, ({}, {}))[1].get(name) if demo else None
            before = None
            if box:
                session.shot(work / "before.png", settle=0)
                before = raw_picture(work / "before.png", (box[1], box[3]))
            session.type(keys, pause=0)
            if keys:
                keys_taken(session)
            settled(session, work, shown, demo, name, before)
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
