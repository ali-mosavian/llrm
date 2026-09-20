"""Build a provenance-recorded, direct-QB qrender correctness gate.

This is deliberately a qrender-sized runner, not a project build framework.
It accepts one clean, explicitly pinned qrender tree; emits *all* QB modules
once through ``tools.qbproject``; and then proves that the two links consumed
only those objects.  The older BC-object rewrite evidence is a different
route and must not be mixed into this receipt.
"""

from __future__ import annotations

import os
import re
import json
import math
import shutil
import hashlib
import argparse
import tempfile
import subprocess
from typing import Any
from pathlib import Path
from dataclasses import dataclass
from collections.abc import Mapping
from collections.abc import Callable
from collections.abc import Sequence

from tools import qbproject
from tools import qbfootprint

ORACLE_MODULES = frozenset({"qglchk", "qgldiff", "qglarr", "qglface"})
ORACLE_FLAGS = {
    "qglcheck": "-qglcheck",
    "qgldiff": "-qgldiff",
    "qglarr": "-qglarr",
    "qglface": "-lm -nostats -yaw 183 -bench 1 -qglface",
}
RUNTIME_ARTIFACTS = frozenset(
    {
        "bench.bmp",
        "bench.txt",
        "ran.txt",
        "error.log",
        "run.out",
        "next.bat",
        "qglchk.log",
        "qgldiff.log",
        "qglarr.log",
        "qglface.log",
    }
)
PAYLOAD_SUFFIXES = frozenset({".obj", ".exe", ".map", ".rsp", ".conf", ".out"})
LINK_ERROR = re.compile(r"unresolved external|error l[0-9]+", re.IGNORECASE)
SEVERE_ERROR = re.compile(r"(?<!0 )\b[1-9][0-9]* severe +errors?\b", re.IGNORECASE)
REVISION = re.compile(r"[0-9a-f]{40}\Z")


@dataclass(frozen=True, slots=True)
class Project:
    source_dirs: tuple[Path, ...]
    modules: tuple[Path, ...]
    include_dirs: tuple[Path, ...]
    production: tuple[str, ...]
    oracle: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class Options:
    source_root: Path
    revision: str
    output: Path
    jobs: int = 4
    timeout: int = 600
    debug_info: bool = True


@dataclass(frozen=True, slots=True)
class Process:
    returncode: int
    stdout: str = ""
    stderr: str = ""


Runner = Callable[[Sequence[str], Path, Mapping[str, str], int], Process]
Emitter = Callable[[Path, str, Path, tuple[Path, ...], tuple[Path, ...], qbproject.Options], dict[str, Any]]


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _relative(root: Path, path: Path) -> str:
    return path.resolve().relative_to(root.resolve()).as_posix()


def _run(command: Sequence[str], cwd: Path, env: Mapping[str, str], timeout: int) -> Process:
    completed = subprocess.run(
        tuple(command), cwd=cwd, env={**os.environ, **env}, capture_output=True, text=True, timeout=timeout, check=False
    )
    return Process(completed.returncode, completed.stdout, completed.stderr)


def _checked(runner: Runner, command: Sequence[str], cwd: Path, env: Mapping[str, str], timeout: int) -> Process:
    result = runner(command, cwd, env, timeout)
    if result.returncode:
        detail = (result.stderr or result.stdout).strip()
        raise ValueError(f"command failed ({result.returncode}): {' '.join(command)}{': ' + detail if detail else ''}")
    return result


def _git(runner: Runner, root: Path, *arguments: str) -> str:
    return _checked(runner, ("git", "-C", str(root), *arguments), root, {}, 30).stdout.strip()


def _source_dirs(root: Path) -> tuple[Path, ...]:
    makefile = root / "Makefile"
    if not makefile.is_file():
        raise ValueError("qrender source root has no Makefile")
    match = re.search(r"^SRC_DIRS\s*:?=\s*(.+)$", makefile.read_text(), re.MULTILINE)
    if match is None:
        raise ValueError("Makefile does not declare SRC_DIRS")
    dirs = tuple((root / item).resolve() for item in match.group(1).split())
    if not dirs or any(not directory.is_dir() for directory in dirs):
        raise ValueError("Makefile SRC_DIRS contains a missing source directory")
    return dirs


def _casefold_unique(paths: Sequence[Path], *, kind: str) -> None:
    seen: dict[str, Path] = {}
    for path in paths:
        key = path.stem.casefold()
        if previous := seen.get(key):
            raise ValueError(f"{kind} casefold collision: {previous.name}, {path.name}")
        seen[key] = path


def discover(root: Path) -> Project:
    """Read qrender's current source layout without duplicating its module list."""
    root = root.resolve()
    dirs = _source_dirs(root)
    grouped = tuple(tuple(sorted(directory.glob("*.bas"), key=lambda path: path.name.casefold())) for directory in dirs)
    absolute_modules = tuple(path for group in grouped for path in group)
    _casefold_unique(absolute_modules, kind="BASIC module")
    names = {path.stem.casefold() for path in absolute_modules}
    if "main" not in names:
        raise ValueError("qrender BASIC modules do not include main")
    includes = tuple(directory.relative_to(root) for directory in dirs if any(directory.rglob("*.bi")))
    ordered_modules = tuple(path for path in absolute_modules if path.stem.casefold() == "main") + tuple(
        path for path in absolute_modules if path.stem.casefold() != "main"
    )
    ordered = tuple(path.stem.casefold() for path in ordered_modules)
    production = tuple(name for name in ordered if name not in ORACLE_MODULES)
    oracle = tuple(name for name in ordered if name != "qglstub")
    if "qglstub" not in names or not names >= ORACLE_MODULES:
        raise ValueError("qrender source does not contain its qglstub/oracle module split")
    return Project(
        tuple(directory.relative_to(root) for directory in dirs),
        tuple(path.relative_to(root) for path in ordered_modules),
        includes,
        production,
        oracle,
    )


def _case_path(directory: Path, name: str) -> Path | None:
    found = [path for path in directory.iterdir() if path.name.casefold() == name.casefold()]
    if len(found) > 1:
        raise ValueError(f"casefold collision in {directory.name}: {name}")
    return found[0] if found else None


def _read_case(directory: Path, name: str) -> Path:
    path = _case_path(directory, name)
    if path is None or not path.is_file():
        raise ValueError(f"missing required artifact {directory.name}/{name}")
    return path


def _output_root(path: Path) -> Path:
    if path.exists() and (not path.is_dir() or any(path.iterdir())):
        raise ValueError("evidence output root must be absent or empty")
    path.mkdir(parents=True, exist_ok=True)
    return path.resolve()


def _source_identity(runner: Runner, root: Path, revision: str) -> dict[str, str]:
    if not REVISION.fullmatch(revision):
        raise ValueError("--revision must be a full 40-character git revision")
    head = _git(runner, root, "rev-parse", "HEAD")
    if head != revision:
        raise ValueError(f"source HEAD is {head}, expected {revision}")
    dirty = _git(runner, root, "status", "--porcelain", "--", "Makefile", "src", "tools", "dosbox", "data")
    if dirty:
        raise ValueError("qrender source/build/tool paths are modified or untracked")
    return {"revision": head, "tree": _git(runner, root, "rev-parse", "HEAD^{tree}")}


def _write_receipt(root: Path, receipt: dict[str, Any]) -> None:
    target = root / "qrender-gate.json"
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(dir=root, prefix=".qrender-gate-", delete=False) as temporary:
            temporary_path = Path(temporary.name)
            temporary.write((json.dumps(receipt, indent=2, sort_keys=True) + "\n").encode())
        os.replace(temporary_path, target)
    except BaseException:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)
        raise


def _source_relative(source: Path, path: Path) -> Path:
    if not path.is_absolute():
        return path
    return path.resolve().relative_to(source.resolve())


def _source_relative_paths(source: Path, paths: Sequence[Path]) -> tuple[Path, ...]:
    return tuple(_source_relative(source, path) for path in paths)


def _command(command: Sequence[str], *, source: Path, root: Path) -> list[str]:
    """Store a replayable command without leaking host-specific paths."""
    result: list[str] = []
    source_text, root_text = str(source), str(root)
    for item in command:
        if item == source_text or item.startswith(source_text + os.sep):
            result.append("{source}" + item.removeprefix(source_text).replace(os.sep, "/"))
        elif item == root_text or item.startswith(root_text + os.sep):
            result.append("{output}" + item.removeprefix(root_text).replace(os.sep, "/"))
        else:
            result.append(item.replace(source_text, "{source}").replace(root_text, "{output}"))
    return result


def _build_baseline(
    runner: Runner, source: Path, evidence: Path, build: Path, options: Options, *, oracles: bool
) -> dict[str, Any]:
    command = [
        "make",
        "-C",
        str(source),
        "build",
        f"BUILD={build}",
        f"DEBUGINFO={int(options.debug_info)}",
        f"-j{options.jobs}",
    ]
    if oracles:
        command.append("ORACLES=1")
    result = _checked(runner, command, source, {"TIMEOUT": str(options.timeout)}, options.timeout)
    (build / "gate-build.stdout").write_text(result.stdout)
    (build / "gate-build.stderr").write_text(result.stderr)
    build_log = result.stdout + result.stderr
    if SEVERE_ERROR.search(build_log):
        raise ValueError("baseline compiler reported severe errors")
    link = _read_case(build, "link.out")
    if LINK_ERROR.search(link.read_text(errors="replace")):
        raise ValueError("baseline link reported errors")
    exe = _read_case(build, "qrender.exe")
    return {
        "command": _command(command, source=source, root=evidence),
        "build_log_sha256": hashlib.sha256(build_log.encode()).hexdigest(),
        "link": _artifact(build, link),
        "exe": _artifact(build, exe),
    }


def _manifest_objects(root: Path, manifest: Mapping[str, Any], project: Project, revision: str) -> dict[str, Path]:
    if manifest.get("schema") != 1 or manifest.get("producer") != "qbopt.frontend.qb":
        raise ValueError("frontend manifest has an unknown producer or schema")
    if manifest.get("source_revision") != revision:
        raise ValueError("frontend manifest revision does not match the pinned source")
    outputs = manifest.get("outputs")
    if not isinstance(outputs, list):
        raise ValueError("frontend manifest has no outputs")
    objects: dict[str, Path] = {}
    for item in outputs:
        if (
            not isinstance(item, dict)
            or not isinstance(item.get("path"), str)
            or not isinstance(item.get("sha256"), str)
        ):
            raise ValueError("frontend manifest has an invalid output record")
        path = (root / item["path"]).resolve()
        if root not in path.parents or not path.is_file():
            raise ValueError("frontend output escapes or is missing from the object root")
        name = path.stem.casefold()
        if name in objects:
            raise ValueError(f"frontend manifest has colliding output {path.name}")
        if _sha256(path) != item["sha256"]:
            raise ValueError(f"frontend object changed after emission: {path.name}")
        objects[name] = path
    expected = {path.stem.casefold() for path in project.modules}
    if set(objects) != expected:
        missing, extra = sorted(expected - set(objects)), sorted(set(objects) - expected)
        raise ValueError(f"frontend manifest BASIC outputs differ; missing={missing}, extra={extra}")
    return objects


def _overwrite_basic(build: Path, names: Sequence[str], objects: Mapping[str, Path]) -> None:
    for name in names:
        source = objects.get(name)
        if source is None:
            raise ValueError(f"frontend manifest misses linked BASIC module {name}")
        destination = build / f"{name}.obj"
        shutil.copyfile(source, destination)


def _assert_fresh_basic(build: Path, names: Sequence[str], objects: Mapping[str, Path]) -> None:
    for name in names:
        expected = objects.get(name)
        actual = _read_case(build, f"{name}.obj")
        if expected is None or _sha256(actual) != _sha256(expected):
            raise ValueError(f"linked BASIC object is stale or missing frontend provenance: {name}.obj")


def _response_objects(path: Path) -> list[str]:
    text = path.read_text(encoding="latin1")
    object_list = text.split("qrender.exe", 1)[0]
    return [
        match.group(1).casefold()
        for match in re.finditer(r"(?:^|[+\s])([^+\s]+\.obj)(?=[+\s]|$)", object_list, re.IGNORECASE)
    ]


def _nonbasic_from_response(path: Path, all_basic: set[str]) -> tuple[str, ...]:
    objects = _response_objects(path)
    if not objects:
        raise ValueError("baseline link.rsp has no object list")
    return tuple(Path(item).stem for item in objects if Path(item).stem.casefold() not in all_basic)


def _nonbasic_manifest(build: Path, names: Sequence[str]) -> list[dict[str, Any]]:
    records = []
    for name in names:
        path = _read_case(build, f"{name}.obj")
        records.append({"name": name.casefold(), **_artifact(build, path)})
    return records


def _assert_nonbasic_copy(build: Path, expected: Sequence[Mapping[str, Any]]) -> list[dict[str, Any]]:
    actual = _nonbasic_manifest(build, tuple(str(record["name"]) for record in expected))
    if actual != list(expected):
        raise ValueError("candidate non-BASIC object differs from its fresh baseline copy")
    return actual


def _seed_oracle(baseline: Path, oracle_baseline: Path, nonbasic: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    """Keep the fresh production objects and their mtimes for incremental ORACLES=1."""
    shutil.copytree(baseline, oracle_baseline, copy_function=shutil.copy2)
    return {
        "from": "baseline",
        "preserve_mtimes": True,
        "nonbasic_objects": _assert_nonbasic_copy(oracle_baseline, nonbasic),
    }


def basic_order_from_response(response: Path, all_basic: set[str], excluded: set[str]) -> tuple[str, ...]:
    """Keep Makefile/LINK order while proving its selected BASIC set is exact."""
    basic = tuple(
        Path(item).stem.casefold() for item in _response_objects(response) if Path(item).stem.casefold() in all_basic
    )
    expected = all_basic - excluded
    if not basic or basic[0] != "main":
        raise ValueError("baseline link response does not name main first")
    if len(basic) != len(set(basic)):
        raise ValueError("baseline link response names a BASIC module more than once")
    if set(basic) != expected:
        missing, extra = sorted(expected - set(basic)), sorted(set(basic) - expected)
        raise ValueError(f"baseline link BASIC set differs; missing={missing}, extra={extra}")
    return basic


def assert_response_coverage(response: Path, expected: Sequence[str], excluded: set[str]) -> None:
    objects = [Path(item).stem.casefold() for item in _response_objects(response)]
    basic = [name for name in objects if name in set(expected) | excluded]
    if not basic or basic[0] != "main":
        raise ValueError("link response does not name main first")
    if basic != list(expected):
        raise ValueError(f"link response BASIC coverage differs: {basic}")
    if any(name in excluded for name in basic):
        raise ValueError("link response names a BASIC module excluded from this link")
    if len(basic) != len(set(basic)):
        raise ValueError("link response names a BASIC module more than once")


def _link(
    runner: Runner,
    source: Path,
    build: Path,
    evidence: Path,
    names: tuple[str, ...],
    all_basic: set[str],
    nonbasic: tuple[str, ...],
    options: Options,
) -> dict[str, Any]:
    command = (str(source / "tools/link-qr.sh"), str(build), " ".join(names), " ".join(nonbasic))
    _checked(
        runner,
        command,
        source,
        {"TIMEOUT": str(options.timeout), "DEBUGINFO": str(int(options.debug_info))},
        options.timeout,
    )
    response = _read_case(build, "link.rsp")
    assert_response_coverage(response, names, all_basic - set(names))
    link = _read_case(build, "link.out")
    if LINK_ERROR.search(link.read_text(errors="replace")):
        raise ValueError("candidate link reported errors")
    exe = _read_case(build, "qrender.exe")
    return {
        "command": _command(command, source=source, root=evidence),
        "rsp": _artifact(build, response),
        "link": _artifact(build, link),
        "exe": _artifact(build, exe),
    }


def _clear_runtime(build: Path) -> None:
    for child in build.iterdir():
        if child.name.casefold() in RUNTIME_ARTIFACTS:
            child.unlink()


def _runtime_file(build: Path, name: str) -> Path | None:
    return _case_path(build, name)


def _bench_fields(path: Path) -> dict[str, str]:
    fields: dict[str, str] = {}
    for line in path.read_text(encoding="latin1").replace("\r", "").splitlines():
        words = line.split()
        if len(words) >= 2:
            fields[words[0].casefold()] = words[1]
    return fields


def _bench_rows(path: Path) -> dict[str, tuple[str, ...]]:
    rows: dict[str, tuple[str, ...]] = {}
    for line in path.read_text(encoding="latin1").replace("\r", "").splitlines():
        words = tuple(line.split())
        if words:
            rows[words[0].casefold()] = words
    return rows


def _float_value(value: str, *, field: str) -> float:
    try:
        parsed = float(value)
    except ValueError as error:
        raise ValueError(f"benchmark field {field} is not numeric: {value!r}") from error
    if not math.isfinite(parsed):
        raise ValueError(f"benchmark field {field} is not finite: {value!r}")
    return parsed


def _validate_bench(path: Path) -> dict[str, str]:
    fields, rows = _bench_fields(path), _bench_rows(path)
    if fields.get("ticks") != "60" or fields.get("sc_test") != "1":
        raise ValueError("benchmark fields do not establish ticks=60 and sc_test=1")
    for name, row in rows.items():
        if not name.startswith("pt_") or len(row) < 4:
            continue
        minimum, mean, maximum = (_float_value(value, field=name) for value in row[1:4])
        if mean < minimum - 0.001 or mean > maximum + 0.001:
            raise ValueError(f"benchmark {name} mean is outside min..max")
    if _float_value(fields.get("fp_sites", "0"), field="fp_sites") <= 0:
        raise ValueError("benchmark fp_sites is not positive")
    if _float_value(fields.get("mdl_bf_bad", "nan"), field="mdl_bf_bad") != 0:
        raise ValueError("benchmark mdl_bf_bad is not zero")
    qn, qnc = rows.get("pt_q_n"), rows.get("pt_q_noclip")
    if qn is not None and qnc is not None:
        if len(qn) < 3 or len(qnc) < 3:
            raise ValueError("benchmark pt_q_n rows lack mean values")
        total = _float_value(qn[2], field="pt_q_n")
        noclip = _float_value(qnc[2], field="pt_q_noclip")
        if noclip < 0 or noclip > total:
            raise ValueError("benchmark pt_q_noclip is outside 0..pt_q_n")
    return fields


def _run_bench(runner: Runner, source: Path, evidence: Path, build: Path, options: Options) -> dict[str, Any]:
    _clear_runtime(build)
    command = (str(source / "tools/dosbox.sh"), "run", "dm3ish.bsp")
    environment = {
        "VBD_OUT": str(build),
        "QFLAGS": "-lm -nostats -yaw 183 -bench 40 -ticks 60",
        "TIMEOUT": str(options.timeout),
        "CORE": "dynamic",
        "CYCLES": "75000",
    }
    _checked(runner, command, source, environment, options.timeout)
    ran = _runtime_file(build, "ran.txt")
    if ran is None or ran.read_text(encoding="latin1").strip().casefold() != "done":
        raise ValueError("runtime did not write a fresh RAN.TXT DONE marker")
    error = _runtime_file(build, "error.log")
    if error is not None:
        raise ValueError("runtime wrote ERROR.LOG")
    image = _runtime_file(build, "bench.bmp")
    bench = _runtime_file(build, "bench.txt")
    if image is None or image.stat().st_size <= 1000 or bench is None:
        raise ValueError("runtime did not write a nontrivial BENCH.BMP and BENCH.TXT")
    fields = _validate_bench(bench)
    return {
        "command": _command(command, source=source, root=evidence),
        "environment": {"QFLAGS": environment["QFLAGS"], "CORE": "dynamic", "CYCLES": "75000"},
        "ran": _artifact(build, ran),
        "bench": _artifact(build, bench),
        "bmp": _artifact(build, image),
        "fields": fields,
    }


def _run_oracle(
    runner: Runner, source: Path, evidence: Path, build: Path, flag: str, options: Options, *, require_pass: bool
) -> dict[str, Any]:
    _clear_runtime(build)
    command = (str(source / "tools/dosbox.sh"), "run", "dm3ish.bsp")
    environment = {
        "VBD_OUT": str(build),
        "QFLAGS": ORACLE_FLAGS[flag],
        "TIMEOUT": str(options.timeout),
        "CORE": "dynamic",
        "CYCLES": "75000",
    }
    _checked(runner, command, source, environment, options.timeout)
    ran = _runtime_file(build, "ran.txt")
    if ran is None or ran.read_text(encoding="latin1").strip().casefold() != "done":
        raise ValueError(f"-{flag} did not write a fresh RAN.TXT DONE marker")
    if _runtime_file(build, "error.log") is not None:
        raise ValueError(f"-{flag} wrote ERROR.LOG")
    log = (
        _read_case(build, f"{flag.replace('check', 'chk')}.log")
        if flag == "qglcheck"
        else _read_case(build, f"{flag}.log")
    )
    result = log.read_text(encoding="latin1").replace("\r", "").strip().splitlines()
    final = result[-1].strip() if result else ""
    if require_pass and final != "RESULT PASS":
        raise ValueError(f"-{flag} did not finish RESULT PASS")
    if not require_pass and "FAIL" not in final:
        raise ValueError("known qglface failure stopped being an explicit FAIL")
    return {
        "flag": flag,
        "command": _command(command, source=source, root=evidence),
        "environment": {"QFLAGS": environment["QFLAGS"], "CORE": "dynamic", "CYCLES": "75000"},
        "log": _artifact(build, log),
        "final": final,
        "status": "passed" if require_pass else "known-failure",
    }


def _payload(build: Path) -> list[dict[str, Any]]:
    """Hash the non-code files staged beside the EXE, in deterministic order."""
    paths = [
        path
        for path in build.rglob("*")
        if path.is_file()
        and path.suffix.casefold() not in PAYLOAD_SUFFIXES
        and path.name.casefold() not in RUNTIME_ARTIFACTS
        and not path.name.startswith("gate-build.")
    ]
    return [_artifact(build, path) for path in sorted(paths, key=lambda path: _relative(build, path).casefold())]


def _runtime_payload(source: Path, baseline: Path, candidate: Path) -> dict[str, Any]:
    map_path = source / "data/dm3ish.bsp"
    if not map_path.is_file():
        raise ValueError("qrender source has no data/dm3ish.bsp runtime input")
    baseline_payload, candidate_payload = _payload(baseline), _payload(candidate)
    if baseline_payload != candidate_payload:
        raise ValueError("candidate staged runtime payload differs from the fresh baseline")
    return {
        "map_input": {"path": "data/dm3ish.bsp", "sha256": _sha256(map_path), "size": map_path.stat().st_size},
        "baseline": baseline_payload,
        "candidate": candidate_payload,
        "identical": True,
    }


def _source_artifact(source: Path, relative: str) -> dict[str, Any]:
    path = source / relative
    if not path.is_file():
        raise ValueError(f"qrender source misses required gate tool {relative}")
    return {"path": relative, "sha256": _sha256(path), "size": path.stat().st_size}


def _artifact(root: Path, path: Path) -> dict[str, Any]:
    return {"path": _relative(root, path), "sha256": _sha256(path), "size": path.stat().st_size}


def _quality(baseline: Path, candidate: Path) -> dict[str, Any]:
    baseline_map, candidate_map = _read_case(baseline, "qrender.map"), _read_case(candidate, "qrender.map")
    rows, (basic_baseline, basic_candidate) = qbfootprint.compare_maps(baseline_map, candidate_map)
    complete_baseline, complete_candidate = qbfootprint.linked_code_totals(baseline_map, candidate_map)
    if basic_candidate > basic_baseline or complete_candidate > complete_baseline:
        raise ValueError("candidate generated-code footprint regressed")
    return {
        "baseline_map": _artifact(baseline, baseline_map),
        "candidate_map": _artifact(candidate, candidate_map),
        "basic": {"baseline": basic_baseline, "candidate": basic_candidate},
        "complete": {"baseline": complete_baseline, "candidate": complete_candidate},
        "rows": rows,
    }


def run(options: Options, *, runner: Runner = _run, emitter: Emitter = qbproject.emit_project) -> Path:
    """Execute the all-fresh route and leave a receipt even after a failure."""
    source, root = options.source_root.resolve(), _output_root(options.output)
    receipt: dict[str, Any] = {
        "schema": 1,
        "producer": "qbopt.qrender-gate",
        "phase": "prepare",
        "status": "running",
        "options": {"jobs": options.jobs, "timeout": options.timeout, "debug_info": options.debug_info},
    }
    _write_receipt(root, receipt)
    try:
        identity = _source_identity(runner, source, options.revision)
        project = discover(source)
        receipt["source"] = {
            **identity,
            "tools": [
                _source_artifact(source, relative)
                for relative in ("Makefile", "tools/bc.sh", "tools/bcc-qr.sh", "tools/link-qr.sh", "tools/dosbox.sh")
            ],
        }
        receipt["project"] = {
            "source_dirs": [path.as_posix() for path in project.source_dirs],
            "modules": [path.as_posix() for path in project.modules],
            "include_dirs": [path.as_posix() for path in project.include_dirs],
            "production": list(project.production),
            "oracle": list(project.oracle),
        }
        receipt["phase"] = "baseline"
        baseline = root / "baseline"
        baseline.mkdir()
        receipt["baseline"] = _build_baseline(runner, source, root, baseline, options, oracles=False)

        receipt["phase"] = "frontend"
        objects_root = root / "frontend-objects"
        manifest = emitter(
            source,
            options.revision,
            objects_root,
            _source_relative_paths(source, project.modules),
            _source_relative_paths(source, project.include_dirs),
            qbproject.Options("vbdos", "vbdos", "row-major"),
        )
        objects = _manifest_objects(objects_root, manifest, project, options.revision)
        receipt["frontend"] = {
            "command": {
                "tool": "tools/qbproject.py",
                "output": "frontend-objects",
                "dialect": "vbdos",
                "runtime": "vbdos",
                "array_order": "row-major",
            },
            "manifest": _artifact(root, objects_root / "frontend-manifest.json"),
            "emitter_sha256": manifest["emitter_sha256"],
            "options": manifest["options"],
            "inputs": [*manifest["modules"], *manifest["include_files"]],
            "objects": [_artifact(root, objects[name]) for name in sorted(objects)],
        }

        all_basic = {path.stem.casefold() for path in project.modules}
        baseline_response = _read_case(baseline, "link.rsp")
        production = basic_order_from_response(baseline_response, all_basic, set(ORACLE_MODULES))
        nonbasic = _nonbasic_from_response(baseline_response, all_basic)
        baseline_nonbasic = _nonbasic_manifest(baseline, nonbasic)
        receipt["baseline"]["nonbasic_objects"] = baseline_nonbasic
        receipt["phase"] = "candidate-link"
        candidate = root / "candidate"
        shutil.copytree(baseline, candidate)
        candidate_nonbasic_before = _assert_nonbasic_copy(candidate, baseline_nonbasic)
        _overwrite_basic(candidate, production, objects)
        _assert_fresh_basic(candidate, production, objects)
        receipt["candidate"] = _link(runner, source, candidate, root, production, all_basic, nonbasic, options)
        _assert_fresh_basic(candidate, production, objects)
        candidate_nonbasic_after = _assert_nonbasic_copy(candidate, baseline_nonbasic)
        receipt["candidate"]["nonbasic_objects"] = {
            "before_link": candidate_nonbasic_before,
            "after_link": candidate_nonbasic_after,
        }

        receipt["runtime_payload"] = _runtime_payload(source, baseline, candidate)

        receipt["phase"] = "benchmark"
        baseline_run = _run_bench(runner, source, root, baseline, options)
        candidate_run = _run_bench(runner, source, root, candidate, options)
        reference = _source_artifact(source, "tools/ref/bench.bmp")
        if baseline_run["bmp"]["sha256"] != candidate_run["bmp"]["sha256"]:
            raise ValueError("candidate BENCH.BMP differs from fresh baseline")
        if baseline_run["bmp"]["sha256"] != reference["sha256"]:
            raise ValueError("fresh baseline BENCH.BMP differs from pinned reference")
        if candidate_run["bmp"]["sha256"] != reference["sha256"]:
            raise ValueError("candidate BENCH.BMP differs from pinned reference")
        receipt["benchmark"] = {
            "baseline": baseline_run,
            "candidate": candidate_run,
            "reference": reference,
            "bmp_identical": True,
        }

        receipt["phase"] = "oracle-link"
        oracle_baseline = root / "oracle" / "baseline"
        oracle_baseline.parent.mkdir()
        receipt["oracle_seed"] = _seed_oracle(baseline, oracle_baseline, baseline_nonbasic)
        receipt["oracle_baseline"] = _build_baseline(runner, source, root, oracle_baseline, options, oracles=True)
        oracle_response = _read_case(oracle_baseline, "link.rsp")
        oracle = basic_order_from_response(oracle_response, all_basic, {"qglstub"})
        oracle_nonbasic = _nonbasic_from_response(oracle_response, all_basic)
        oracle_baseline_nonbasic = _nonbasic_manifest(oracle_baseline, oracle_nonbasic)
        receipt["oracle_baseline"]["nonbasic_objects"] = oracle_baseline_nonbasic
        oracle_candidate = root / "oracle" / "candidate"
        shutil.copytree(oracle_baseline, oracle_candidate)
        oracle_candidate_nonbasic_before = _assert_nonbasic_copy(oracle_candidate, oracle_baseline_nonbasic)
        _overwrite_basic(oracle_candidate, oracle, objects)
        _assert_fresh_basic(oracle_candidate, oracle, objects)
        receipt["oracle_candidate"] = _link(
            runner, source, oracle_candidate, root, oracle, all_basic, oracle_nonbasic, options
        )
        _assert_fresh_basic(oracle_candidate, oracle, objects)
        oracle_candidate_nonbasic_after = _assert_nonbasic_copy(oracle_candidate, oracle_baseline_nonbasic)
        receipt["oracle_candidate"]["nonbasic_objects"] = {
            "before_link": oracle_candidate_nonbasic_before,
            "after_link": oracle_candidate_nonbasic_after,
        }

        receipt["phase"] = "oracles"
        passed = []
        for flag in ("qglcheck", "qgldiff", "qglarr"):
            oracle_baseline_run = _run_oracle(runner, source, root, oracle_baseline, flag, options, require_pass=True)
            oracle_candidate_run = _run_oracle(runner, source, root, oracle_candidate, flag, options, require_pass=True)
            if oracle_baseline_run["log"]["sha256"] != oracle_candidate_run["log"]["sha256"]:
                raise ValueError(f"-{flag} log differs from fresh baseline")
            passed.append(
                {"flag": flag, "baseline": oracle_baseline_run, "candidate": oracle_candidate_run, "identical": True}
            )
        face_baseline = _run_oracle(runner, source, root, oracle_baseline, "qglface", options, require_pass=False)
        face_candidate = _run_oracle(runner, source, root, oracle_candidate, "qglface", options, require_pass=False)
        if face_baseline["log"]["sha256"] != face_candidate["log"]["sha256"]:
            raise ValueError("qglface known-failure log differs from fresh baseline")
        receipt["oracles"] = {
            "passed": passed,
            "qglface": {"baseline": face_baseline, "candidate": face_candidate, "identical": True},
        }

        receipt["phase"] = "quality"
        receipt["quality"] = _quality(baseline, candidate)
        receipt["status"] = "passed"
    except BaseException as error:
        receipt["status"] = "failed"
        receipt["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        _write_receipt(root, receipt)
    return root / "qrender-gate.json"


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--revision", required=True, help="full, clean qrender git revision")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--timeout", type=int, default=600)
    parser.add_argument("--no-debug-info", action="store_false", dest="debug_info")
    args = parser.parse_args(arguments)
    if args.jobs < 1 or args.timeout < 1:
        parser.error("--jobs and --timeout must be positive")
    try:
        print(run(Options(args.source_root, args.revision, args.output, args.jobs, args.timeout, args.debug_info)))
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
