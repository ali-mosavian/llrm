import json
from dataclasses import replace
from hashlib import sha256
from pathlib import Path
from unittest.mock import Mock

import pytest

from qbopt import rewrite
from qbopt.abi import profile, runtime
from qbopt.objectfile import omf


@pytest.fixture
def declaration(tmp_path: Path) -> tuple[Path, dict]:
    source = Path("fixtures/regressions/qrender-main-v-g3.obj").read_bytes()
    dependency = Path("fixtures/omf/hotlop-p-g2.obj").read_bytes()
    (tmp_path / "main.obj").write_bytes(source)
    (tmp_path / "dependency.obj").write_bytes(dependency)
    document = {
        "version": 1,
        "artifacts": {"main.obj": sha256(source).hexdigest(), "dependency.obj": sha256(dependency).hexdigest()},
        "contracts": {
            "HOST_SHUTDOWN": {
                "defined_in": "main.obj",
                "inputs": ["ax", "bx", "cx", "dx", "si", "di"],
                "evidence": "B$ENRA precedes flag reads; all six GP inputs and unknown effects retained.",
            }
        },
    }
    path = tmp_path / "contracts.json"
    path.write_text(json.dumps(document))
    return path, document


def test_audited_profile_keeps_unknown_effects(declaration: tuple[Path, dict]) -> None:
    path, _ = declaration
    loaded = profile.load(path)
    (rule,) = loaded.rules
    assert rule == replace(
        runtime.worst("HOST_SHUTDOWN"),
        inputs=frozenset(
            {runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI}
        ),
        evidence=rule.evidence,
    )
    assert rule.control is runtime.Control.UNKNOWN
    assert rule.cleanup is None
    assert rule.reads is rule.writes is runtime.Memory.ANY
    assert rule.clobbers == runtime.EVERY


@pytest.mark.parametrize(
    "field,value",
    [
        ("defined_in", "dependency.obj"),
        ("inputs", ["eax"]),
        ("inputs", ["ax", "ax"]),
        ("cleanup", -2),
        ("cleanup", 3),
        ("cleanup", True),
        ("evidence", ""),
        ("clobbers", []),
    ],
)
def test_invalid_profile_refuses(declaration: tuple[Path, dict], field: str, value: object) -> None:
    path, document = declaration
    document["contracts"]["HOST_SHUTDOWN"][field] = value
    path.write_text(json.dumps(document))
    with pytest.raises(ValueError):
        profile.load(path)


def test_stale_dependency_does_not_overwrite_cli_output(declaration: tuple[Path, dict]) -> None:
    path, _ = declaration
    (path.parent / "dependency.obj").write_bytes(b"changed dependency")
    output = path.parent / "existing.obj"
    output.write_bytes(b"keep this")
    with pytest.raises(SystemExit) as stopped:
        rewrite.main(["fixtures/omf/hotlop-p-g2.obj", "--contracts", str(path), "-o", str(output)])
    assert stopped.value.code == 2
    assert output.read_bytes() == b"keep this"


def test_profile_fingerprint_is_order_independent(declaration: tuple[Path, dict]) -> None:
    path, document = declaration
    first = profile.load(path)
    path.write_text(json.dumps(dict(reversed(list(document.items()))), indent=4))
    assert profile.load(path) == first


def test_duplicate_json_keys_are_not_silently_replaced(tmp_path: Path) -> None:
    path = tmp_path / "duplicate.json"
    path.write_text('{"version":1,"version":2}')
    with pytest.raises(ValueError, match="duplicate"):
        profile.load(path)


def test_stage_dumps_use_the_same_profile_and_native_mode(
    declaration: tuple[Path, dict], monkeypatch: pytest.MonkeyPatch
) -> None:
    from tools import stages

    path, _ = declaration
    output = path.parent / "stages"
    raised = Mock(wraps=stages._bodies)
    emitted = Mock(wraps=stages.wholeseg.emitted)
    monkeypatch.setattr(stages, "_bodies", raised)
    monkeypatch.setattr(stages.wholeseg, "emitted", emitted)
    assert (
        stages.main(
            ["fixtures/omf/hotlop-p-g2.obj", "--contracts", str(path), "--native-fpu", "--dump", str(output), "--quiet"]
        )
        == 0
    )
    expected = {rule.name: rule for rule in profile.load(path).rules}
    assert raised.call_args.kwargs["external_contracts"] == expected
    assert emitted.call_args.kwargs["external_contracts"] == expected
    assert emitted.call_args.kwargs["native_fpu"] is True
    assert any("rebuilt" in file.read_text() for file in output.glob("*-asm-emitted.txt"))


def test_cli_forwards_profile_and_marks_its_identity(
    declaration: tuple[Path, dict], monkeypatch: pytest.MonkeyPatch
) -> None:
    path, document = declaration
    output = path.parent / "rewritten.obj"
    capture = Mock(wraps=rewrite.wholeseg.emitted)
    monkeypatch.setattr(rewrite.wholeseg, "emitted", capture)
    assert rewrite.main(["fixtures/omf/hotlop-p-g2.obj", "--contracts", str(path), "-o", str(output)]) == 0
    loaded = profile.load(path)
    assert capture.call_count == 1
    assert capture.call_args.kwargs["external_contracts"] == {rule.name: rule for rule in loaded.rules}
    marker = omf.finalised_at(omf.parse(output.read_bytes()))
    assert marker is not None
    assert f"contracts={loaded.fingerprint}" in marker
    assert json.loads(output.with_suffix(".json").read_text())["contract_profile_sha256"] == loaded.fingerprint
    again, _ = rewrite.rewrite(output.read_bytes(), dry_run=False, contract_profile=loaded)
    assert again == output.read_bytes() and capture.call_count == 1
    document["contracts"]["HOST_SHUTDOWN"]["evidence"] += " Revised audit."
    path.write_text(json.dumps(document))
    with pytest.raises(rewrite.Finalised):
        rewrite.rewrite(again, dry_run=False, contract_profile=profile.load(path))
