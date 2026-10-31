"""Where the built binaries are: LLRM_BIN, else $CARGO_TARGET_DIR/release, else <repo>/target/release.

The one resolver every Python tool and test uses; a tree built elsewhere (the gate's) has no ./target.
"""
import atexit
import contextlib
import hashlib
import os
import shutil
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
STAMPED = ("llrm-llc",)


# llrm-cc's table row and CPU for a C compile at `bits`, and the variables it reads to find its clang and Open Watcom headers.
C_ROWS = {16: ("x86_16-wat", "-march=i386"), 32: ("x86_32-wat", "-march=i486")}
CC_ENV = ("LLRM_OW", "LLRM_PIN_CLANG", "LLRM_CLANG_M16")


def c_args(bits: int) -> list[str]:
    """What names the target to llrm-cc: its row (a flag, so the measured environment needs no variable for it) and the CPU."""
    row, cpu = C_ROWS[bits]
    return [f"--target={row}", cpu]


def cc_environment() -> dict[str, str]:
    """The variables llrm-cc needs, from the caller's, with the usual default for the Open Watcom tree."""
    env = {name: os.environ[name] for name in CC_ENV if name in os.environ}
    env.setdefault("LLRM_OW", str(Path.home() / "scratch/ow-src"))
    pin = REPO / "toolchain/clang/pin"
    lines = pin.read_text().splitlines() if pin.exists() else []
    if len(lines) > 1 and "LLRM_CLANG_M16" not in env and (m16 := Path.home() / "scratch/clang-by-sha" / lines[1].split()[0] / "bin/clang").exists():
        env["LLRM_CLANG_M16"] = str(m16)
    return env


def compiler(found: Path) -> Path:
    """The C compiler in the binaries directory `found`: llrm-cc. Every tool names it through here."""
    return found / "llrm-cc"


def bin_dir(env: dict | None = None, repo: Path = REPO) -> Path:
    env = os.environ if env is None else env
    if env.get("LLRM_BIN"):
        found = Path(env["LLRM_BIN"])
    elif env.get("CARGO_TARGET_DIR"):
        found = Path(env["CARGO_TARGET_DIR"]) / "release"
    else:
        found = target_dir(env, repo) / "release"
    stale = stale_binaries(found, repo)
    if stale:
        raise SystemExit(f"stale binaries in {found}: {', '.join(stale)} older than the source they are built from; run `cargo build --release --bins` (`cargo build -p X` builds only the library)")
    stamped(found)
    return found


def stamped(found: Path) -> None:
    """Prints the commit the binaries were built from (llrm-llc's stamp); refuses another than LLRM_EXPECT_SHA names. A tree with changes is the gate's own, so it is not refused here."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import stamp

    expect = os.environ.get("LLRM_EXPECT_SHA") or None
    # llrm-cc prints clang's --version; the stamp is the backend's, which is built from the same commit.
    stamped_one = next((found / name for name in STAMPED if (found / name).is_file()), None)
    if stamped_one:
        try:
            stamp.check(stamped_one, expect, allow_dirty=True, label=str(found))
        except SystemExit:
            if expect:
                raise  # a binary without a stamp is only refused where a sha is named


def declared(repo: Path) -> set[str]:
    """The binaries `cargo build --bins` builds: the root package's [[bin]] names (others in target/ are leftovers)."""
    import tomllib

    manifest = repo / "Cargo.toml"
    return {one["name"] for one in tomllib.loads(manifest.read_text()).get("bin", [])} if manifest.is_file() else set()


def built_from(directory: Path, default: Path) -> Path:
    """The tree the binaries in `directory` were built from: the root (a Cargo.toml beside a crates/) above the first workspace source
    their dep-info lists, else `default`. A measurement's base is built in a tree of its own, not in the one being gated."""
    for info in sorted(directory.glob("*.d")) if directory.is_dir() else []:
        for path in info.read_text().partition(":")[2].replace("\\\n", " ").split():
            for parent in Path(path).parents:
                if (parent / "Cargo.toml").is_file() and (parent / "crates").is_dir():
                    return parent
    return default


def recorded_md5(directory: Path) -> dict[str, str]:
    """name -> md5 from the `ok` file beside a `dist/` that tools/pairs/pair_build.sh built (`<md5>  <name>` lines, written at build time), else empty."""
    ok = directory.parent / "ok"
    if directory.name != "dist" or not ok.is_file():
        return {}
    pairs = (line.split() for line in ok.read_text().splitlines())
    return {fields[1]: fields[0] for fields in pairs if len(fields) == 2 and len(fields[0]) == 32}


def md5_of(binary: Path) -> str:
    return hashlib.md5(binary.read_bytes()).hexdigest()


def stale_found(directory: Path, repo: Path = REPO) -> list[tuple[str, float, str, float]]:
    """A binary of a pair_build dist is judged by its md5 against the `ok` record (`recorded_md5`), not by time. (binary, its mtime, the newest source it is built from, that source's mtime) for each binary in `directory` older than a file cargo's dep-info says it is
    built from, or than the toolchain patches (build.rs reruns on them; dep-info does not list them) of the tree they were built from, which is `repo` unless
    their dep-info names another. A directory without dep-info is not checked."""
    repo = built_from(directory, repo)
    patches = [(one.stat().st_mtime, str(one)) for one in (repo / "toolchain/owshim").rglob("*") if one.is_file()]
    stale, built_here = [], declared(repo)
    for info in sorted(directory.glob("*.d")) if directory.is_dir() else []:
        binary = info.with_suffix("")
        if binary.name not in built_here or not binary.is_file():
            continue
        built = binary.stat().st_mtime
        # a pair_build dist records each binary's md5 when it is built: that decides, not the mtime (a copy or a checkout moves mtimes, and touching one would pass the guard)
        if recorded := recorded_md5(directory).get(binary.name):
            if md5_of(binary) != recorded:
                stale.append((binary.name, built, f"{directory.parent / 'ok'} (md5 differs)", built))
            continue
        sources = [Path(path) for path in info.read_text().partition(":")[2].replace("\\\n", " ").split()]
        # Sources only: a build script rewrites its outputs under the target directory whenever it reruns.
        written = directory.resolve().parent
        tracked = [source for source in sources if source.is_file() and source.resolve().is_relative_to(repo.resolve()) and not source.resolve().is_relative_to(written)]
        newest, where = max([(source.stat().st_mtime, str(source)) for source in tracked] + patches, default=(0.0, ""))
        if newest > built:
            stale.append((binary.name, built, where, newest))
    return stale


def stale_binaries(directory: Path, repo: Path = REPO) -> list[str]:
    """The binaries in `directory` older than the newest source they are built from (`stale_found`)."""
    return [name for name, *_ in stale_found(directory, repo)]


def stale_detail(directory: Path, repo: Path = REPO) -> str:
    """Which binary is the oldest and which source the newest, with their times: a rebuild that left a binary stale is a failed build or a clock, and this tells which."""
    import time

    found = stale_found(directory, repo)
    if not found:
        return ""
    oldest = min(found, key=lambda one: one[1])
    newest = max(found, key=lambda one: one[3])
    clock = lambda t: time.strftime("%H:%M:%S", time.localtime(t))
    future = " (its mtime is in the FUTURE: a copy that kept a future timestamp stays newer than any binary built today)" if newest[3] > time.time() else ""
    return f"oldest binary {oldest[0]} built {clock(oldest[1])}; newest source {newest[2]} written {clock(newest[3])} ({newest[3] - oldest[1]:.0f} s later){future}"


def target_dir(env: dict | None = None, repo: Path = REPO) -> Path:
    """Cargo's target directory, which the tools' scratch work goes under: CARGO_TARGET_DIR, else <repo>/target."""
    env = os.environ if env is None else env
    return Path(env["CARGO_TARGET_DIR"]) if env.get("CARGO_TARGET_DIR") else repo / "target"


if __name__ == "__main__":
    import sys

    kinds = {"bin": bin_dir, "target": target_dir}
    if len(sys.argv) != 2 or sys.argv[1] not in kinds:
        sys.exit("usage: llrmbin.py bin|target")
    print(kinds[sys.argv[1]]())


PINNED_BYTES = 120
_pins: list[str] = []


def pinned(binary: Path) -> Path:
    """`binary` by a path of PINNED_BYTES bytes, a symlink of this process's own. The compiler reads its argv[0] at start-up and the
    length of it moved an instruction count by 1.6e-4 to 5e-4 (`functions` at 2N: 3,192,407,376 from a short directory, 3,192,925,702
    from a long one), the size of a tolerance on a ratio: the tree a gate builds in and the one a base was measured in differ in it."""
    if not _pins:
        _pins.append(tempfile.mkdtemp(prefix="llrm-pin-", dir="/tmp"))
        atexit.register(shutil.rmtree, _pins[0], True)
    key = hashlib.sha256(str(binary).encode()).hexdigest()[:8]
    room = PINNED_BYTES - len(_pins[0]) - len("/") - len(binary.name) - 1
    folder = Path(_pins[0]) / (key + "p" * (room - len(key)))
    folder.mkdir(exist_ok=True)
    link = folder / binary.name
    try:
        link.symlink_to(os.path.abspath(binary))
    except FileExistsError:  # another thread of this process pinned it first
        pass
    return link


def md5(path) -> str:
    return hashlib.md5(Path(path).read_bytes()).hexdigest()


def assert_distinct(base, new, same_source: bool = False) -> str:
    """The header line of an A/B table: both binaries' md5. SystemExit when they are equal and the sources are not known to be: `cargo build -p X` does not
    relink target/release/X, so two builds of different commits were the same binary and a pair read "0 differ" (lir-cleanup-port's identity run)."""
    a, b = md5(base), md5(new)
    if a == b and not same_source:
        raise SystemExit(f"same binary: base {base} and new {new} both have md5 {a}; the new commit was not built into it (`cargo build --bin X`, not `-p`). Refusing to print a diff.")
    return f"binaries: base {a[:12]} {base} | new {b[:12]} {new}" + (" (same binary, same source)" if a == b else "")


OBJECT = "<object>"
OBJECT_BYTES = 100


@contextlib.contextmanager
def with_object(command: list[str]):
    """`command` with the `OBJECT` placeholder made a path to write the object to. llrm-cc encodes only for an `.o` or `.obj` output; `-o /dev/null` gets
    the assembly text, so a cost measured there leaves out the encoder and the object write (emit: 4 M of 22 M Ir on d_faces, 2026-10-23). The path is
    OBJECT_BYTES long whatever the directory: a longer argv moved an instruction count (see `pinned`)."""
    if OBJECT not in command:
        yield command
        return
    with tempfile.TemporaryDirectory(prefix="llrm-obj-", dir="/tmp") as directory:
        path = Path(directory) / ("o" * (OBJECT_BYTES - len(directory) - len("/.obj")) + ".obj")
        yield [str(path) if one == OBJECT else one for one in command]
