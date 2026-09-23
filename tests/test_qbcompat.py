from typing import Any
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


def _write_frontend_manifest(namespace: dict, directory: Path, case_name: str) -> Path:
    case = next(case for case in namespace["cases"]("qb45") if case.name == case_name)
    obj = directory / f"{case.source.stem}.OBJ"
    obj.write_bytes(b"object emitted by test qb frontend fixture")
    manifest = directory / "frontend-manifest.json"
    manifest.write_text(
        __import__("json").dumps(
            {
                "schema": 1,
                "producer": "qbopt.frontend.qb",
                "profile": "qb45",
                "emitter_sha256": namespace["frontend_emitter_sha256"](),
                "cases": [
                    {
                        "name": case_name,
                        "dialect": case.profile,
                        "expected": case.expected(),
                        "inputs": namespace["_source_inputs"](case),
                        "runtime_recipe": namespace["_runtime_recipe"](case),
                        "status": "emitted",
                        "objects": [
                            {
                                "path": obj.name,
                                "sha256": __import__("hashlib").sha256(obj.read_bytes()).hexdigest(),
                            }
                        ],
                    }
                ],
            }
        )
    )
    return manifest


def test_vbdos_compatibility_suite_inherits_every_earlier_case() -> None:
    """A dialect suite that silently dropped its parent would turn missing coverage green."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    found = namespace["validate"]("vbdos")
    profiles = {one.profile for one in found}

    assert profiles == {"qb45", "pds71", "vbdos10"}
    assert len(found) == len({one.name for one in found})
    assert all(one.source.is_file() and one.expected() for one in found)


def test_help_coverage_points_to_cases_and_source_lines() -> None:
    """A coverage label without a real case and line once overstated the suite."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")

    for profile in ("qb45", "pds71", "vbdos"):
        topics = namespace["validate_coverage"](profile, verify_help=False)
        assert topics
        assert all(topic["status"] != "covered" or topic["cases"] for topic in topics)
        case_by_name = {case.name: case for case in namespace["cases"](profile)}
        assert all(
            topic["status"] != "covered"
            or any(case_by_name[name].values["runtime"] == "required" for name in topic["cases"])
            for topic in topics
        )


def test_help_coverage_loads_cases_once_per_profile(monkeypatch: pytest.MonkeyPatch) -> None:
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    globals_ = namespace["validate_coverage"].__globals__
    original = globals_["cases"]
    loaded: list[str] = []

    def cases(profile: str) -> list[Any]:
        loaded.append(profile)
        return original(profile)

    monkeypatch.setitem(globals_, "cases", cases)

    assert namespace["validate_coverage"]("vbdos", verify_help=False)
    assert loaded == ["vbdos"]


def test_measured_artifact_requires_a_fresh_prepared_dos_run(tmp_path: Path) -> None:
    """A stale copied golden once passed because the fixture compared it to itself."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    first = namespace["cases"]("qb45")[0]
    assert first.name == "text-screen-capture"
    artifact = first.values["artifacts"][0]
    golden = first.directory / first.values["artifact_goldens"][0]
    (tmp_path / artifact).write_bytes(golden.read_bytes())

    with pytest.raises(ValueError, match="fresh-run marker"):
        namespace["check_artifacts"]("qb45", tmp_path)

    manifest = _write_frontend_manifest(namespace, tmp_path, first.name)
    namespace["prepare_artifact_run"]("qb45", tmp_path, manifest)
    with pytest.raises(ValueError, match="did not produce"):
        namespace["check_artifacts"]("qb45", tmp_path)

    (tmp_path / artifact).write_bytes(b"wrong DOS output")
    with pytest.raises(ValueError, match="differs from measured golden"):
        namespace["check_artifacts"]("qb45", tmp_path)

    (tmp_path / artifact).write_bytes(golden.read_bytes())
    assert namespace["check_artifacts"]("qb45", tmp_path) == [tmp_path / artifact]


def test_artifact_older_than_prepared_run_is_rejected(tmp_path: Path) -> None:
    """An old valid artifact must not make a run that produced nothing look green."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    first = namespace["cases"]("qb45")[0]
    artifact = tmp_path / first.values["artifacts"][0]
    golden = first.directory / first.values["artifact_goldens"][0]
    manifest = _write_frontend_manifest(namespace, tmp_path, first.name)
    marker = namespace["prepare_artifact_run"]("qb45", tmp_path, manifest)
    artifact.write_bytes(golden.read_bytes())
    receipt = __import__("json").loads(marker.read_text())
    old_ns = receipt["started_ns"] - 1
    __import__("os").utime(artifact, ns=(old_ns, old_ns))

    with pytest.raises(ValueError, match="predates"):
        namespace["check_artifacts"]("qb45", tmp_path)


def test_help_extraction_hash_detects_same_line_count_corruption(tmp_path: Path) -> None:
    """Line counts alone let a truncated or edited FULL extraction pass validation."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    source = ROOT / "frontends/qb/compat/qb45/qb45qck.txt"
    damaged = tmp_path / "QB45QCK.TXT"
    data = source.read_bytes()
    damaged.write_bytes(bytes([data[0] ^ 1]) + data[1:])
    digest = __import__("hashlib").sha256(source.read_bytes()).hexdigest()

    assert len(damaged.read_bytes().splitlines()) == len(data.splitlines())
    with pytest.raises(ValueError, match="extraction hash changed"):
        namespace["_validate_help_extract"](
            tmp_path,
            {"path": damaged.name, "lines": len(data.splitlines()), "sha256": digest},
            tmp_path / "coverage.toml",
        )


def test_toml_evidence_selector_must_resolve() -> None:
    """A nonnumeric evidence suffix used to bypass evidence validation entirely."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    directory = ROOT / "frontends/qb/compat/vbdos"

    namespace["_validate_evidence"](directory, "suite.toml:g2_byval_long.switches", "valid selector")
    with pytest.raises(ValueError, match="does not resolve"):
        namespace["_validate_evidence"](directory, "suite.toml:g2_byval_long.not_a_field", "bad selector")


def test_gap_report_keeps_pending_compiler_work_visible() -> None:
    """Source fixtures are obligations, not proof that our compiler supports a feature."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    gaps = namespace["gap_obligations"]("vbdos")

    assert any("case qb45/gotos" in gap and "runtime=pending" in gap for gap in gaps)
    assert any("executable qb-compiler test required" in gap for gap in gaps)
    assert any("help QB45" in gap for gap in gaps)
    assert any("help BAS7" in gap or "help BC.HLP" in gap for gap in gaps)
    assert any("help VBDOS" in gap for gap in gaps)
    assert any("unclassified pds71/B7QCK.TXT" in gap for gap in gaps)
    assert any("unclassified vbdos/VBDOS.TXT" in gap for gap in gaps)


def test_full_help_headings_cannot_disappear_behind_sparse_coverage_rows() -> None:
    """Topic-count metadata once called 44 PDS rows complete for 1,183 headings."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")

    assert namespace["unclassified_help_obligations"]("qb45") == []
    pds = namespace["unclassified_help_obligations"]("pds71")
    vbdos = namespace["unclassified_help_obligations"]("vbdos")
    assert len(pds) == 409
    assert len(vbdos) == 983
    assert any("B7ENER.TXT" in gap and "Utility menu" in gap for gap in pds)
    assert any("VBDOS.TXT" in gap and "Invalid object reference" in gap for gap in vbdos)
    assert not any("ISAM" in gap or "OS/2" in gap for gap in pds + vbdos)
    for profile in ("pds71", "vbdos"):
        coverage = __import__("tomllib").loads((ROOT / f"frontends/qb/compat/{profile}/coverage.toml").read_text())
        assert coverage["help_exclusions"]
        assert not any(
            "isam" in f"{topic['context']} {topic['title']}".casefold()
            or "os/2" in f"{topic['context']} {topic['title']}".casefold()
            for topic in coverage["topic"]
        )


def test_artifact_window_rejects_bc_or_stale_object_provenance(tmp_path: Path) -> None:
    """An oracle BC object or an object changed after emission must never seed a PASS."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    manifest = _write_frontend_manifest(namespace, tmp_path, "text-screen-capture")
    data = __import__("json").loads(manifest.read_text())
    data["producer"] = "Microsoft BC"
    manifest.write_text(__import__("json").dumps(data))
    with pytest.raises(ValueError, match="not from the qb frontend"):
        namespace["prepare_artifact_run"]("qb45", tmp_path / "run", manifest)

    manifest = _write_frontend_manifest(namespace, tmp_path, "text-screen-capture")
    data = __import__("json").loads(manifest.read_text())
    data["cases"][0]["runtime_recipe"]["runtime_library"] = "WRONG.LIB"
    manifest.write_text(__import__("json").dumps(data))
    with pytest.raises(ValueError, match="mismatched runtime_recipe"):
        namespace["prepare_artifact_run"]("qb45", tmp_path / "run", manifest)

    manifest = _write_frontend_manifest(namespace, tmp_path, "text-screen-capture")
    data = __import__("json").loads(manifest.read_text())
    data["cases"][0]["inputs"][0]["sha256"] = "0" * 64
    manifest.write_text(__import__("json").dumps(data))
    with pytest.raises(ValueError, match="mismatched inputs"):
        namespace["prepare_artifact_run"]("qb45", tmp_path / "run", manifest)

    manifest = _write_frontend_manifest(namespace, tmp_path, "text-screen-capture")
    (tmp_path / "Q45L00.OBJ").write_bytes(b"stale replacement")
    with pytest.raises(ValueError, match="object changed"):
        namespace["prepare_artifact_run"]("qb45", tmp_path / "run", manifest)


def test_frontend_manifest_rejects_an_object_outside_its_emission_root(tmp_path: Path) -> None:
    """A valid hash must not let a BC-produced or unrelated OBJ enter through ../."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    manifest = _write_frontend_manifest(namespace, tmp_path, "numeric")
    data = __import__("json").loads(manifest.read_text())
    data["cases"][0]["objects"][0]["path"] = "../OUTSIDE.OBJ"
    manifest.write_text(__import__("json").dumps(data))

    with pytest.raises(ValueError, match="escapes its manifest directory"):
        namespace["_frontend_manifest"](manifest, "qb45")


def test_runtime_gate_reemits_before_accepting_a_bc_produced_object(tmp_path: Path) -> None:
    """A forged producer label and matching hash once could feed BC output to the qbopt gate."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    manifest = _write_frontend_manifest(namespace, tmp_path, "numeric")
    data = __import__("json").loads(manifest.read_text())
    obj = tmp_path / "Q45N01.OBJ"
    obj.write_bytes((ROOT / "fixtures/omf/arith-q-O.obj".lower()).read_bytes())
    data["cases"][0]["objects"][0]["sha256"] = __import__("hashlib").sha256(obj.read_bytes()).hexdigest()
    manifest.write_text(__import__("json").dumps(data))

    # Metadata and the declared hash agree; only deterministic re-emission
    # establishes that this is not the oracle compiler's unrelated object.
    namespace["_frontend_manifest"](manifest, "qb45")
    with pytest.raises(ValueError, match="BC-produced, stale, or mismatched OBJ refused"):
        namespace["run_frontend_cases"]("qb45", manifest, output=tmp_path / "runtime")


def test_unique_runtime_directory_does_not_depend_on_host_timestamp_precision(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A fresh DOS RESULT.TXT once looked stale because its timestamp was coarser than time_ns."""
    from types import SimpleNamespace

    from tools import dosbox

    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    manifest = _write_frontend_manifest(namespace, tmp_path, "numeric")
    runtime_globals = namespace["run_frontend_cases"].__globals__
    runtime_globals["_runtime_toolchain"] = lambda _profile: SimpleNamespace(link=r"V:\BIN\LINK.EXE", mount=tmp_path)
    runtime_globals["_reemit_frontend_objects"] = lambda _case: {"Q45N01.OBJ": (tmp_path / "Q45N01.OBJ").read_bytes()}

    def launch(workdir: Path, _mount: Path, _lines: list[str], **_kwargs: object) -> SimpleNamespace:
        (workdir / "LINK.TXT").write_text("Microsoft LINK: no errors\r\n")
        (workdir / "PROGRAM.EXE").write_bytes(b"fresh executable")
        result = workdir / "RESULT.TXT"
        result.write_text("PASS numeric\r\n")
        __import__("os").utime(result, ns=(1, 1))
        return SimpleNamespace(finished=True, timed_out=False)

    monkeypatch.setattr(dosbox, "launch", launch)
    receipt = namespace["run_frontend_cases"]("qb45", manifest, output=tmp_path / "runtime")
    record = __import__("json").loads(receipt.read_text())["cases"][0]

    assert record["link"] == "passed"
    assert record["execution"] == "passed"
    assert record["verdict"] == "passed"


def test_runtime_cli_fails_instead_of_claiming_an_unimplemented_pass() -> None:
    """Object emission and artifact bytes are not a linked DOS runtime verdict."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")

    with pytest.raises(SystemExit, match="2"):
        namespace["main"](["qb45", "--run-frontend"])


def test_verdict_channel_rejects_open_con_and_file_handle_print() -> None:
    """OPEN CON bypassed COMMAND.COM redirection and made every verdict invisible."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")

    namespace["_validate_verdict_channel"]('print "PASS sample"', "sample", "required")
    with pytest.raises(ValueError, match='must not use OPEN "CON"'):
        namespace["_validate_verdict_channel"](
            'open "CON" for output as #1\nprint #1, "PASS sample"',
            "sample",
            "required",
        )
    with pytest.raises(ValueError, match="must use bare PRINT"):
        namespace["_validate_verdict_channel"]('print #1, "PASS sample"', "sample", "required")


def test_rem_is_reserved_for_the_explicit_comment_equivalence_witness() -> None:
    """Ordinary REM comments obscured the corpus-wide apostrophe convention."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")

    namespace["_validate_comment_style"]("' ordinary comment", "sample", False)
    with pytest.raises(ValueError, match="apostrophe"):
        namespace["_validate_comment_style"]("REM ordinary comment", "sample", False)
    namespace["_validate_comment_style"]("REM hidden = 1", "witness", True)


def test_alternate_math_cannot_silently_link_the_default_runtime() -> None:
    """A /FPa object linked to BCL71ENR aborted before main but looked buildable."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    required = {"switches": ["/O", "/FPa"], "runtime": "required"}

    with pytest.raises(ValueError, match="explicit runtime_library"):
        namespace["_validate_runtime_library"](required, "pds-fpa")
    required["runtime_library"] = "BCL71ANR.LIB"
    namespace["_validate_runtime_library"](required, "pds-fpa")
    required["runtime_library"] = "TOO-LONG-RUNTIME.LIB"
    with pytest.raises(ValueError, match="8.3 DOS basename"):
        namespace["_validate_runtime_library"](required, "pds-fpa")
    required["runtime_library"] = "BCL71ANR.LIB"
    required["link_libraries"] = ["NOT-A-DOS-LIBRARY.LIB"]
    with pytest.raises(ValueError, match="link library"):
        namespace["_validate_runtime_library"](required, "pds-fpa")


def test_compiler_library_switch_cannot_disappear_before_link() -> None:
    """INTERRUPT emitted cleanly but LINK never received /L VBDOS.LIB."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    required = {
        "switches": ["/O", "/L VBDOS.LIB"],
        "runtime": "required",
        "link_libraries": [],
    }

    with pytest.raises(ValueError, match="explicit link_libraries"):
        namespace["_validate_runtime_library"](required, "interrupt_abi")
    required["link_libraries"] = ["VBDOS.LIB"]
    namespace["_validate_runtime_library"](required, "interrupt_abi")


def test_emission_manifest_pins_linker_and_dos_process_inputs() -> None:
    """A frontend OBJ without its runtime, extra libraries, or environment is not runnable provenance."""
    namespace = __import__("runpy").run_path(ROOT / "tools/qbcompat.py")
    all_cases = namespace["cases"]("vbdos")
    environment = next(case for case in all_cases if case.name == "environment")
    command_line = next(case for case in all_cases if case.name == "command-line")
    pds_fpa = next(case for case in all_cases if case.name == "pds-fpa")
    financial = next(case for case in all_cases if case.name == "financial-functions")

    assert namespace["_runtime_recipe"](environment) == {
        "runtime_library": "BCOM45.LIB",
        "link_libraries": [],
        "switches": [],
        "arguments": [],
        "environment": {"QBCOMPAT": "Padding8"},
    }
    assert namespace["_runtime_recipe"](pds_fpa)["runtime_library"] == "BCL71ANR.LIB"
    assert namespace["_runtime_recipe"](financial)["link_libraries"] == ["FINANCE.LIB"]
    assert namespace["_runtime_recipe"](command_line)["arguments"] == ["Alpha", "Beta42"]

    with pytest.raises(ValueError, match="environment name"):
        namespace["_validate_execution_inputs"]({"environment": {"BAD=NAME": "value"}}, "bad-environment")


def test_error_resume_cases_require_exact_error_line_and_post_resume_witness() -> None:
    """ERL was once impossible in QB and VBDOS RESUME once jumped into FAIL."""
    qb = (ROOT / "frontends/qb/compat/qb45/q45r35.bas").read_text(encoding="latin1")
    vb = (ROOT / "frontends/qb/compat/vbdos/locerr.bas").read_text(encoding="latin1")

    assert "100 error 11" in qb
    assert "if err <> 11 then" in qb
    assert "if erl <> 100 then" in qb
    assert qb.index("100 error 11") < qb.index("resumed = 1") < qb.index('"PASS resume"')
    assert "resume next" in qb

    assert "100 error 53" in vb
    assert "if err <> 53 then" in vb
    assert "if erl <> 100 then" in vb
    assert "resume 200" in vb and "200 ' recovery target" in vb
    assert vb.index("200 ' recovery target") < vb.index("recovery_count = recovery_count + 1")
    assert vb.index("call recover_locally") < vb.index('"PASS local_error"')
