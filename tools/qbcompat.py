"""Validate and enumerate the additive QB/PDS/VBDOS source suites."""

from __future__ import annotations

import re
import json
import time
import shutil
import hashlib
import tomllib
import argparse
from pathlib import Path
from collections import Counter
from dataclasses import dataclass

ROOT = Path(__file__).resolve().parents[1]
SUITES = ROOT / "frontends" / "qb" / "compat"
PROFILES = {
    "qb45": SUITES / "qb45" / "suite.toml",
    "pds71": SUITES / "pds71" / "suite.toml",
    "vbdos": SUITES / "vbdos" / "suite.toml",
}
STATUS = {"required", "pending", "reference-only"}
TOPIC_STATUS = {"covered", "required-reference", "out-of-scope"}
TOPIC_CLASS = {"language", "runtime", "compiler", "ide", "tooling"}
DOS_NAME = re.compile(r"^[^./\\]{1,8}(?:\.[^./\\]{1,3})?$")
RUN_MARKER = ".qbcompat-run.json"
DEFAULT_RUNTIME_LIBRARIES = {
    "qb45": "BCOM45.LIB",
    "pds71": "BCL71ENR.LIB",
    "vbdos10": "VBDCL10E.LIB",
}


@dataclass(frozen=True, slots=True)
class Case:
    profile: str
    directory: Path
    values: dict

    @property
    def name(self) -> str:
        return self.values["name"]

    @property
    def source(self) -> Path:
        return self.directory / self.values["source"]

    def expected(self) -> str:
        value = self.values["expected"]
        sidecar = self.directory / value
        return sidecar.read_text(encoding="latin1").strip() if sidecar.is_file() else value


def _manifest(path: Path, seen: frozenset[Path] = frozenset()) -> list[Case]:
    path = path.resolve()
    if path in seen:
        raise ValueError(f"cyclic compatibility-suite inheritance at {path}")
    data = tomllib.loads(path.read_text())
    if data.get("schema") != 1:
        raise ValueError(f"{path}: unsupported schema {data.get('schema')!r}")
    inherited: list[Case] = []
    if parent := data.get("inherits"):
        inherited = _manifest(path.parent / parent, seen | {path})
    profile = data["dialect"]
    return inherited + [Case(profile, path.parent, one) for one in data.get("case", [])]


def _validate_dos_text(path: Path, case_name: str) -> None:
    data = path.read_bytes()
    without_crlf = data.replace(b"\r\n", b"")
    if b"\r" in without_crlf or b"\n" in without_crlf:
        raise ValueError(f"{case_name}: DOS text must use CRLF exclusively: {path}")


def _validate_verdict_channel(source: str, case_name: str, runtime: str) -> None:
    """Verdicts must be capturable by COMMAND.COM output redirection."""
    if re.search(r'open\s+"con"\s+for\s+output', source, re.IGNORECASE):
        raise ValueError(f'{case_name}: verdict must not use OPEN "CON"')
    if re.search(
        r'^\s*print\s+#.*["\'](?:pass|fail|reference only)\b',
        source,
        re.IGNORECASE | re.MULTILINE,
    ):
        raise ValueError(f"{case_name}: PASS/FAIL verdict must use bare PRINT")
    if runtime != "reference-only" and not re.search(
        r'^\s*print\s+(?!#).*?["\']pass\b', source, re.IGNORECASE | re.MULTILINE
    ):
        raise ValueError(f"{case_name}: executable case has no redirected bare PRINT PASS")


def _validate_comment_style(source: str, case_name: str, allow_rem_witness: bool) -> None:
    rem = re.search(r"^\s*(?:\d+\s+)?rem(?:\s|$)", source, re.IGNORECASE | re.MULTILINE)
    if rem and not allow_rem_witness:
        raise ValueError(f"{case_name}: ordinary comments must use apostrophe, not REM")


def _validate_runtime_library(values: dict, case_name: str) -> None:
    """Keep compiler modes which select a non-default ABI tied to their linker input."""
    library = values.get("runtime_library")
    raw_switches = values.get("switches", [])
    switches = {switch.casefold() for switch in raw_switches}
    if "/fpa" in switches and values.get("runtime") == "required" and not library:
        raise ValueError(f"{case_name}: /FPa runtime requires an explicit runtime_library")
    if library and (Path(library).name != library or not DOS_NAME.fullmatch(library)):
        raise ValueError(f"{case_name}: runtime_library is not an 8.3 DOS basename: {library!r}")
    extras = values.get("link_libraries", [])
    for extra in extras:
        if Path(extra).name != extra or not DOS_NAME.fullmatch(extra):
            raise ValueError(f"{case_name}: link library is not an 8.3 DOS basename: {extra!r}")
    selected = {
        match.group(1).upper()
        for switch in raw_switches
        if (match := re.fullmatch(r"/L\s+([^\s]+)", switch, re.IGNORECASE)) is not None
    }
    missing = selected - {extra.upper() for extra in extras}
    if missing:
        raise ValueError(f"{case_name}: compiler /L library needs explicit link_libraries input: {sorted(missing)}")


def _validate_execution_inputs(values: dict, case_name: str) -> None:
    """Reject DOS process inputs a future runner could silently drop or reinterpret."""
    environment = values.get("environment", {})
    if not isinstance(environment, dict):
        raise ValueError(f"{case_name}: environment must be a TOML table")
    for key, value in environment.items():
        if not isinstance(key, str) or not re.fullmatch(r"[A-Za-z][A-Za-z0-9_]{0,30}", key):
            raise ValueError(f"{case_name}: invalid DOS environment name {key!r}")
        if not isinstance(value, str) or any(character in value for character in "\0\r\n"):
            raise ValueError(f"{case_name}: invalid DOS environment value for {key!r}")
    arguments = values.get("arguments", [])
    if not isinstance(arguments, list) or not all(isinstance(item, str) for item in arguments):
        raise ValueError(f"{case_name}: arguments must be a list of strings")
    if any(any(character in item for character in "\0\r\n") for item in arguments):
        raise ValueError(f"{case_name}: DOS argument contains a line break or NUL")


def _runtime_recipe(case: Case) -> dict:
    """Pin every linker and DOS-process input beside an emitted frontend object."""
    values = case.values
    return {
        "runtime_library": values.get("runtime_library", DEFAULT_RUNTIME_LIBRARIES[case.profile]),
        "link_libraries": list(values.get("link_libraries", [])),
        "switches": list(values.get("switches", [])),
        "arguments": list(values.get("arguments", [])),
        "environment": dict(sorted(values.get("environment", {}).items())),
    }


def _source_inputs(case: Case) -> list[dict[str, str]]:
    paths = [
        case.source,
        *(case.directory / name for name in case.values.get("companions", [])),
    ]
    return [{"name": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()} for path in paths]


def cases(profile: str) -> list[Case]:
    found = _manifest(PROFILES[profile])
    names: set[str] = set()
    for case in found:
        if case.name in names:
            raise ValueError(f"duplicate inherited case name {case.name!r}")
        names.add(case.name)
    return found


def validate(profile: str) -> list[Case]:
    found = cases(profile)
    if not found or found[0].name != "text-screen-capture":
        raise ValueError(f"{profile}: first inherited case must prove text-screen capture")
    for case in found:
        values = case.values
        if not case.source.is_file():
            raise ValueError(f"{case.name}: missing source {case.source}")
        if not 1 <= values["level"] <= 5:
            raise ValueError(f"{case.name}: level must be 1..5")
        for stage in ("parser", "lowering", "runtime"):
            if values[stage] not in STATUS:
                raise ValueError(f"{case.name}: invalid {stage} status {values[stage]!r}")
        _validate_runtime_library(values, case.name)
        _validate_execution_inputs(values, case.name)
        expected = case.expected()
        if not expected:
            raise ValueError(f"{case.name}: empty expected output")
        sources = [case.source, *(case.directory / name for name in values.get("companions", []))]
        for source_path in sources:
            if not source_path.is_file():
                raise ValueError(f"{case.name}: missing companion source {source_path}")
            if not DOS_NAME.fullmatch(source_path.name):
                raise ValueError(f"{case.name}: DOS source is not an 8.3 name: {source_path.name!r}")
            _validate_dos_text(source_path, case.name)
        expected_path = case.directory / values["expected"]
        if expected_path.is_file():
            if not DOS_NAME.fullmatch(expected_path.name):
                raise ValueError(f"{case.name}: DOS expected file is not an 8.3 name: {expected_path.name!r}")
            _validate_dos_text(expected_path, case.name)
        source = "\n".join(path.read_text(encoding="latin1").lower() for path in sources)
        _validate_verdict_channel(source, case.name, values["runtime"])
        _validate_comment_style(source, case.name, values.get("allow_rem_witness", False))
        if values["runtime"] == "required":
            if not values.get("verification", "").strip():
                raise ValueError(f"{case.name}: runtime case has no feature-specific verification claim")
            if not re.search(
                r'^\s*print\s+(?!#).*?["\']fail\b',
                source,
                re.IGNORECASE | re.MULTILINE,
            ):
                raise ValueError(f"{case.name}: runtime case has no explicit failing witness")
        for artifact in values.get("artifacts", []):
            if not DOS_NAME.fullmatch(Path(artifact).name):
                raise ValueError(f"{case.name}: artifact is not an 8.3 DOS name: {artifact!r}")
        goldens = values.get("artifact_goldens", [])
        if values.get("artifacts", []) and not goldens:
            raise ValueError(f"{case.name}: artifact is declared without an original-toolchain golden")
        if goldens and len(goldens) != len(values.get("artifacts", [])):
            raise ValueError(f"{case.name}: artifact and golden counts differ")
        for golden in goldens:
            golden_path = case.directory / golden
            if not golden_path.is_file():
                raise ValueError(f"{case.name}: missing measured artifact golden {golden_path}")
    return found


def _artifact_plan(profile: str) -> tuple[list[tuple[Case, str, Path]], str]:
    plan: list[tuple[Case, str, Path]] = []
    digest = hashlib.sha256()
    for case in validate(profile):
        artifacts = case.values.get("artifacts", [])
        goldens = case.values.get("artifact_goldens", [])
        for artifact, golden in zip(artifacts, goldens, strict=True):
            golden_path = case.directory / golden
            plan.append((case, artifact, golden_path))
            digest.update(case.name.encode())
            digest.update(b"\0")
            digest.update(artifact.encode())
            digest.update(b"\0")
            digest.update(hashlib.sha256(golden_path.read_bytes()).digest())
    if not plan:
        raise ValueError(f"{profile}: no measured artifact goldens are declared")
    return plan, digest.hexdigest()


def frontend_emitter_sha256() -> str:
    """Identify the in-tree source path that emits QB objects, not an oracle BC."""
    paths = [
        ROOT / "qbopt/frontend/qb/compile.py",
        ROOT / "qbopt/frontend/qb/driver.py",
        ROOT / "frontends/qb/Cargo.lock",
        *(ROOT / "frontends/qb/src").glob("*.rs"),
    ]
    digest = hashlib.sha256()
    for path in sorted(paths):
        digest.update(str(path.relative_to(ROOT)).encode())
        digest.update(b"\0")
        digest.update(hashlib.sha256(path.read_bytes()).digest())
    return digest.hexdigest()


def _frontend_manifest(frontend_manifest: Path, profile: str) -> tuple[dict, str]:
    try:
        data = json.loads(frontend_manifest.read_text())
    except (json.JSONDecodeError, OSError) as error:
        raise ValueError(f"{profile}: invalid qb-frontend emission manifest {frontend_manifest}") from error
    if data.get("schema") != 1 or data.get("producer") != "qbopt.frontend.qb":
        raise ValueError(f"{profile}: manifest is not from the qb frontend")
    if data.get("profile") != profile:
        raise ValueError(f"{profile}: qb-frontend manifest selects {data.get('profile')!r}")
    if data.get("emitter_sha256") != frontend_emitter_sha256():
        raise ValueError(f"{profile}: stale or non-qb-frontend emitter provenance")
    emitted = [item for item in data.get("cases", []) if item.get("status") == "emitted"]
    if not emitted:
        raise ValueError(f"{profile}: qb-frontend manifest contains no emitted objects")
    known = {case.name: case for case in cases(profile)}
    for item in emitted:
        case = known.get(item.get("name"))
        if case is None:
            raise ValueError(f"{profile}: qb-frontend manifest names an unknown case")
        expected_metadata = {
            "dialect": case.profile,
            "expected": case.expected(),
            "inputs": _source_inputs(case),
            "runtime_recipe": _runtime_recipe(case),
        }
        for key, value in expected_metadata.items():
            if item.get(key) != value:
                raise ValueError(f"{profile}: qb-frontend manifest has stale or mismatched {key} for {case.name}")
        if not item.get("objects"):
            raise ValueError(f"{profile}: emitted case has no qb-frontend objects: {case.name}")
        for obj in item.get("objects", []):
            relative = Path(obj["path"])
            root = frontend_manifest.parent.resolve()
            path = (root / relative).resolve()
            if relative.is_absolute() or path != root and root not in path.parents:
                raise ValueError(f"{profile}: emitted qb-frontend object escapes its manifest directory")
            if not path.is_file():
                raise ValueError(f"{profile}: emitted qb-frontend object is missing: {path}")
            actual = hashlib.sha256(path.read_bytes()).hexdigest()
            if actual != obj.get("sha256"):
                raise ValueError(f"{profile}: emitted qb-frontend object changed: {path}")
    digest = hashlib.sha256(frontend_manifest.read_bytes()).hexdigest()
    return data, digest


def _runtime_toolchain(profile: str):
    """Select LINK for a case's own inherited Microsoft runtime profile."""
    from tools.configs import CONFIGS

    tag = {"qb45": "q-O", "pds71": "p-g2", "vbdos10": "v-g3"}[profile]
    config = CONFIGS[tag]
    if not config.available:
        raise ValueError(f"{profile}: Microsoft runtime toolchain is unavailable at {config.mount}")
    return config


def _link_line(case: Case, objects: list[str]) -> str:
    """Build LINK's response line without invoking an oracle compiler."""
    if not objects or len(objects) != len(set(objects)):
        raise ValueError(f"{case.name}: emitted object names are empty or collide in DOS")
    recipe = _runtime_recipe(case)
    libraries = [recipe["runtime_library"], *recipe["link_libraries"]]
    library_field = "+".join(rf"V:\LIB\{name}" for name in libraries)
    return "+".join(objects) + rf",PROGRAM.EXE,PROGRAM.MAP,{library_field}; > LINK.TXT"


def _reemit_frontend_objects(case: Case) -> dict[str, bytes]:
    """Reproduce a manifest's objects from its pinned source with this frontend.

    A producer label and an object hash prove only that a manifest and object
    agree; they do not prove Microsoft BC was not substituted for our emitter.
    Deterministic reproduction is the provenance boundary used by the runtime
    gate, immediately before any object reaches LINK.
    """
    from qbopt.frontend.qb import parsed
    from qbopt.frontend.qb import compile as qb_compile

    dialects = {"qb45": "qb45", "pds71": "pds71", "vbdos10": "vbdos"}
    runtimes = {"qb45": "qb45", "pds71": "pds71", "vbdos10": "vbdos"}
    sources = [
        case.source,
        *(case.directory / name for name in case.values.get("companions", []) if Path(name).suffix.lower() == ".bas"),
    ]
    return {
        f"{source.stem}.OBJ".upper(): qb_compile.object_bytes(
            parsed(
                source,
                dialect=dialects[case.profile],
                runtime=runtimes[case.profile],
                array_order=("row-major" if "/R" in case.values.get("switches", []) else "column-major"),
                huge_arrays="/Ah" in case.values.get("switches", []),
                checked_arrays="/D" in case.values.get("switches", []),
                mbf="/MBF" in case.values.get("switches", []),
                alternate_math="/FPa" in case.values.get("switches", []),
                include_dirs=(case.directory,),
            ),
            source.name,
        )
        for source in sources
    }


def run_frontend_cases(
    profile: str,
    frontend_manifest: Path,
    output: Path | None = None,
) -> Path:
    """Link and run only hash-verified objects emitted by our QB frontend.

    Microsoft LINK and the selected runtime are consumers of the fresh OMF;
    BC is never invoked.  Each case receives a new directory, so an old EXE,
    verdict, or artifact cannot satisfy this run.
    """
    from tools.dosbox import launch
    from tools.dosbox import dos_file
    from tools.dosbox import read_dos

    manifest, manifest_hash = _frontend_manifest(frontend_manifest, profile)
    known = {case.name: case for case in validate(profile)}
    root = output or frontend_manifest.parent / "dos-run"
    run = root / f"run-{time.time_ns()}"
    run.mkdir(parents=True, exist_ok=False)
    results: list[dict] = []
    for item in manifest["cases"]:
        if item.get("status") != "emitted":
            continue
        case = known[item["name"]]
        case_dir = run / f"{case.profile}-{case.source.stem}"
        case_dir.mkdir()
        reproduced = _reemit_frontend_objects(case)
        declared_names = {Path(obj["path"]).name.upper() for obj in item["objects"]}
        if declared_names != set(reproduced):
            raise ValueError(f"{case.name}: emitted object set does not match deterministic qb-frontend output")
        object_names: list[str] = []
        object_hashes: list[dict[str, str]] = []
        for obj in item["objects"]:
            source = (frontend_manifest.parent / obj["path"]).resolve()
            name = source.name.upper()
            if not DOS_NAME.fullmatch(name) or Path(name).suffix.upper() != ".OBJ":
                raise ValueError(f"{case.name}: emitted object is not a DOS .OBJ: {name}")
            if name in object_names:
                raise ValueError(f"{case.name}: emitted object names collide in DOS: {name}")
            if source.read_bytes() != reproduced[name]:
                raise ValueError(
                    f"{case.name}: emitted object does not reproduce from the pinned source; "
                    "BC-produced, stale, or mismatched OBJ refused"
                )
            shutil.copyfile(source, case_dir / name)
            object_names.append(name)
            object_hashes.append({"name": name, "sha256": obj["sha256"]})

        config = _runtime_toolchain(case.profile)
        arguments = item["runtime_recipe"]["arguments"]
        command = "PROGRAM.EXE" + (" " + " ".join(arguments) if arguments else "")
        artifact_instrument = bool(case.values.get("artifacts"))
        lines = [rf"{config.link} {_link_line(case, object_names)}"]
        lines.append(f"if exist PROGRAM.EXE {command}" + ("" if artifact_instrument else " > RESULT.TXT"))
        executed = launch(
            case_dir,
            config.mount,
            lines,
            timeout=60,
            env=item["runtime_recipe"]["environment"],
        )
        link_output = read_dos(case_dir, "LINK.TXT")
        exe = dos_file(case_dir, "PROGRAM.EXE")
        record = {
            "name": case.name,
            "dialect": case.profile,
            "objects": object_hashes,
            "directory": str(case_dir.relative_to(run)),
            "frontend_stages": item.get("stages", {}),
            "reproduction": "passed",
            "link": "failed",
            "execution": "not-run",
            "verdict": "not-checked",
        }
        link_error = re.search(
            r"(?:fatal\s+error|error\s+L\d+|unresolved\s+external|severe\s+error)",
            link_output,
            re.IGNORECASE,
        )
        if not executed.finished or executed.timed_out:
            record["error"] = "DOSBox did not complete the link/run batch"
            results.append(record)
            continue
        if exe is None or link_error:
            record["error"] = "Microsoft LINK rejected the qb-frontend object"
            results.append(record)
            continue
        record["link"] = "passed"

        artifacts_ok = True
        for artifact, golden in zip(
            case.values.get("artifacts", []),
            case.values.get("artifact_goldens", []),
            strict=True,
        ):
            actual = dos_file(case_dir, artifact)
            expected = case.directory / golden
            if actual is None or actual.read_bytes() != expected.read_bytes():
                artifacts_ok = False
                record["error"] = f"fresh artifact mismatch or absence: {artifact}"
                break
        if not artifacts_ok:
            record["execution"] = "failed"
            record["verdict"] = "artifact-failed"
            results.append(record)
            continue

        record["execution"] = "passed"
        if artifact_instrument:
            # Q45L00 saves B800 before PRINT and deliberately runs without
            # redirection. Its byte golden proves screen behavior, not the
            # COMMAND.COM verdict channel.
            record["verdict"] = "artifact-passed-unredirected"
        else:
            result = dos_file(case_dir, "RESULT.TXT")
            actual = read_dos(case_dir, "RESULT.TXT").strip("\r\n")
            if result is None:
                record["execution"] = "failed"
                record["verdict"] = "missing"
                record["error"] = "fresh redirected verdict was not produced"
            elif actual != case.expected():
                record["execution"] = "failed"
                record["verdict"] = "failed"
                record["actual"] = actual
                record["error"] = "redirected verdict differs from the manifest expectation"
            else:
                record["verdict"] = "passed"
        results.append(record)

    receipt = run / "runtime-manifest.json"
    receipt.write_text(
        json.dumps(
            {
                "schema": 1,
                "producer": "qbopt.frontend.qb",
                "profile": profile,
                "frontend_manifest_sha256": manifest_hash,
                "cases": results,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    return receipt


def prepare_artifact_run(profile: str, run_directory: Path, frontend_manifest: Path) -> Path:
    """Open a run window bound to fresh objects emitted by our QB frontend."""
    plan, plan_hash = _artifact_plan(profile)
    manifest, manifest_hash = _frontend_manifest(frontend_manifest, profile)
    artifact_cases = {case.name for case, _artifact, _golden in plan}
    emitted_cases = {item["name"] for item in manifest["cases"] if item.get("status") == "emitted"}
    if not artifact_cases <= emitted_cases:
        missing = sorted(artifact_cases - emitted_cases)
        raise ValueError(f"{profile}: artifact cases were not emitted by the qb frontend: {missing}")
    run_directory.mkdir(parents=True, exist_ok=True)
    for _case, artifact, _golden in plan:
        actual = run_directory / artifact
        if actual.is_file():
            actual.unlink()
    started_ns = time.time_ns()
    marker = run_directory / RUN_MARKER
    marker.write_text(
        json.dumps(
            {
                "schema": 1,
                "producer": "qbopt.frontend.qb",
                "profile": profile,
                "plan_sha256": plan_hash,
                "frontend_manifest_sha256": manifest_hash,
                "started_ns": started_ns,
            },
            sort_keys=True,
        )
        + "\n"
    )
    return marker


def check_artifacts(profile: str, run_directory: Path) -> list[Path]:
    """Require artifacts from a prepared run window and byte-compare to goldens."""
    plan, plan_hash = _artifact_plan(profile)
    marker = run_directory / RUN_MARKER
    if not marker.is_file():
        raise ValueError(f"{profile}: missing fresh-run marker; prepare the DOS artifact directory first")
    try:
        receipt = json.loads(marker.read_text())
    except (json.JSONDecodeError, OSError) as error:
        raise ValueError(f"{profile}: invalid fresh-run marker {marker}") from error
    expected_receipt = {
        "schema": 1,
        "producer": "qbopt.frontend.qb",
        "profile": profile,
        "plan_sha256": plan_hash,
    }
    for key, value in expected_receipt.items():
        if receipt.get(key) != value:
            raise ValueError(f"{profile}: stale or mismatched fresh-run marker {marker}")
    if not re.fullmatch(r"[0-9a-f]{64}", receipt.get("frontend_manifest_sha256", "")):
        raise ValueError(f"{profile}: fresh-run marker is not bound to qb-frontend objects")
    started_ns = receipt.get("started_ns")
    if not isinstance(started_ns, int) or started_ns <= 0:
        raise ValueError(f"{profile}: invalid run start time in {marker}")

    checked: list[Path] = []
    for case, artifact, expected in plan:
        actual = run_directory / artifact
        if not actual.is_file():
            raise ValueError(f"{case.name}: fresh DOS run did not produce {actual}")
        if actual.stat().st_mtime_ns < started_ns:
            raise ValueError(f"{case.name}: artifact predates the prepared DOS run: {actual}")
        if actual.read_bytes() != expected.read_bytes():
            raise ValueError(f"{case.name}: artifact differs from measured golden: {actual}")
        checked.append(actual)
    return checked


def _evidence_path(directory: Path, evidence: str) -> tuple[Path, int | None]:
    """Resolve ``file:line`` evidence without confusing it with a TOML selector."""
    filename, separator, location = evidence.partition(":")
    path = directory / filename
    line = int(location) if separator and location.isdecimal() else None
    return path, line


def _validate_evidence(directory: Path, evidence: str, label: str) -> None:
    evidence_path, line = _evidence_path(directory, evidence)
    if not evidence_path.is_file():
        raise ValueError(f"{label}: missing evidence file {evidence_path}")
    if line is not None:
        lines = evidence_path.read_text(encoding="latin1").splitlines()
        line_count = len(lines)
        if not 1 <= line <= line_count:
            raise ValueError(f"{label}: evidence line {line} is outside {evidence_path} (1..{line_count})")
        if evidence_path.suffix.casefold() in {".bas", ".inc"}:
            statement = lines[line - 1].strip()
            lowered = statement.casefold()
            control_only = re.fullmatch(
                r"(?:end(?:\s+(?:if|sub|function|type))?|else)",
                lowered,
            )
            if (
                not statement
                or (statement.startswith("'") and not statement.startswith("' $"))
                or re.search(r'^print\s+.*["\'](?:pass|fail|reference only)\b', lowered)
                or control_only
            ):
                raise ValueError(
                    f"{label}: BASIC evidence points at non-semantic line {evidence_path.name}:{line}: {statement!r}"
                )
        return

    _filename, separator, selector = evidence.partition(":")
    if not separator:
        return
    if evidence_path.suffix.lower() != ".toml":
        raise ValueError(f"{label}: selector is only supported for TOML evidence: {evidence}")
    pieces = selector.split(".")
    if len(pieces) != 2 or not all(pieces):
        raise ValueError(f"{label}: invalid TOML evidence selector {selector!r}")
    case_name, field = pieces
    document = tomllib.loads(evidence_path.read_text())
    selected = next((item for item in document.get("case", []) if item.get("name") == case_name), None)
    if selected is None or field not in selected:
        raise ValueError(f"{label}: TOML evidence selector does not resolve: {selector!r}")


def _validate_help_extract(directory: Path, item: dict, coverage_path: Path) -> None:
    extract = directory / item["path"]
    if not DOS_NAME.fullmatch(extract.name):
        raise ValueError(f"{coverage_path}: help extraction is not an 8.3 name: {extract.name!r}")
    if not extract.is_file():
        raise ValueError(f"{coverage_path}: missing help extraction {extract}")
    _validate_dos_text(extract, f"{directory.name} help extraction")
    actual_lines = len(extract.read_bytes().splitlines())
    if actual_lines != item["lines"]:
        raise ValueError(f"{coverage_path}: {extract.name} has {actual_lines} lines, expected {item['lines']}")
    digest = item.get("sha256", "")
    if not re.fullmatch(r"[0-9a-f]{64}", digest):
        raise ValueError(f"{coverage_path}: invalid extraction SHA-256 for {extract.name}")
    actual_digest = hashlib.sha256(extract.read_bytes()).hexdigest()
    if actual_digest != digest:
        raise ValueError(f"{coverage_path}: checked-in extraction hash changed for {extract.name}")


def validate_coverage(profile: str, *, verify_help: bool = True) -> list[dict]:
    """Validate that QuickHelp claims remain attached to real cases and evidence."""
    directory = PROFILES[profile].parent
    path = directory / "coverage.toml"
    data = tomllib.loads(path.read_text())
    if data.get("schema") != 1:
        raise ValueError(f"{path}: unsupported schema {data.get('schema')!r}")

    help_files = data.get("help_files", [])
    if not help_files:
        raise ValueError(f"{path}: no pinned help files")
    for item in help_files:
        source = Path(item["path"])
        digest = item["sha256"]
        if not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise ValueError(f"{path}: invalid SHA-256 for {source}")
        if verify_help:
            if not source.is_file():
                raise ValueError(f"{path}: missing installed help file {source}")
            actual = hashlib.sha256(source.read_bytes()).hexdigest()
            if actual != digest:
                raise ValueError(f"{path}: help-file hash changed for {source}")

    extracts = data.get("help_extracts", [])
    if not extracts:
        raise ValueError(f"{path}: no checked-in help extraction indexes")
    for item in extracts:
        _validate_help_extract(directory, item, path)

    topics = data.get("topic", [])
    if not topics:
        raise ValueError(f"{path}: no help topics")
    known_cases = {case.name for case in cases(profile)}
    identities: set[tuple[str, object]] = set()
    for number, topic in enumerate(topics, 1):
        label = f"{path}: topic {number} ({topic.get('context', '?')})"
        if topic.get("status") not in TOPIC_STATUS:
            raise ValueError(f"{label}: invalid status {topic.get('status')!r}")
        if topic.get("class") not in TOPIC_CLASS:
            raise ValueError(f"{label}: invalid class {topic.get('class')!r}")
        identity = (topic["source"], topic.get("topic_number", topic["context"]))
        if identity in identities:
            raise ValueError(f"{label}: duplicate source topic {identity!r}")
        identities.add(identity)

        mapped = topic.get("cases", [])
        unknown = set(mapped) - known_cases
        if unknown:
            raise ValueError(f"{label}: unknown compatibility cases {sorted(unknown)!r}")
        status = topic["status"]
        if status == "covered" and not mapped:
            raise ValueError(f"{label}: covered topic has no compatibility case")
        mapped_cases = [case for case in cases(profile) if case.name in mapped]
        if status == "covered" and not any(
            case.values["runtime"] == "required" and case.values.get("verification", "").strip()
            for case in mapped_cases
        ):
            raise ValueError(
                f"{label}: covered topic has no executable qb-compiler runtime obligation; "
                "classify it as required-reference"
            )
        if status == "out-of-scope":
            if mapped:
                raise ValueError(f"{label}: out-of-scope topic maps executable cases")
            if not topic.get("rationale"):
                raise ValueError(f"{label}: out-of-scope topic has no rationale")

        evidence = topic.get("evidence")
        if status != "out-of-scope" and not evidence:
            raise ValueError(f"{label}: {status} topic has no evidence pointer")
        if evidence:
            _validate_evidence(directory, evidence, label)

    expected_topics = sum(data.get("help_topic_counts", {}).values())
    if expected_topics and expected_topics != len(topics):
        raise ValueError(f"{path}: inventory has {len(topics)} rows, expected {expected_topics}")
    return topics


def _topic_key(title: str) -> str:
    """Compare versioned help headings without treating presentation suffixes as deltas."""
    value = title.casefold().strip()
    value = re.sub(r"\s+(quickscreen|details|programming examples?|definition)$", "", value)
    value = value.replace("...", " ")
    value = re.sub(r"[^a-z0-9$#]+", " ", value)
    return " ".join(value.split())


def _extract_headings(path: Path) -> list[tuple[int, str]]:
    headings: list[tuple[int, str]] = []
    for line_number, raw in enumerate(path.read_text(encoding="latin1").splitlines(), 1):
        if raw.startswith(":n"):
            headings.append((line_number, raw[2:].strip()))
    return headings


def unclassified_help_obligations(profile: str) -> list[str]:
    """Find FULL-extraction topics absent from the hand-written coverage rows.

    Header topic counts alone previously made a sparse delta inventory look complete.
    Each coverage manifest now selects the exact heading ranges that can contain a
    language/compiler/runtime contract; every unmatched non-ISAM/non-OS/2 heading in
    those ranges is an explicit obligation.  Version-inherited titles are matched
    after removing only QuickHelp's presentation suffixes, not by fuzzy keywords.
    """
    directory = PROFILES[profile].parent
    document = tomllib.loads((directory / "coverage.toml").read_text())
    inventory = document.get("help_inventory", [])
    if not inventory:
        raise ValueError(f"{profile}: coverage has no FULL help inventory ranges")
    excluded: set[str] = set()
    for exclusion in document.get("help_exclusions", []):
        if not exclusion.get("rationale", "").strip():
            raise ValueError(f"{profile}: help exclusion has no rationale: {exclusion}")
        titles = exclusion.get("titles", [])
        if not titles:
            raise ValueError(f"{profile}: help exclusion has no exact titles: {exclusion}")
        excluded.update(_topic_key(title) for title in titles)

    local = Counter(
        (topic["source"], _topic_key(topic["title"])) for topic in validate_coverage(profile, verify_help=False)
    )
    ancestors = {
        "qb45": (),
        "pds71": ("qb45",),
        "vbdos": ("qb45", "pds71"),
    }
    inherited: set[str] = set()
    for ancestor in ancestors[profile]:
        inherited.update(_topic_key(topic["title"]) for topic in validate_coverage(ancestor, verify_help=False))

    gaps: list[str] = []
    matched_excluded: set[str] = set()
    for section in inventory:
        extract = directory / section["path"]
        headings = _extract_headings(extract)
        first = section.get("first_heading", 1)
        count = section["headings"]
        selected = headings[first - 1 : first - 1 + count]
        if len(selected) != count:
            raise ValueError(
                f"{profile}: {extract.name} heading range {first}..{first + count - 1} "
                f"contains only {len(selected)} headings"
            )
        disposition = section["disposition"]
        if disposition == "out-of-scope":
            if not section.get("rationale"):
                raise ValueError(f"{profile}: excluded help range has no rationale: {section}")
            continue
        if disposition != "classify":
            raise ValueError(f"{profile}: invalid help-range disposition {disposition!r}")

        source = section["source"]
        for line_number, title in selected:
            key = _topic_key(title)
            if local[(source, key)]:
                local[(source, key)] -= 1
                continue
            if section.get("inherits_titles", False) and key in inherited:
                continue
            if re.search(r"isam|\bos/2\b", title, re.IGNORECASE):
                continue
            if key in excluded:
                matched_excluded.add(key)
                continue
            gaps.append(f"unclassified {profile}/{extract.name}:{line_number}: {title or '<untitled>'}")
    if unmatched := excluded - matched_excluded:
        raise ValueError(
            f"{profile}: exact help exclusions did not match selected headings: {', '.join(sorted(unmatched))}"
        )
    return gaps


def gap_obligations(profile: str) -> list[str]:
    """List unsupported qb-compiler stages and help rows without executable coverage."""
    gaps: list[str] = []
    for case in validate(profile):
        missing = [stage for stage in ("parser", "lowering", "runtime") if case.values[stage] != "required"]
        if missing:
            detail = ", ".join(f"{stage}={case.values[stage]}" for stage in missing)
            gaps.append(f"case {case.profile}/{case.name}: {detail}")
    coverage_chain = {
        "qb45": ("qb45",),
        "pds71": ("qb45", "pds71"),
        "vbdos": ("qb45", "pds71", "vbdos"),
    }
    for coverage_profile in coverage_chain[profile]:
        for topic in validate_coverage(coverage_profile, verify_help=False):
            if topic["status"] == "required-reference":
                gaps.append(f"help {topic['source']}:{topic['context']}: executable qb-compiler test required")
        gaps.extend(unclassified_help_obligations(coverage_profile))
    return gaps


def emit_frontend_cases(profile: str, output: Path, *, only: frozenset[str] = frozenset()) -> Path:
    """Emit our compiler's objects and adjacent stages; never invoke Microsoft BC."""
    from qbstages import dumped

    from qbopt import hir
    from qbopt.frontend.qb import parsed
    from qbopt.frontend.qb import syntax_checked
    from qbopt.frontend.qb import compile as qb_compile

    dialects = {"qb45": "qb45", "pds71": "pds71", "vbdos10": "vbdos"}
    runtimes = {"qb45": "qb45", "pds71": "pds71", "vbdos10": "vbdos"}
    output.mkdir(parents=True, exist_ok=True)
    records: list[dict] = []
    for case in validate(profile):
        if only and case.name not in only:
            continue
        case_output = output / f"{case.profile}-{case.name}"
        case_output.mkdir(parents=True, exist_ok=True)
        source_paths = [
            case.source,
            *(
                case.directory / name
                for name in case.values.get("companions", [])
                if Path(name).suffix.lower() == ".bas"
            ),
        ]
        record = {
            "name": case.name,
            "dialect": case.profile,
            "expected": case.expected(),
            "inputs": _source_inputs(case),
            "runtime_recipe": _runtime_recipe(case),
            "objects": [],
            "stages": {
                "parse": "not-run",
                "hir": "not-run",
                "mir": "not-run",
                "optimized_mir": "not-run",
                "object": "not-run" if case.values["runtime"] == "required" else "not-required",
            },
            "status": "compile-failed",
        }
        active_stage = "parse"
        try:
            options = {
                "dialect": dialects[case.profile],
                "runtime": runtimes[case.profile],
                "array_order": ("row-major" if "/R" in case.values.get("switches", []) else "column-major"),
                "huge_arrays": "/Ah" in case.values.get("switches", []),
                "checked_arrays": "/D" in case.values.get("switches", []),
                "mbf": "/MBF" in case.values.get("switches", []),
                "alternate_math": "/FPa" in case.values.get("switches", []),
                "include_dirs": (case.directory,),
            }
            for source in source_paths:
                syntax_checked(source, **options)
            record["stages"]["parse"] = "passed"

            active_stage = "hir"
            programs = []
            for source in source_paths:
                unit_output = case_output / source.stem
                unit_output.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, unit_output / "00-input.bas")
                program = parsed(
                    source,
                    dump=unit_output / "01-hir.json",
                    **options,
                )
                programs.append((source, unit_output, program))
            record["stages"]["hir"] = "passed"

            active_stage = "mir"
            lowered = [(program, hir.lower(program)) for _source, _output, program in programs]
            record["stages"]["mir"] = "passed"
            active_stage = "optimized_mir"
            for program, bodies in lowered:
                functions = tuple(function for module in program.modules for function in module.functions)
                for function, body in zip(functions, bodies, strict=True):
                    qb_compile.optimized(program, function, body)
            record["stages"]["optimized_mir"] = "passed"

            if case.values["runtime"] == "required":
                active_stage = "object"
                for source, unit_output, program in programs:
                    dumped(
                        source,
                        unit_output,
                        dialect=options["dialect"],
                        runtime=options["runtime"],
                        array_order=options["array_order"],
                        huge_arrays=options["huge_arrays"],
                        checked_arrays=options["checked_arrays"],
                        mbf=options["mbf"],
                        alternate_math=options["alternate_math"],
                        includes=options["include_dirs"],
                    )
                    obj = case_output / f"{source.stem}.OBJ"
                    obj.write_bytes(qb_compile.object_bytes(program, source.name))
                    record["objects"].append(
                        {
                            "path": str(obj.relative_to(output)),
                            "sha256": hashlib.sha256(obj.read_bytes()).hexdigest(),
                        }
                    )
                record["stages"]["object"] = "passed"
            record["status"] = "emitted" if case.values["runtime"] == "required" else "parsed-reference"
        except Exception as error:
            if record["stages"][active_stage] == "not-run":
                record["stages"][active_stage] = "failed"
            record["status"] = "compile-failed"
            record["error"] = f"{type(error).__name__}: {error}"
        records.append(record)
    manifest = output / "frontend-manifest.json"
    manifest.write_text(
        json.dumps(
            {
                "schema": 1,
                "producer": "qbopt.frontend.qb",
                "profile": profile,
                "emitter_sha256": frontend_emitter_sha256(),
                "cases": records,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    return manifest


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="qbcompat", description=__doc__)
    parser.add_argument("profile", choices=tuple(PROFILES))
    parser.add_argument("--list", action="store_true")
    parser.add_argument(
        "--gaps",
        action="store_true",
        help="list unsupported compiler stages and help topics lacking executable tests",
    )
    parser.add_argument(
        "--emit-frontend",
        type=Path,
        metavar="OUTPUT_DIRECTORY",
        help="emit our QB frontend's OBJ and adjacent stages; Microsoft BC is not invoked",
    )
    parser.add_argument(
        "--case",
        action="append",
        default=[],
        help="limit --emit-frontend to a named inherited compatibility case",
    )
    artifact_options = parser.add_mutually_exclusive_group()
    artifact_options.add_argument(
        "--artifacts",
        type=Path,
        metavar="DOS_RUN_DIRECTORY",
        help="byte-compare artifacts from an already completed DOS run",
    )
    parser.add_argument(
        "--frontend-manifest",
        type=Path,
        help="manifest from --emit-frontend required to prepare a runtime artifact run",
    )
    parser.add_argument(
        "--run-frontend",
        action="store_true",
        help="link and run hash-verified qb-frontend objects against the selected Microsoft runtime",
    )
    artifact_options.add_argument(
        "--prepare-artifacts",
        type=Path,
        metavar="DOS_RUN_DIRECTORY",
        help="delete known outputs and open a fresh DOS artifact run window",
    )
    options = parser.parse_args(arguments)
    found = validate(options.profile)
    topics = validate_coverage(options.profile)
    if options.run_frontend:
        if options.frontend_manifest is None:
            parser.error("--run-frontend requires --frontend-manifest from --emit-frontend")
        receipt = run_frontend_cases(options.profile, options.frontend_manifest)
        data = json.loads(receipt.read_text())
        failed = [
            item
            for item in data["cases"]
            if item["link"] != "passed"
            or item["execution"] != "passed"
            or item["verdict"] not in {"passed", "artifact-passed-unredirected"}
        ]
        for item in failed:
            print(
                f"FAIL {item['dialect']}/{item['name']}: "
                f"link={item['link']} execution={item['execution']} "
                f"verdict={item['verdict']}: {item.get('error', 'unknown error')}"
            )
        print(f"{options.profile}: qb-frontend runtime manifest {receipt}; {len(failed)} failure(s)")
        return 1 if failed else 0
    if options.emit_frontend:
        requested = frozenset(options.case)
        unknown = requested - {case.name for case in found}
        if unknown:
            parser.error(f"unknown compatibility case(s): {sorted(unknown)}")
        manifest = emit_frontend_cases(options.profile, options.emit_frontend, only=requested)
        data = json.loads(manifest.read_text())
        failed = [item for item in data["cases"] if item["status"] == "compile-failed"]
        for item in failed:
            print(f"FAIL {item['dialect']}/{item['name']}: {item['error']}")
        print(f"{options.profile}: qb-frontend emission manifest {manifest}; {len(failed)} failure(s)")
        return 1 if failed else 0
    if options.prepare_artifacts:
        if options.frontend_manifest is None:
            parser.error("--prepare-artifacts requires --frontend-manifest from --emit-frontend")
        marker = prepare_artifact_run(options.profile, options.prepare_artifacts, options.frontend_manifest)
        print(f"{options.profile}: prepared fresh artifact run at {marker}")
        return 0
    if options.artifacts:
        checked = check_artifacts(options.profile, options.artifacts)
        print(
            f"{options.profile}: {len(checked)} fresh qb-frontend-run artifact(s) match; "
            "console/runtime verdict is not established"
        )
        return 0
    if options.gaps:
        gaps = gap_obligations(options.profile)
        for gap in gaps:
            print(gap)
        print(f"{options.profile}: {len(gaps)} explicit compatibility gap(s)")
        return 1 if gaps else 0
    if options.list:
        for case in found:
            print(
                f"{case.values['level']} {case.profile:<7} {case.name:<28} "
                f"{case.values['parser']}/{case.values['lowering']}/{case.values['runtime']}"
            )
    else:
        counts = {name: sum(one.profile == name for one in found) for name in {one.profile for one in found}}
        detail = ", ".join(f"{name}={count}" for name, count in sorted(counts.items()))
        print(f"{options.profile}: {len(found)} inherited cases valid ({detail}); {len(topics)} help-topic rows valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
