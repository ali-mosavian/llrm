"""NIBBLES and GORILLA on dos32 played with typed keys and compared with BCOM45's screens, in one playthrough each.

BCOM45's screens are cached (~/.cache/llrm/qb32-demo-refs, keyed by the source, the keys and BCOM45's linker), so
only the first run, or one after a change to the script, builds and plays BCOM45's; the dos32 side is built and
played every time, the two demos at once.

NIBBLES's snake runs into the wall (the delay loop is calibrated on TIMER, which once ran backwards on dos32 and sent
the snake across the arena in two seconds), and GORILLA's players throw bananas (the font of the graphics screen once
came back from a protected-mode interrupt as noise). The skyline, the sparkles and where the number lies are random,
so a screen is compared where nothing is: whole where it holds none, else the box of a prompt.

The game is played in lockstep with the emulator (qbplay.Session.resume): the guest runs only between commands, so a key, a look and a
screenshot happen at an emulated instant, and the picture sequence, random parts included, is the same at any host speed or load.
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
import qbbuild  # noqa: E402

CURSOR_PIXELS = 32   # the cursor blinks: a few pixels of difference at most

# (keys to type, seconds (unused: nothing waits for a time), checkpoint name or None, text the screen must show first or None) for
# each demo.
SCRIPTS = {
    "NIBBLES": [
        ([], 7, None, "Press any key"),
        (["space"], 3, "players", "How many players"),
        (["1", "enter"], 2, None, "Skill level"),
        (["1", "enter"], 2, None, "Increase game speed"),
        (["n", "enter"], 2, "monochrome", "Monochrome or color"),
        (["c", "enter"], 1, "level", "Push Space"),
    ],
    "GORILLA": [
        ([], 7, None, "Press any key"),
        (["space"], 3, "names", "Name of Player 1"),
        (["A", "l", "enter"], 1.5, "first", "Name of Player 2"),
        (["enter"], 1.5, "second", "Play to how many"),
        (["1", "enter"], 2, "points", None),
        (["enter"], 4, "ready", None),
    ],
}
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


LONGEST_MS = 60_000     # the most emulated time a step waits, whatever it is waiting for
LOOKS = 3               # successive looks at a screen that must show the same picture
TIMED_GAP_MS = 150      # emulated ms between two keys typed without waiting for the program's read
LOOK_MS = 50            # emulated ms between two looks


def ready(session, shown: str | None, typed: bool) -> None:
    """Returns once the program has taken the keys and has put up what the step waits for: the text it names (a text screen) and then its
    next keyboard read that finds nothing, else (keys typed) the read that took the last of them. The guest runs in emulated slices, so the number of slices it takes is a property of
    the program and not of the host."""
    if shown:
        session.run_until(lambda: shown in session.screen(), LONGEST_MS, repr(shown))
    if shown or not typed:
        session.send({"cmd": "key_wait_arm", "stop": True})
        session.run_to_key(LONGEST_MS)


def settled(session, name: str | None, before: str | None, looks: int = LOOKS, look_ms: int = LOOK_MS) -> None:
    """For a checkpoint, until its screen is the same picture LOOKS times running, LOOK_MS of emulated time apart: by the hash of the
    adapter's memory (`looks` of them, `look_ms` of emulated time apart); where a box of the screen is compared, until that part has changed from `before`."""
    if not name:
        return
    same, last, seen = 0, None, []
    for _ in range(LONGEST_MS // look_ms):
        look = session.screen_hash()
        same = same + 1 if look == last else 1
        if same >= looks and (before is None or look != before):
            return
        last = look
        seen.append(look)
        session.resume(look_ms)
    # What a runner that never settled looked like: a picture that moves for ever, or one that never left `before`.
    raise TimeoutError(
        f"the screen never settled for {name}: {len(seen)} looks in {LONGEST_MS} emulated ms, "
        f"{len(set(seen))} distinct pictures, {'still the one before the step' if set(seen) <= {before} else 'last ' + ' '.join(map(str, seen[-5:]))}"
    )


def play(exe: Path, work: Path, steps: list, demo: str | None = None, conf: str = qb32.DEMO_CONF, timed: bool = False, ended: dict | None = None) -> dict[str, Path]:
    """The steps played in lockstep with the emulator: the guest runs only between commands, for as long as the program takes to
    ask for a key or for a stated number of emulated milliseconds, so each key, look and screenshot happens at an emulated instant
    that does not depend on the host. Nothing waits for a time, and none waits longer than LONGEST_MS of emulated time. `conf` and `timed`
    (each key typed at a fixed emulated gap instead of at the program's read) are for nibprobe's matrix; `ended` gets the emulated
    microseconds and cycles per ms at the end."""
    work.mkdir(exist_ok=True)
    shutil.copy(exe, work / "P.EXE")
    shots: dict[str, Path] = {}
    session = qbplay.Session(work, conf)
    trace = os.environ.get("QB32DEMO_TRACE")
    try:
        session.launch(LONGEST_MS)
        for keys, _seconds, name, shown in steps:
            before = session.screen_hash() if name else None
            session.keys_timed(keys, TIMED_GAP_MS) if timed else session.keys_in_lockstep(keys, LONGEST_MS)
            ready(session, shown, bool(keys))
            settled(session, name, before)
            if name:
                shots[name] = work / f"{name}.png"
                session.snap(shots[name])
            if trace:
                print(f"{work.parent.parent.name}: {keys} {name} at {session.now_us} emulated us", file=sys.stderr)
        if ended is not None:
            status = session.send({"cmd": "virtual_time_status"})
            ended.update(us=status["emulated_us"], cycles_per_ms=status["cycles_per_ms"])
    finally:
        session.close()
    return shots


def script(name: str) -> list:
    return SCRIPTS[name] + PLAYED[name][0]


def region_difference(want: Path, got: Path, box: tuple[int, int, int, int]) -> int:
    (width, _, left), (_, _, right) = qbplay.png_pixels(want), qbplay.png_pixels(got)
    scale = width // 640
    x0, y0, x1, y1 = (v * scale for v in box)

    def pixel(data: bytes, x: int, y: int) -> bytes:
        return data[(y * width + x) * 3 : (y * width + x) * 3 + 3]

    return sum(pixel(left, x, y) != pixel(right, x, y) for y in range(y0, y1) for x in range(x0, x1))


def difference_mask(want: Path, got: Path, cols: int = 64, rows: int = 25) -> str:
    """Where two screens differ, as text a job log can hold: the bounding box, then a grid of the screen with `#` in each cell that has a
    differing pixel (the screens themselves stay out of a private group's log)."""
    (width, height, left), (_, _, right) = qbplay.png_pixels(want), qbplay.png_pixels(got)
    hits = [(i // 3 % width, i // 3 // width) for i in range(0, len(left), 3) if left[i : i + 3] != right[i : i + 3]]
    if not hits:
        return "no differing pixel"
    grid = [[" "] * cols for _ in range(rows)]
    for x, y in hits:
        grid[y * rows // height][x * cols // width] = "#"
    xs, ys = [x for x, _ in hits], [y for _, y in hits]
    return f"{len(hits)} pixels in x {min(xs)}-{max(xs)}, y {min(ys)}-{max(ys)} of {width}x{height}\n" + "\n".join("|" + "".join(row) + "|" for row in grid)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def hash_lines(name: str, target: str, built, objects: dict[str, Path]) -> list[str]:
    """`sha256 label` for each file a demo's build of `target` is made of: its object, the runtime objects it links, the EXE, and (once per run)
    the DOSBox-X binary and the demo conf. A hosted run's table against a local run's of the same commit says whether the build is the
    same (then the host's emulator is the difference) or not (then the compiler's output depends on the host)."""
    obj = built.exe.with_suffix(".obj")
    files = {f"{name}.{target}.obj": obj, **{f"runtime/{one.name}": one for one in qbbuild.needed((obj,), objects)}, f"{name}.{target}.EXE": built.exe}
    return [f"{sha256(path)} {label}" for label, path in sorted(files.items())]


def record_hashes(name: str, built: dict, objects: dict[str, dict[str, Path]]) -> None:
    """Print the hash table of each target's build and keep it in the gate's logs beside the screens."""
    lines = [f"{hashlib.sha256(qb32.DEMO_CONF.encode()).hexdigest()} demo conf", f"{sha256(Path(dosbatch.DOSBOX).resolve())} dosbox-x"]
    for target, one in built.items():
        lines += hash_lines(name, target, one, objects[target])
    print("\n".join(f"hash {line}" for line in lines))
    if logs := os.environ.get("CARGO_TARGET_DIR"):
        (kept := Path(logs) / "gate-logs").mkdir(parents=True, exist_ok=True)
        with (kept / "demo-hashes.txt").open("a") as out:
            out.write("\n".join(f"{name} {line}" for line in lines) + "\n")


def keep_screens(name: str, target: str, checkpoint: str, want: Path, got: Path) -> None:
    """Both screens of a mismatch into the gate's logs (CI uploads them): the pixel count alone cannot say what drew wrong."""
    if logs := os.environ.get("CARGO_TARGET_DIR"):
        (kept := Path(logs) / "gate-logs" / "screens").mkdir(parents=True, exist_ok=True)
        for kind, path in (("want", want), ("got", got)):
            shutil.copy(path, kept / f"{name}-{target}-{checkpoint}-{kind}.png")


def differences(name: str, want: dict[str, Path], got: dict[str, Path], target: str = "") -> list[str]:
    """The checkpoints of `name` whose screens differ by more than a blinking cursor."""
    boxes, bad = PLAYED[name][1], []
    for checkpoint in want:
        box = boxes.get(checkpoint)
        count = region_difference(want[checkpoint], got[checkpoint], box) if box else qbplay.differing_pixels(want[checkpoint], got[checkpoint])
        if count > CURSOR_PIXELS:
            bad.append(f"{checkpoint}: {count} pixels")
            print(f"{name} {target} {checkpoint}: {difference_mask(want[checkpoint], got[checkpoint])}")
            keep_screens(name, target, checkpoint, want[checkpoint], got[checkpoint])
    return bad


def _key(name: str, source: Path) -> str:
    linker = dosbatch.QB45 / "LINK.EXE"
    stamp = f"{linker.stat().st_size}:{int(linker.stat().st_mtime)}" if linker.exists() else ""
    return hashlib.sha256(source.read_bytes() + repr(script(name)).encode() + stamp.encode() + qb32.DEMO_CONF.encode() + b"lockstep").hexdigest()[:20]


def reference_shots(name: str, source: Path, work: Path) -> dict[str, Path]:
    """BCOM45's checkpoint screens of `name`: from the cache, or built, played and put there."""
    names = [one[2] for one in script(name) if one[2]]
    cached = CACHE / name / _key(name, source)
    if all((cached / f"{one}.png").exists() for one in names):
        return {one: cached / f"{one}.png" for one in names}
    work.mkdir(parents=True, exist_ok=True)
    (work / "build").mkdir(exist_ok=True)
    exe = qbplay.reference({name: source}, work / "build")[name]
    shots = play(exe, work / "play", script(name), name)
    cached.mkdir(parents=True, exist_ok=True)
    for one, path in shots.items():
        shutil.copy(path, cached / f"{one}.png")
    return {one: cached / f"{one}.png" for one in names}


def candidate_shots(name: str, built: dict, work: Path) -> dict[str, dict[str, Path]]:
    """The checkpoint screens of each target's build of `name`: {target: {checkpoint: screenshot}}."""
    work.mkdir(parents=True, exist_ok=True)
    shots = {}
    for target, one in built.items():
        play_dir = work / target / "play"
        play_dir.mkdir(parents=True, exist_ok=True)
        for beside in one.files:
            shutil.copy(beside, play_dir / Path(beside).name)
        shots[target] = play(one.exe, play_dir, script(name), name)
    return shots


def run(sources: dict[str, Path], work: Path) -> dict[str, dict[str, list[str]] | Exception]:
    """For each demo and target of ours, the checkpoints that differ from BCOM45's: [] where it looks the same. A demo that could not be
    played is its exception, not a failure of the others': GORILLA's `never settled` read as NIBBLES's too."""
    objects = qbbuild.runtimes(work)

    def one(name: str) -> dict[str, list[str]]:
        built = qbbuild.build(sources[name], work / name / "build", objects)
        record_hashes(name, built, objects)
        with ThreadPoolExecutor(2) as pool:
            reference = pool.submit(reference_shots, name, sources[name], work / name / "ref")
            candidate = pool.submit(candidate_shots, name, built, work / name / "cand")
            want, got = reference.result(), candidate.result()
        return {target: differences(name, want, shots, target) for target, shots in got.items()}

    with ThreadPoolExecutor(len(sources)) as pool:
        futures = {name: pool.submit(one, name) for name in sources}
        return {name: outcome(future) for name, future in futures.items()}


def outcome(future):
    """What a demo's run gave: its differences, or the exception that stopped it."""
    try:
        return future.result()
    except Exception as why:
        return why


def of(results: dict, name: str) -> dict[str, list[str]]:
    """`name`'s differences from `run`'s results; raises only if `name` itself could not be played."""
    if isinstance(results[name], Exception):
        raise results[name]
    return results[name]
