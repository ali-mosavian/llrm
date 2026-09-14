"""
The corpus, computed once instead of once per test.

Every stage below is a pure function of an object's bytes, and the suite asks
for the same answer over and over: counted across a serial run, 3615
blocks.code_map() calls produce 110 distinct answers, 1109 ir.decode_module()
calls a few more than that, and 880 rewrite() calls a few hundred. Nothing
about that is wasted checking -- every one of those tests wants a real answer
about a real object -- so the redundancy belongs here rather than in the tests.

The key is SHA-256 over the input bytes, every qbopt source that computes the
answer, and the iced-x86 version they decode with. An edited fixture, an
edited pass and an upgraded decoder therefore all miss; no filename and no
mtime is part of it, so a fixture edited and reverted lands on the key it
always had. Set QBOPT_NO_CORPUS_CACHE to bypass both levels entirely.

Only plain data crosses to disk. CodeMap, extent.Partition and rewrite()'s own
(bytes, Region) pair are ints, strings and bytes, so a stored entry cannot be a
subtly wrong object. Module and the IR carry iced-x86 Instructions: those do
round trip faithfully -- emit() from unpickled nodes is byte-identical on all
110 -- but loading them back is only 1.8x faster than decoding them again
(1.19s against 2.09s for the whole corpus), which does not buy the risk. They
are memoised in the process and nowhere else.

What is handed out is shared, not copied, so nothing here may be mutated. Every
result but two is a frozen dataclass of immutable fields; the two lists --
Module.records and partitioned()'s blocks -- are handed out fresh at the top
level, and no test in this suite mutates either.
"""

import os
import pickle
import shutil
import hashlib
import tempfile
import contextlib
from typing import Any
from pathlib import Path
from functools import cache
from importlib import metadata
from collections.abc import Callable

from qbopt.model import ir
from qbopt.objectfile import omf
from qbopt.frontend import blocks
from qbopt.frontend import extent
from qbopt.objectfile import module
from qbopt.frontend.declen import Insn
from qbopt.objectfile.relocate import Shift
from qbopt.rewrite import Region
from qbopt.rewrite import rewrite
from qbopt.objectfile.relocate import relocate

ROOT = Path(__file__).resolve().parents[1]
CACHE_ROOT = ROOT / "build" / "corpus-cache"

_MEMO: dict[str, Any] = {}


@cache
def _pass_identity() -> bytes:
    digest = hashlib.sha256()
    for path in sorted((ROOT / "qbopt").rglob("*.py")):
        _write(digest, path.relative_to(ROOT).as_posix().encode())
        _write(digest, path.read_bytes())
    _write(digest, metadata.version("iced-x86").encode())
    return digest.digest()


def _write(digest: "hashlib._Hash", data: bytes) -> None:
    # length-prefixed, so two fields whose concatenation happens to match
    # never collide the way two bare concatenations could
    digest.update(len(data).to_bytes(8, "big"))
    digest.update(data)


def _key(stage: str, data: bytes, *extra: bytes) -> str:
    digest = hashlib.sha256()
    _write(digest, _pass_identity())
    _write(digest, stage.encode())
    _write(digest, data)
    for part in extra:
        _write(digest, part)
    return digest.hexdigest()


def _bypassed() -> bool:
    return bool(os.environ.get("QBOPT_NO_CORPUS_CACHE"))


def _in_process[T](key: str, compute: Callable[[], T]) -> T:
    if _bypassed():
        return compute()
    if key not in _MEMO:
        _MEMO[key] = compute()
    return _MEMO[key]


def _stored[T](entry: Path) -> tuple[T] | None:
    # a half-written or stale-format entry is a miss, never a wrong answer
    with contextlib.suppress(OSError, pickle.PickleError, EOFError, AttributeError, ImportError):
        return pickle.loads(entry.read_bytes())
    return None


@cache
def _generation() -> Path:
    """This pass's own entries, and only this pass's.

    Every key already carries _pass_identity(), so an entry written by any
    other source tree can never be hit again -- but left where they were they
    accumulate about four megabytes per edit to qbopt/. Naming the directory
    after the identity makes the stale ones findable and this one self-cleaning.
    """
    here = CACHE_ROOT / _pass_identity().hex()[:16]
    here.mkdir(parents=True, exist_ok=True)
    for other in CACHE_ROOT.iterdir():
        if other != here:
            shutil.rmtree(other, ignore_errors=True)
    return here


def _store[T](entry: Path, value: tuple[T]) -> None:
    handle, staged = tempfile.mkstemp(dir=_generation())
    with os.fdopen(handle, "wb") as out:
        out.write(pickle.dumps(value, protocol=5))
    # another worker finishing the identical key first is not an error
    with contextlib.suppress(OSError):
        os.replace(staged, entry)


def _on_disk[T](key: str, compute: Callable[[], T]) -> T:
    if _bypassed():
        return compute()
    if key in _MEMO:
        return _MEMO[key]
    entry = _generation() / key
    if (held := _stored(entry)) is not None:
        _MEMO[key] = held[0]
        return held[0]
    value = compute()
    _MEMO[key] = value
    # boxed, so a stage whose own answer is None is still a hit next time
    _store(entry, (value,))
    return value


@cache
def _contents(path: Path) -> bytes:
    return path.read_bytes()


def _bytes(source: Path | bytes) -> bytes:
    return source if isinstance(source, bytes) else _contents(source)


def loaded(source: Path | bytes) -> module.Module | None:
    data = _bytes(source)
    return _in_process(_key("module", data), lambda: module.of(omf.parse(data)))


def _module(data: bytes) -> module.Module:
    found = loaded(data)
    assert found is not None, "not a BASIC object this pass can read"
    return found


def mapped(source: Path | bytes) -> blocks.CodeMap | str:
    data = _bytes(source)
    return _on_disk(_key("code-map", data), lambda: blocks.code_map(_module(data)))


def partitioned(source: Path | bytes) -> list[blocks.Block]:
    data = _bytes(source)
    found_map = mapped(data)
    assert not isinstance(found_map, str), found_map
    shared = _in_process(_key("blocks", data), lambda: blocks.partition(_module(data), found_map))
    return list(shared)


def reached(source: Path | bytes) -> list[Insn] | str:
    data = _bytes(source)
    shared = _in_process(_key("reached", data), lambda: blocks.instructions(_module(data)))
    return shared if isinstance(shared, str) else list(shared)


def bodies(source: Path | bytes) -> tuple[ir.BodyIR, ...] | str:
    data = _bytes(source)
    return _in_process(_key("ir", data), lambda: ir.decode_module(_module(data)))


def extents(source: Path | bytes) -> extent.Partition | str:
    data = _bytes(source)
    return _on_disk(_key("extent", data), lambda: extent.partition(_module(data)))


def rewritten(
    source: Path | bytes,
    *,
    dry_run: bool,
    take: set[int] | None = None,
    max_regions: int | None = None,
) -> tuple[bytes, list[Region]]:
    data = _bytes(source)
    key = _key("rewrite", data, repr((dry_run, sorted(take) if take else None, max_regions)).encode())
    out, regions = _on_disk(key, lambda: rewrite(data, dry_run=dry_run, take=take, max_regions=max_regions))
    return out, list(regions)


def relocated(source: Path | bytes, shift: Shift) -> list[omf.Record] | str:
    data = _bytes(source)

    def compute() -> list[omf.Record] | str:
        records = omf.parse(data)
        segment = omf.code_segment(records)
        assert segment is not None
        seg, _name, size = segment
        return relocate(records, seg, omf.segment_image(records, seg, size), shift)

    # a Shift is plain data too, so pickling it is a content key like any other
    out = _on_disk(_key("relocate", data, pickle.dumps(shift, protocol=5)), compute)
    return out if isinstance(out, str) else list(out)


def mappable(source: Path | bytes) -> bool:
    data = _bytes(source)
    return _on_disk(_key("mappable", data), lambda: loaded(data) is not None and not isinstance(mapped(data), str))


def runtime_library(obj: Path) -> Path:
    """The BC runtime an object in fixtures/ links against. The CLI resolves
    every external before it optimizes anything, so an object alone is refused."""
    import re

    import pytest
    from configs import PDS71, QB45, VBDOS

    compiler = re.search(r"-([pqv])-", Path(obj).name).group(1)
    library = {
        "p": PDS71 / "LIB" / "BCL71ENR.LIB",
        "q": QB45 / "LIB" / "BCOM45.LIB",
        "v": VBDOS / "LIB" / "VBDCL10E.LIB",
    }[compiler]
    if not library.exists():
        pytest.skip(f"no {library}")
    return library
