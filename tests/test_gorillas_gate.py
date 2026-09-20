import json
import runpy
import hashlib
from typing import Any
from pathlib import Path
from collections.abc import Callable

import pytest

from tools.dosbox import Run

pytestmark = pytest.mark.fast


ROOT = Path(__file__).resolve().parents[1]
Runner = Callable[[Path, Path, list[str], int], Run]


def _namespace() -> dict[str, Any]:
    return runpy.run_path(ROOT / "tools/gorillas_gate.py")


def _source() -> bytes:
    return """SUB GetInputs (Player1$, Player2$, NumGames)
END SUB
FUNCTION GetNum# (Row, Col)
END FUNCTION
DECLARE SUB SparklePause ()
SUB GorillaIntro (Player1$, Player2$)
  DO WHILE Char$ = ""
    Char$ = INKEY$
  LOOP
END SUB
SUB PlayGame (Player1$, Player2$, NumGames)
  FOR i = 1 TO NumGames
    RANDOMIZE (TIMER)
  NEXT i

  SCREEN 0
  WIDTH 80, 25
END SUB
SUB Rest (t#)
END SUB
SUB SparklePause
END SUB
""".encode("latin1")


def _run(namespace: dict[str, Any], tmp_path: Path, runner: Runner, *, source: bytes | None = None) -> Path:
    source_path = tmp_path / "gorilla.bas"
    source_path.write_bytes(source or _source())
    globals_ = namespace["run"].__globals__
    globals_["GORILLAS_SHA256"] = hashlib.sha256(source_path.read_bytes()).hexdigest()
    globals_["host_path"] = lambda *_args: Path(__file__)
    return namespace["run"](
        namespace["Options"](source_path, tmp_path / "evidence", 1),
        runner=runner,
        emitter=lambda _source: (b"candidate", {"qbfront_sha256": "front", "emitter_sha256": "emitter"}),
    )


def _write_common(work: Path, *, output: bytes = b"GORILLAS-PROBE-1\r\nDONE\r\n", link: str = "ok") -> None:
    for name in ("BASE.OBJ", "BASE.EXE", "CAND.EXE"):
        (work / name).write_bytes(name.encode())
    for name in ("BASEBC.OUT", "BASELINK.OUT"):
        (work / name).write_text("0 Severe Errors\r\n" if name == "BASEBC.OUT" else "ok\r\n")
    (work / "CANDLINK.OUT").write_text(link)
    (work / "BASE.OUT").write_text("")
    (work / "CAND.OUT").write_text("")
    map_text = "00000H 00009H 0000AH GORILLA_CODE BC_CODE\n0000AH 0000BH 00002H RUNTIME CODE\n"
    (work / "BASE.MAP").write_text(map_text)
    (work / "CAND.MAP").write_text(map_text)
    (work / "GORILLAB.OUT").write_bytes(output)
    (work / "GORILLAQ.OUT").write_bytes(output)


def _runner_for(namespace: dict[str, Any], *, output: bytes | None = None, link: str = "ok", omit: str = "") -> Runner:
    def runner(work: Path, _mount: Path, _lines: list[str], _timeout: int) -> Run:
        _write_common(work, output=output or b"GORILLAS-PROBE-1\r\nDONE\r\n", link=link)
        if omit:
            (work / omit).unlink()
        return Run(True, False, 0.1)

    return runner


def test_happy_gate_writes_deterministic_receipt(tmp_path: Path) -> None:
    namespace = _namespace()
    receipt_path = _run(namespace, tmp_path, _runner_for(namespace))
    receipt = json.loads(receipt_path.read_text())

    assert receipt["status"] == "passed"
    assert receipt["source"]["sha256"] == hashlib.sha256(_source()).hexdigest()
    assert receipt["transformation"]["derived"]["path"] == "GORILLA.BAS"
    assert receipt["quality"]["basic"] == {"baseline": 10, "candidate": 10}
    assert receipt["commands"][0].endswith("/O /E /X /FPi GORILLA.BAS, BASE.OBJ; > BASEBC.OUT")


def test_source_hash_mismatch_writes_failed_receipt(tmp_path: Path) -> None:
    namespace = _namespace()
    source = tmp_path / "gorilla.bas"
    source.write_bytes(b"wrong")

    with pytest.raises(ValueError, match="sha256"):
        namespace["run"](namespace["Options"](source, tmp_path / "evidence", 1))

    receipt = json.loads((tmp_path / "evidence/gorillas-gate.json").read_text())
    assert receipt["status"] == "failed"
    assert "sha256" in receipt["error"]


def test_probe_rejects_anchor_drift() -> None:
    namespace = _namespace()

    with pytest.raises(ValueError, match="random seed"):
        namespace["derive_probe"](_source().replace(b"RANDOMIZE (TIMER)", b"RANDOMIZE 7"))


def test_gate_refuses_stale_output_directory(tmp_path: Path) -> None:
    namespace = _namespace()
    source = tmp_path / "gorilla.bas"
    source.write_bytes(_source())
    output = tmp_path / "evidence"
    output.mkdir()
    (output / "old.obj").write_bytes(b"stale")

    with pytest.raises(ValueError, match="absent or empty"):
        namespace["run"](namespace["Options"](source, output, 1))

    assert (output / "old.obj").read_bytes() == b"stale"


def test_gate_rejects_compile_or_link_diagnostic(tmp_path: Path) -> None:
    namespace = _namespace()

    with pytest.raises(ValueError, match="rejected"):
        _run(namespace, tmp_path, _runner_for(namespace, link="error L2029 unresolved external"))

    receipt = json.loads((tmp_path / "evidence/gorillas-gate.json").read_text())
    assert receipt["status"] == "failed"


def test_gate_rejects_compiler_log_without_zero_severe_summary(tmp_path: Path) -> None:
    namespace = _namespace()

    def runner(work: Path, _mount: Path, _lines: list[str], _timeout: int) -> Run:
        _write_common(work)
        (work / "BASEBC.OUT").write_text("compiler completed\r\n")
        return Run(True, False, 0.1)

    with pytest.raises(ValueError, match="zero severe"):
        _run(namespace, tmp_path, runner)


def test_gate_rejects_candidate_object_changed_during_run(tmp_path: Path) -> None:
    namespace = _namespace()

    def runner(work: Path, _mount: Path, _lines: list[str], _timeout: int) -> Run:
        _write_common(work)
        (work / "CAND.OBJ").write_bytes(b"mutated")
        return Run(True, False, 0.1)

    with pytest.raises(ValueError, match="pre-run artifact changed: candidate_object"):
        _run(namespace, tmp_path, runner)


def test_gate_rejects_missing_completion_output(tmp_path: Path) -> None:
    namespace = _namespace()

    with pytest.raises(ValueError, match="missing fresh artifact"):
        _run(namespace, tmp_path, _runner_for(namespace, omit="GORILLAQ.OUT"))


def test_gate_rejects_differential_output(tmp_path: Path) -> None:
    namespace = _namespace()

    def runner(work: Path, _mount: Path, _lines: list[str], _timeout: int) -> Run:
        _write_common(work)
        (work / "GORILLAQ.OUT").write_bytes(b"GORILLAS-PROBE-1\r\nchecksum=2\r\nDONE\r\n")
        return Run(True, False, 0.1)

    with pytest.raises(ValueError, match="output differ"):
        _run(namespace, tmp_path, runner)


def test_gate_rejects_code_size_regression(tmp_path: Path) -> None:
    namespace = _namespace()

    def runner(work: Path, _mount: Path, _lines: list[str], _timeout: int) -> Run:
        _write_common(work)
        (work / "CAND.MAP").write_text("00000H 00009H 0000BH GORILLA_CODE BC_CODE\n")
        return Run(True, False, 0.1)

    with pytest.raises(ValueError, match="footprint"):
        _run(namespace, tmp_path, runner)
