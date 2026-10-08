"""Corpora the tests and benchmarks read that are too big to keep in the tree: downloaded once, cached under
~/.cache/llrm-bench (LLRM_BENCH_CACHE), and verified against a pinned SHA-256 on every use. A file that is missing,
or does not match, and cannot be fetched is `Unavailable`: its users skip, with the reason. Never another file in its place.
"""

from __future__ import annotations

import bz2
import io
import os
import hashlib
import zipfile
import urllib.request
from pathlib import Path
from dataclasses import dataclass

CACHE = Path(os.environ.get("LLRM_BENCH_CACHE", Path.home() / ".cache" / "llrm-bench"))


class Unavailable(Exception):
    pass


@dataclass(frozen=True)
class Corpus:
    sha256: str
    size: int
    sources: tuple[str, ...]  # tried in order; .bz2 and .zip are unpacked


CORPORA = {
    # The Silesia corpus's dickens: public-domain text, 10,192,446 bytes.
    "dickens": Corpus(
        "b24c37886142e11d0ee687db6ab06f936207aa7f2ea1fd1d9a36763c7a507e6a",
        10192446,
        ("https://sun.aei.polsl.pl/~sdeor/corpus/dickens.bz2", "https://github.com/MiloszKrajewski/SilesiaCorpus/raw/master/dickens.zip"),
    )
}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def unpack(source: str, data: bytes, name: str) -> bytes:
    if source.endswith(".bz2"):
        return bz2.decompress(data)
    if source.endswith(".zip"):
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            return archive.read(name)
    return data


def fetch(name: str, corpus: Corpus) -> bytes:
    """The corpus's bytes from the first source that gives the pinned hash."""
    problems = []
    for source in corpus.sources:
        try:
            with urllib.request.urlopen(source, timeout=120) as response:
                data = unpack(source, response.read(), name)
        except Exception as error:  # offline, a refusal, a bad archive: try the next source
            problems.append(f"{source}: {error}")
            continue
        if digest(data) == corpus.sha256:
            return data
        problems.append(f"{source}: its SHA-256 is not the pinned {corpus.sha256}")
    raise Unavailable(f"{name} is not cached in {CACHE} and could not be fetched (offline?): " + "; ".join(problems))


def path(name: str) -> Path:
    """The cached file for `name`, its hash checked now; fetched first if missing or wrong. Raises Unavailable."""
    corpus = CORPORA[name]
    file = CACHE / name
    if file.exists() and digest(file.read_bytes()) == corpus.sha256:
        return file
    data = fetch(name, corpus)
    CACHE.mkdir(parents=True, exist_ok=True)
    partial = file.with_name(file.name + ".partial")
    partial.write_bytes(data)
    partial.replace(file)
    return file
