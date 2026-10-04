"""`uv run --project tools python -m pytest tools/dosbatch/test_corpus.py`

The corpus cache: a file is used only when it is the pinned one (rule 3)."""

import sys
import bz2
import hashlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import pytest  # noqa: E402
import corpus  # noqa: E402

TEXT = b"the pinned text\r\n" * 50


def register(monkeypatch, tmp_path, sources, sha=None):
    monkeypatch.setattr(corpus, "CACHE", tmp_path / "cache")
    monkeypatch.setitem(corpus.CORPORA, "t", corpus.Corpus(sha or hashlib.sha256(TEXT).hexdigest(), len(TEXT), tuple(sources)))


def test_a_missing_file_is_fetched_unpacked_and_cached(monkeypatch, tmp_path):
    packed = tmp_path / "t.bz2"
    packed.write_bytes(bz2.compress(TEXT))
    register(monkeypatch, tmp_path, [packed.as_uri()])
    assert corpus.path("t").read_bytes() == TEXT


def test_a_cached_file_that_is_not_the_pinned_one_is_replaced_not_used(monkeypatch, tmp_path):
    """A truncated download in the cache read as the corpus: the scan ran over other bytes and printed other numbers."""
    packed = tmp_path / "t.bz2"
    packed.write_bytes(bz2.compress(TEXT))
    register(monkeypatch, tmp_path, [packed.as_uri()])
    (tmp_path / "cache").mkdir()
    (tmp_path / "cache" / "t").write_bytes(TEXT[:-1])
    assert corpus.path("t").read_bytes() == TEXT


def test_a_source_with_another_file_is_never_substituted(monkeypatch, tmp_path):
    other = tmp_path / "t.bz2"
    other.write_bytes(bz2.compress(b"some other text"))
    register(monkeypatch, tmp_path, [other.as_uri()])
    with pytest.raises(corpus.Unavailable, match="pinned"):
        corpus.path("t")
    assert not (tmp_path / "cache" / "t").exists()


def test_the_second_source_is_the_fallback(monkeypatch, tmp_path):
    good = tmp_path / "t.bz2"
    good.write_bytes(bz2.compress(TEXT))
    register(monkeypatch, tmp_path, [(tmp_path / "gone.bz2").as_uri(), good.as_uri()])
    assert corpus.path("t").read_bytes() == TEXT


def test_missing_and_offline_says_so(monkeypatch, tmp_path):
    register(monkeypatch, tmp_path, [(tmp_path / "gone.bz2").as_uri()])
    with pytest.raises(corpus.Unavailable, match="could not be fetched"):
        corpus.path("t")


def test_the_pinned_dickens_is_the_one_silesia_publishes():
    assert corpus.CORPORA["dickens"].size == 10192446
    assert len(corpus.CORPORA["dickens"].sha256) == 64
