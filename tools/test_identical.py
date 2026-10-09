"""identical.py fails where its corpus is smaller than the known one, instead of saying 'identical' of what is left."""
import importlib.util
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("identical", HERE / "identical.py")
identical = importlib.util.module_from_spec(spec)
spec.loader.exec_module(identical)


def test_the_programs_are_the_known_number(tmp_path):
    assert len(identical.corpus(tmp_path, None, None, programs_only=True)) >= identical.KNOWN_PROGRAMS


def test_without_qcport_it_fails_instead_of_skipping_it(tmp_path):
    """compile-cost.py's list drops QCport's modules when QCPORT is unset; an identity check on it compared 66 files and said so of 131."""
    with pytest.raises(SystemExit, match="QCPORT"):
        identical.corpus(tmp_path, None, None)


def test_a_qcport_with_fewer_modules_than_known_fails(tmp_path):
    (tmp_path / "host").mkdir()
    for n in range(3):
        (tmp_path / "host" / f"m{n}.c").write_text("int x;\n")
    with pytest.raises(SystemExit, match="QCport modules"):
        identical.corpus(tmp_path / "work", tmp_path, tmp_path, False) if (tmp_path / "work").mkdir() is None else None


def test_the_known_corpus_is_all_of_it(tmp_path):
    """The full corpus is programs + modules, each once: a name in both lists would silently shrink it."""
    import os
    qcport, headers = os.environ.get("QCPORT"), os.environ.get("QCPORT_INC")
    if not (qcport and headers):
        pytest.skip("no QCport")
    files = identical.corpus(tmp_path, Path(qcport).expanduser(), Path(headers).expanduser())
    assert len(files) >= identical.KNOWN_PROGRAMS + identical.KNOWN_QCPORT


def test_levels_given_on_the_command_line_are_not_read_as_options(tmp_path, monkeypatch):
    """`--levels -O2 -Os` was an argparse error (a value that starts with '-'), and a walk over forty commits printed the usage line forty
    times where it meant to say 'identical'."""
    import subprocess
    import sys
    fake = tmp_path / "cc"
    fake.write_text('#!/bin/sh\nwhile [ "$1" != -o ]; do shift; done\necho x > "$2"\n')
    fake.chmod(0o755)
    done = subprocess.run([sys.executable, str(HERE / "identical.py"), str(fake), str(fake), "--levels=-O2,-Os", "--programs-only"], capture_output=True, text=True, env={**__import__("os").environ, "CARGO_TARGET_DIR": str(tmp_path)})
    assert "usage:" not in done.stderr and "objects compared" in done.stdout, done.stderr[-300:] + done.stdout[-300:]
