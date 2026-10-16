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


def test_two_jobs_fetching_at_once_do_not_share_a_partial_file(monkeypatch, tmp_path):
    """Both wrote `signal.partial`; the first rename took it and the second died with FileNotFoundError (gate(tools) on #1857)."""
    packed = tmp_path / "t.bz2"
    packed.write_bytes(bz2.compress(TEXT))
    register(monkeypatch, tmp_path, [packed.as_uri()])
    replace, racing = Path.replace, []

    def other_job_first(self, target):
        if not racing:
            racing.append(self)
            corpus.path("t")  # the other job runs between this one's write and its rename
        return replace(self, target)

    monkeypatch.setattr(Path, "replace", other_job_first)
    assert corpus.path("t").read_bytes() == TEXT
    assert list((tmp_path / "cache").glob("*.partial")) == []


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


def test_the_signal_is_generated_offline_to_its_pinned_hash(monkeypatch, tmp_path):
    """The STFT benches' input has no site: it is built here, and a generator that drifts is Unavailable, not another signal."""
    monkeypatch.setattr(corpus, "CACHE", tmp_path / "cache")
    assert len(corpus.path("signal").read_bytes()) == 131072
    monkeypatch.setitem(corpus.CORPORA, "signal", corpus.Corpus("0" * 64, 131072, (), lambda: b"x"))
    (tmp_path / "cache" / "signal").unlink()
    with pytest.raises(corpus.Unavailable, match="pinned"):
        corpus.path("signal")
