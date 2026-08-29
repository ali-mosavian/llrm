"""
Memoizes a dosbox.launch() call: given the same workdir inputs, the same batch
commands, and the same toolchain, DOSBox produces the same output every time,
so a warm run copies that output back instead of booting DOSBox again.

The key is the workdir's own file tree exactly as the caller left it right
before this is called -- whatever is sitting there (a copied-in .bas fixture,
or a prior step's own compiled and host-rewritten .OBJ) is exactly what DOSBox
is about to read, so hashing that tree hashes the real input rather than a
hand-maintained proxy for it. Content is hashed, not mtime, so a fixture
edited and reverted lands on the key it always had, and a fixture only
touched is not a cache miss by itself. This is also what makes a link+run step
invalidate correctly when qbopt's own rewrite changes: the rewritten .OBJ it
is about to link is already sitting in the workdir when the key is computed,
so a different rewrite means a different key, with no separate mechanism
needed to notice that.

launch() itself writes RUN.BAT and dosbox.conf into the workdir, and they
carry the workdir's own absolute host path -- left in place for a second
launch() in the same directory (compile then link), that path would leak into
a key that has nothing to do with it. They are excluded from the tree here for
exactly that reason.

The identity of the compiler is not in the workdir at all -- BC.EXE, LINK.EXE
and the runtime .LIB live on the mounted drive, named only by a DOS path
inside `lines`. The caller passes their bytes explicitly as `identity` rather
than have this module guess which strings in `lines` are paths, so a config
that forgets to name one of its own binaries fails to compile, not to cache
correctly.

Only a run that actually finished is cached -- a timeout is not memoized, so a
hung toolchain is retried rather than replayed as a permanent failure. A run
that finished with BC or LINK reporting an error is still memoized: the
programs here are deterministic and a compiler error is as much a real answer
as a clean object, but see docs/testing.md for what that means when the
*environment*, not the source, is what actually failed.
"""

import os
import shutil
import hashlib
import tempfile
import contextlib
from pathlib import Path
from time import monotonic
from functools import cache

from dosbox import Run
from dosbox import FAST
from dosbox import MARKER
from configs import Config
from dosbox import host_path
from dosbox import dosbox_bin
from dosbox import launch as _boot

CACHE_ROOT = Path(__file__).resolve().parents[1] / "build" / "launch-cache"

# launch()'s own scaffolding: written fresh by every launch() call, so
# reflects only that call's own inputs, but it names the workdir's absolute
# host path, which is not one of them.
_SCAFFOLD = {"run.bat", "dosbox.conf", MARKER.lower()}


@cache
def _file_digest(path: Path) -> bytes:
    return hashlib.sha256(path.read_bytes()).digest()


def toolchain_identity(cfg: Config) -> bytes:
    digest = hashlib.sha256()
    for dos_path in (cfg.bc, cfg.link, cfg.runtime):
        digest.update(_file_digest(host_path(cfg.mount, dos_path)))
    binary = dosbox_bin()
    digest.update(_file_digest(Path(binary)) if binary else b"<no dosbox-x>")
    return digest.digest()


def _write(digest: "hashlib._Hash", data: bytes) -> None:
    # length-prefixed, so two fields whose concatenation happens to match
    # never collide the way two bare concatenations could
    digest.update(len(data).to_bytes(8, "big"))
    digest.update(data)


def _tree_digest(digest: "hashlib._Hash", root: Path) -> None:
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        if path.name.lower() in _SCAFFOLD:
            continue
        _write(digest, path.relative_to(root).as_posix().encode())
        _write(digest, path.read_bytes())


def cache_key(
    workdir: Path,
    lines: list[str],
    conf: str,
    env: dict[str, str] | None,
    identity: bytes,
) -> str:
    digest = hashlib.sha256()
    _tree_digest(digest, workdir)
    _write(digest, "\n".join(lines).encode())
    _write(digest, conf.encode())
    _write(digest, repr(sorted((env or {}).items())).encode())
    _write(digest, identity)
    return digest.hexdigest()


def _load(entry: Path, workdir: Path) -> Run:
    started = monotonic()
    shutil.rmtree(workdir, ignore_errors=True)
    shutil.copytree(entry, workdir)
    return Run(True, False, monotonic() - started)


def _store(entry_root: Path, key: str, workdir: Path) -> None:
    with tempfile.TemporaryDirectory(dir=entry_root) as staging:
        staged = Path(staging) / "tree"
        shutil.copytree(workdir, staged)
        # another worker finishing the identical key first is not an error
        with contextlib.suppress(OSError):
            os.replace(staged, entry_root / key)


def cached_launch(
    workdir: Path,
    mount_v: Path,
    lines: list[str],
    *,
    identity: bytes,
    timeout: int = 300,
    conf: str = FAST,
    env: dict[str, str] | None = None,
) -> Run:
    if os.environ.get("QBOPT_NO_LAUNCH_CACHE"):
        return _boot(workdir, mount_v, lines, timeout=timeout, conf=conf, env=env)

    key = cache_key(workdir, lines, conf, env, identity)
    CACHE_ROOT.mkdir(parents=True, exist_ok=True)
    entry = CACHE_ROOT / key
    if entry.is_dir():
        return _load(entry, workdir)

    run = _boot(workdir, mount_v, lines, timeout=timeout, conf=conf, env=env)
    if run.finished and not run.timed_out:
        _store(CACHE_ROOT, key, workdir)
    return run
