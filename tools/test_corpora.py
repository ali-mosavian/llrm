import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import corpora  # noqa: E402


@pytest.fixture
def trees(tmp_path, monkeypatch):
    """Two small trees registered as `dfaces` and `qcport`, and a third that is neither."""
    made = {}
    for name, body in (("dfaces", "int d;"), ("qcport", "int q;"), ("other", "int o;")):
        root = tmp_path / name
        (root / "render").mkdir(parents=True)
        (root / "render" / "a.c").write_text(body)
        (root / "render" / "a.h").write_text(body)
        made[name] = root
    monkeypatch.setitem(corpora.REGISTRY, "dfaces", corpora.Corpus("DFACES_SRC", str(made["dfaces"]), "d", corpora.tree_hash(made["dfaces"])))
    monkeypatch.setitem(corpora.REGISTRY, "qcport", corpora.Corpus("QCPORT", str(made["qcport"]), "q", corpora.tree_hash(made["qcport"])))
    monkeypatch.delenv("DFACES_SRC", raising=False)
    monkeypatch.delenv("QCPORT", raising=False)
    return made


def test_the_registered_tree_is_accepted_and_its_header_names_the_corpus_and_hash(trees):
    path = corpora.resolve("dfaces")
    assert path == trees["dfaces"]
    assert corpora.header("dfaces", path).startswith(f"corpus dfaces: {path} tree {corpora.tree_hash(path)}")


def test_a_modified_tree_is_refused_unless_the_override_flag_is_given(trees):
    """A tree edited in place measured as if it were the pin; the hash now differs and the tool stops."""
    (trees["dfaces"] / "render" / "a.c").write_text("int d = 1;")
    with pytest.raises(SystemExit, match="not the registered"):
        corpora.resolve("dfaces")
    assert corpora.resolve("dfaces", allow_other=True) == trees["dfaces"]


def test_qcports_src_given_as_the_d_faces_tree_is_refused_naming_qcport(trees):
    """`QCPORT` (the gate's modules, without the game headers) was read as the d_faces tree: the compile stopped at ~8 M instructions and was counted."""
    with pytest.raises(SystemExit, match="registered corpus `qcport`"):
        corpora.resolve("dfaces", trees["qcport"])


def test_the_environment_variable_names_the_tree_and_an_argument_wins_over_it(trees, monkeypatch):
    monkeypatch.setenv("DFACES_SRC", str(trees["other"]))
    with pytest.raises(SystemExit, match="not the registered"):
        corpora.resolve("dfaces")
    assert corpora.resolve("dfaces", trees["dfaces"]) == trees["dfaces"]


def test_a_missing_tree_is_an_error_not_an_empty_hash(trees, tmp_path):
    with pytest.raises(SystemExit, match="not a directory"):
        corpora.resolve("dfaces", tmp_path / "nowhere")


def test_the_gcc_copy_is_a_triage_tree_not_a_corpus_of_record():
    one = corpora.REGISTRY["qcport-gcc"]
    assert one.hash == "52d87710a4fc" and not one.record and "triggers.c:354" in one.purpose


def test_the_hash_ignores_other_files_and_depends_on_paths_and_contents(tmp_path):
    (tmp_path / "x.c").write_text("a")
    first = corpora.tree_hash(tmp_path)
    (tmp_path / "notes.txt").write_text("not source")
    assert corpora.tree_hash(tmp_path) == first
    (tmp_path / "y.h").write_text("b")
    assert corpora.tree_hash(tmp_path) != first
