"""Emit a fresh, provenance-recorded set of QB source modules."""

from __future__ import annotations

import os
import json
import hashlib
import argparse
import tempfile
import subprocess
from pathlib import Path
from dataclasses import asdict
from dataclasses import dataclass

from qbopt.frontend.qb import driver
from qbopt.frontend.qb import compile as qb_compile

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True, slots=True)
class Options:
    dialect: str
    runtime: str
    array_order: str
    huge_arrays: bool = False
    checked_arrays: bool = False
    mbf: bool = False
    alternate_math: bool = False


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _input_record(root: Path, path: Path) -> dict[str, str]:
    return {
        "path": path.relative_to(root).as_posix(),
        "sha256": _sha256(path),
    }


def emitter_sha256() -> str:
    paths = [
        REPOSITORY_ROOT / "Cargo.toml",
        REPOSITORY_ROOT / "Cargo.lock",
        REPOSITORY_ROOT / "build.rs",
        REPOSITORY_ROOT / "src" / "bin" / "qbfront.rs",
        *(REPOSITORY_ROOT / "src" / "frontend" / "qb").rglob("*.rs"),
        *(REPOSITORY_ROOT / "src" / "frontend" / "qb" / "grammar").rglob("*"),
        *(REPOSITORY_ROOT / "tools" / "buildprs").rglob("*"),
        *(REPOSITORY_ROOT / "qbopt").rglob("*.py"),
    ]
    digest = hashlib.sha256()
    for path in sorted(paths):
        if not path.is_file():
            continue
        digest.update(path.relative_to(REPOSITORY_ROOT).as_posix().encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
    return digest.hexdigest()


def _git_revision(root: Path) -> str:
    result = subprocess.run(
        ("git", "-C", str(root), "rev-parse", "HEAD"),
        capture_output=True,
        check=False,
        text=True,
    )
    if result.returncode:
        message = result.stderr.strip() or f"git rev-parse exited with status {result.returncode}"
        raise ValueError(message)
    return result.stdout.strip()


def _assert_clean_inputs(root: Path, paths: tuple[Path, ...]) -> None:
    result = subprocess.run(
        (
            "git",
            "-C",
            str(root),
            "status",
            "--porcelain",
            "--",
            *(path.relative_to(root).as_posix() for path in paths),
        ),
        capture_output=True,
        check=False,
        text=True,
    )
    if result.returncode:
        message = result.stderr.strip() or f"git status exited with status {result.returncode}"
        raise ValueError(message)
    if result.stdout.strip():
        raise ValueError("requested module or include inputs are modified or untracked")


def _relative_path(root: Path, value: Path, *, kind: str) -> Path:
    if value.is_absolute():
        raise ValueError(f"{kind} must be relative to the source root: {value}")
    path = (root / value).resolve()
    if path != root and root not in path.parents:
        raise ValueError(f"{kind} escapes the source root: {value}")
    if not path.exists():
        raise ValueError(f"{kind} does not exist: {value}")
    return path


def _sources(root: Path, modules: tuple[Path, ...]) -> tuple[Path, ...]:
    if not modules:
        raise ValueError("at least one module is required")
    found = tuple(
        sorted(
            (_relative_path(root, path, kind="module") for path in modules),
            key=lambda path: path.relative_to(root).as_posix().casefold(),
        )
    )
    if any(not path.is_file() for path in found):
        raise ValueError("module paths must name files")
    basenames: dict[str, Path] = {}
    for source in found:
        key = source.stem.casefold()
        if previous := basenames.get(key):
            raise ValueError(f"module basenames collide as DOS objects: {previous.name}, {source.name}")
        basenames[key] = source
    return found


def _include_dirs(root: Path, includes: tuple[Path, ...]) -> tuple[Path, ...]:
    found = tuple(_relative_path(root, path, kind="include directory") for path in includes)
    if any(not path.is_dir() for path in found):
        raise ValueError("include paths must name directories")
    return found


def _include_files(root: Path, include_dirs: tuple[Path, ...]) -> tuple[Path, ...]:
    found: list[Path] = []
    for directory in include_dirs:
        for path in directory.rglob("*"):
            if not path.is_file() or path.suffix.casefold() != ".bi":
                continue
            resolved = path.resolve()
            if resolved != root and root not in resolved.parents:
                raise ValueError(f"include file escapes the source root: {path}")
            found.append(resolved)
    return tuple(sorted(found, key=lambda path: path.relative_to(root).as_posix().casefold()))


def _prepare_output(output: Path) -> Path:
    if output.exists() and not output.is_dir():
        raise ValueError(f"output is not a directory: {output}")
    if output.exists() and any(output.iterdir()):
        raise ValueError(f"output directory must be absent or empty: {output}")
    output.mkdir(parents=True, exist_ok=True)
    return output.resolve()


def _atomic_write(path: Path, data: bytes) -> None:
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix=".qbproject-", delete=False) as temporary:
            temporary_path = Path(temporary.name)
            temporary.write(data)
        assert temporary_path is not None
        os.replace(temporary_path, path)
    except BaseException:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)
        raise


def emit_project(
    source_root: Path,
    expected_revision: str,
    output: Path,
    modules: tuple[Path, ...],
    includes: tuple[Path, ...],
    options: Options,
) -> dict:
    root = source_root.resolve()
    if not root.is_dir():
        raise ValueError(f"source root is not a directory: {source_root}")
    sources = _sources(root, modules)
    include_dirs = _include_dirs(root, includes)
    include_files = _include_files(root, include_dirs)
    if (revision := _git_revision(root)) != expected_revision:
        raise ValueError(f"source revision is {revision}, expected {expected_revision}")
    _assert_clean_inputs(root, sources + include_files)
    output = _prepare_output(output)

    compiler_options = {
        **asdict(options),
        "include_dirs": include_dirs,
    }
    emitted: list[Path] = []
    previous_producer = os.environ.get("QBOPT_QBFRONT")
    try:
        os.environ["QBOPT_QBFRONT"] = str(driver.build_release())
        for source in sources:
            program = driver.parsed(source, **compiler_options)
            object_path = output / f"{source.stem.lower()}.obj"
            _atomic_write(object_path, qb_compile.object_bytes(program, source.name))
            emitted.append(object_path)
        manifest = {
            "schema": 1,
            "producer": "qbopt.frontend.qb",
            "source_revision": revision,
            "emitter_sha256": emitter_sha256(),
            "options": asdict(options),
            "modules": [_input_record(root, source) for source in sources],
            "include_dirs": [directory.relative_to(root).as_posix() for directory in include_dirs],
            "include_files": [_input_record(root, path) for path in include_files],
            "outputs": [
                {
                    "path": path.relative_to(output).as_posix(),
                    "sha256": _sha256(path),
                    "size": path.stat().st_size,
                }
                for path in emitted
            ],
        }
        _atomic_write(
            output / "frontend-manifest.json", (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
        )
        return manifest
    except BaseException:
        for path in emitted:
            path.unlink(missing_ok=True)
        (output / "frontend-manifest.json").unlink(missing_ok=True)
        raise
    finally:
        if previous_producer is None:
            os.environ.pop("QBOPT_QBFRONT", None)
        else:
            os.environ["QBOPT_QBFRONT"] = previous_producer


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="qbproject", description=__doc__)
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--expected-revision", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--module", required=True, action="append", type=Path)
    parser.add_argument("--include", action="append", default=[], type=Path)
    parser.add_argument("--dialect", default="vbdos")
    parser.add_argument("--runtime", default="vbdos")
    parser.add_argument("--array-order", choices=("column-major", "row-major"), default="column-major")
    parser.add_argument("--huge-arrays", action="store_true")
    parser.add_argument("--checked-arrays", action="store_true")
    parser.add_argument("--mbf", action="store_true")
    parser.add_argument("--alternate-math", action="store_true")
    arguments = parser.parse_args(arguments)
    manifest = emit_project(
        arguments.source_root,
        arguments.expected_revision,
        arguments.output,
        tuple(arguments.module),
        tuple(arguments.include),
        Options(
            dialect=arguments.dialect,
            runtime=arguments.runtime,
            array_order=arguments.array_order,
            huge_arrays=arguments.huge_arrays,
            checked_arrays=arguments.checked_arrays,
            mbf=arguments.mbf,
            alternate_math=arguments.alternate_math,
        ),
    )
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
