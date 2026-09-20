from __future__ import annotations

import os
import re
import json
import hashlib
import argparse
import tempfile
from typing import Any
from pathlib import Path
from dataclasses import dataclass
from collections.abc import Callable
from collections.abc import Sequence

from tools.dosbox import Run
from tools import qbfootprint
from tools.dosbox import launch
from tools.configs import CONFIGS
from tools.dosbox import read_dos
from tools.dosbox import host_path
from qbopt.frontend.qb import driver
from tools.qbproject import emitter_sha256
from qbopt.frontend.qb import compile as qb_compile

GORILLAS_SHA256 = "9926fc1f50c4b489ec4c1b0da5bd2c497ebf4282b3259c28a835a743e24699f7"
SOURCE_NAME = "GORILLA.BAS"
RECEIPT_NAME = "gorillas-gate.json"
BC_SWITCHES = "/O /E /X /FPi"
ERROR = re.compile(r"unresolved external|error\s+l\d+|fatal error", re.IGNORECASE)
SEVERE = re.compile(r"(?<!0\s)\b[1-9]\d*\s+severe\s+errors?\b", re.IGNORECASE)
ZERO_SEVERE = re.compile(r"\b0\s+severe\s+errors?\b", re.IGNORECASE)
RUNTIME_ERROR = re.compile(r"\b(?:error|overflow|illegal function|division by zero)\b", re.IGNORECASE)


@dataclass(frozen=True, slots=True)
class Options:
    source: Path
    output: Path
    timeout: int = 300


Runner = Callable[[Path, Path, list[str], int], Run]
Emitter = Callable[[Path], tuple[bytes, dict[str, str]]]


def _sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _sha256(path: Path) -> str:
    return _sha256_bytes(path.read_bytes())


def _artifact(root: Path, path: Path) -> dict[str, int | str]:
    return {"path": path.relative_to(root).as_posix(), "sha256": _sha256(path), "size": path.stat().st_size}


def _atomic_write(path: Path, data: bytes) -> None:
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix=".gorillas-", delete=False) as file:
            temporary = Path(file.name)
            file.write(data)
        os.replace(temporary, path)
    except BaseException:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
        raise


def _prepare_output(path: Path) -> Path:
    if path.exists() and (not path.is_dir() or any(path.iterdir())):
        raise ValueError("output directory must be absent or empty")
    path.mkdir(parents=True, exist_ok=True)
    return path.resolve()


def _replace_once(text: str, pattern: str, replacement: str, name: str) -> tuple[str, dict[str, str]]:
    found = list(re.finditer(pattern, text, re.MULTILINE | re.DOTALL))
    if len(found) != 1:
        raise ValueError(f"probe anchor {name!r} matched {len(found)} times")
    before = found[0].group(0)
    after = re.sub(pattern, replacement, text, count=1, flags=re.MULTILINE | re.DOTALL)
    return after, {
        "name": name,
        "before_sha256": _sha256_bytes(before.encode("latin1")),
        "after_sha256": _sha256_bytes(replacement.encode("latin1")),
    }


def derive_probe(original: bytes) -> tuple[bytes, list[dict[str, str]]]:
    text = original.decode("latin1").replace("\r\n", "\n")
    changes: list[dict[str, str]] = []
    edits = (
        (
            "probe declaration",
            r"^DECLARE SUB SparklePause \(\)$",
            "DECLARE SUB SparklePause ()\nDECLARE SUB ProbeReceipt ()",
        ),
        (
            "game inputs",
            r"^SUB GetInputs \(Player1\$, Player2\$, NumGames\)\n.*?^END SUB$",
            (
                'SUB GetInputs (Player1$, Player2$, NumGames)\n  Player1$ = "Player 1"\n'
                '  Player2$ = "Player 2"\n  NumGames = 1\n  gravity# = 9.8\nEND SUB'
            ),
        ),
        (
            "shot inputs",
            r"^FUNCTION GetNum# \(Row, Col\)\n.*?^END FUNCTION$",
            (
                "FUNCTION GetNum# (Row, Col)\n  IF Row = 2 THEN\n    GetNum# = 45\n"
                "  ELSE\n    GetNum# = 1\n  END IF\nEND FUNCTION"
            ),
        ),
        (
            "intro choice",
            r"^  DO WHILE Char\$ = \"\"\n    Char\$ = INKEY\$\n  LOOP$",
            '  Char$ = "P"\n  DO WHILE Char$ = ""\n    Char$ = INKEY$\n  LOOP',
        ),
        ("random seed", r"^    RANDOMIZE \(TIMER\)$", "    RANDOMIZE 1"),
        ("rest", r"^SUB Rest \(t#\)\n.*?^END SUB$", "SUB Rest (t#)\nEND SUB"),
        ("sparkle pause", r"^SUB SparklePause\n.*?^END SUB$", "SUB SparklePause\nEND SUB"),
        (
            "probe call",
            r"^  NEXT i\n\n  SCREEN 0\n  WIDTH 80, 25$",
            "  NEXT i\n\n  CALL ProbeReceipt\n  SCREEN 0\n  WIDTH 80, 25",
        ),
    )
    for name, pattern, replacement in edits:
        text, change = _replace_once(text, pattern, replacement, name)
        changes.append(change)
    appendix = """

SUB ProbeReceipt
  DIM probeX AS INTEGER, probeY AS INTEGER
  DIM probeSum AS LONG, probeValue AS INTEGER
  probeSum = 0
  FOR probeY = 0 TO ScrHeight - 1 STEP 17
    FOR probeX = 0 TO ScrWidth - 1 STEP 19
      probeValue = POINT(probeX, probeY)
      probeSum = (probeSum * 33 + probeValue + probeX + probeY) MOD 32749
    NEXT probeX
  NEXT probeY
  OPEN "GORILLA.OUT" FOR OUTPUT AS #1
  PRINT #1, "GORILLAS-PROBE-1"
  PRINT #1, "mode="; Mode; ";wind="; Wind; ";gorilla1="; GorillaX(1); ","; GorillaY(1)
  PRINT #1, "gorilla2="; GorillaX(2); ","; GorillaY(2); ";screen="; ScrWidth; "x"; ScrHeight
  PRINT #1, "checksum="; probeSum
  PRINT #1, "DONE"
  CLOSE #1
END SUB
"""
    text += appendix
    changes.append(
        {
            "name": "probe receipt",
            "before_sha256": _sha256_bytes(b""),
            "after_sha256": _sha256_bytes(appendix.encode("latin1")),
        }
    )
    return text.replace("\n", "\r\n").encode("latin1"), changes


def _direct_object(source: Path) -> tuple[bytes, dict[str, str]]:
    previous = os.environ.get("QBOPT_QBFRONT")
    try:
        binary = driver.build_release()
        os.environ["QBOPT_QBFRONT"] = str(binary)
        program = driver.parsed(source, dialect="qb45", runtime="qb45", array_order="column-major")
        return qb_compile.object_bytes(program, source.name), {
            "qbfront_sha256": _sha256(binary),
            "emitter_sha256": emitter_sha256(),
        }
    finally:
        if previous is None:
            os.environ.pop("QBOPT_QBFRONT", None)
        else:
            os.environ["QBOPT_QBFRONT"] = previous


def _run_dosbox(work: Path, mount: Path, lines: list[str], timeout: int) -> Run:
    return launch(work, mount, lines, timeout=timeout, env={"LIB": r"V:\\LIB"})


def _commands() -> list[str]:
    return [
        rf"V:\BC.EXE {BC_SWITCHES} {SOURCE_NAME}, BASE.OBJ; > BASEBC.OUT",
        r"V:\LINK.EXE BASE.OBJ, BASE.EXE, BASE.MAP, V:\LIB\BCOM45.LIB; > BASELINK.OUT",
        r"V:\LINK.EXE CAND.OBJ, CAND.EXE, CAND.MAP, V:\LIB\BCOM45.LIB; > CANDLINK.OUT",
        "BASE.EXE > BASE.OUT",
        "copy GORILLA.OUT GORILLAB.OUT > nul",
        "CAND.EXE > CAND.OUT",
        "copy GORILLA.OUT GORILLAQ.OUT > nul",
    ]


def _required(work: Path, names: Sequence[str]) -> list[Path]:
    found: list[Path] = []
    for name in names:
        path = next((candidate for candidate in work.iterdir() if candidate.name.casefold() == name.casefold()), None)
        if path is None or not path.is_file():
            raise ValueError(f"missing fresh artifact {name}")
        found.append(path)
    return found


def _reject_logs(work: Path) -> None:
    for name in ("BASEBC.OUT", "BASELINK.OUT", "CANDLINK.OUT"):
        text = read_dos(work, name)
        if not text:
            raise ValueError(f"missing compiler or linker diagnostic {name}")
        if ERROR.search(text) or (name == "BASEBC.OUT" and SEVERE.search(text)):
            raise ValueError(f"compiler or linker rejected {name}")
        if name == "BASEBC.OUT" and ZERO_SEVERE.search(text) is None:
            raise ValueError("compiler did not report zero severe errors")


def _assert_unchanged(root: Path, records: dict[str, dict[str, int | str]]) -> None:
    for name, expected in records.items():
        actual = _artifact(root, root / str(expected["path"]))
        if actual != expected:
            raise ValueError(f"pre-run artifact changed: {name}")


def _read_probe(path: Path) -> bytes:
    value = path.read_bytes().replace(b"\r\n", b"\n")
    if not value.endswith(b"DONE\n"):
        raise ValueError(f"probe output {path.name} lacks DONE")
    return value


def _reject_runtime(work: Path) -> None:
    for name in ("BASE.OUT", "CAND.OUT"):
        text = read_dos(work, name)
        if RUNTIME_ERROR.search(text):
            raise ValueError(f"runtime rejected {name}")


def _quality(baseline: Path, candidate: Path) -> dict[str, Any]:
    rows, (basic_base, basic_candidate) = qbfootprint.compare_maps(baseline, candidate)
    complete_base, complete_candidate = qbfootprint.linked_code_totals(baseline, candidate)
    if basic_candidate > basic_base or complete_candidate > complete_base:
        raise ValueError("candidate generated-code footprint regressed")
    return {
        "basic": {"baseline": basic_base, "candidate": basic_candidate},
        "complete": {"baseline": complete_base, "candidate": complete_candidate},
        "rows": rows,
    }


def _write_receipt(root: Path, receipt: dict[str, Any]) -> None:
    _atomic_write(root / RECEIPT_NAME, (json.dumps(receipt, indent=2, sort_keys=True) + "\n").encode())


def run(options: Options, *, runner: Runner = _run_dosbox, emitter: Emitter = _direct_object) -> Path:
    root = _prepare_output(options.output)
    receipt: dict[str, Any] = {"schema": 1, "producer": "qbopt.gorillas-gate", "phase": "prepare", "status": "running"}
    _write_receipt(root, receipt)
    try:
        source = options.source.resolve()
        original = source.read_bytes()
        source_hash = _sha256_bytes(original)
        if source_hash != GORILLAS_SHA256:
            raise ValueError(f"GORILLA.BAS sha256 is {source_hash}, expected {GORILLAS_SHA256}")
        derived, transforms = derive_probe(original)
        probe = root / SOURCE_NAME
        _atomic_write(probe, derived)
        receipt["source"] = {"path": str(source), "sha256": source_hash}
        receipt["transformation"] = {
            "description": (
                "fixed game inputs, deterministic seed and self-hit shot; shortened pauses; sampled graphics receipt"
            ),
            "sha256": _sha256_bytes(json.dumps(transforms, sort_keys=True).encode()),
            "changes": transforms,
            "derived": _artifact(root, probe),
        }
        receipt["phase"] = "frontend"
        receipt["frontend"] = {"dialect": "qb45", "runtime": "qb45", "array_order": "column-major"}
        candidate, emitter_hashes = emitter(probe)
        _atomic_write(root / "CAND.OBJ", candidate)
        receipt["pre_run"] = {
            "derived_source": _artifact(root, probe),
            "candidate_object": _artifact(root, root / "CAND.OBJ"),
        }
        config = CONFIGS["q-O"]
        receipt["toolchain"] = {
            "bc_sha256": _sha256(host_path(config.mount, config.bc)),
            "link_sha256": _sha256(host_path(config.mount, config.link)),
            "runtime_sha256": _sha256(host_path(config.mount, config.runtime)),
            **emitter_hashes,
        }
        commands = _commands()
        receipt["commands"] = commands
        receipt["phase"] = "compile-link-run"
        result = runner(root, config.mount, commands, options.timeout)
        receipt["run"] = {"finished": result.finished, "timed_out": result.timed_out, "seconds": result.seconds}
        if not result.finished or result.timed_out:
            raise ValueError("DOSBox did not finish the full compile/link/run batch")
        _assert_unchanged(root, receipt["pre_run"])
        _reject_logs(root)
        _reject_runtime(root)
        artifacts = _required(
            root,
            (
                "BASE.OBJ",
                "CAND.OBJ",
                "BASE.EXE",
                "CAND.EXE",
                "BASE.MAP",
                "CAND.MAP",
                "BASE.OUT",
                "CAND.OUT",
                "GORILLAB.OUT",
                "GORILLAQ.OUT",
            ),
        )
        (
            base_object,
            candidate_object,
            base_exe,
            candidate_exe,
            base_map,
            candidate_map,
            base_stdout,
            candidate_stdout,
            base_output,
            candidate_output,
        ) = artifacts
        if _read_probe(base_output) != _read_probe(candidate_output):
            raise ValueError("baseline and candidate probe output differ")
        receipt["artifacts"] = {
            "baseline_object": _artifact(root, base_object),
            "candidate_object": _artifact(root, candidate_object),
            "baseline_exe": _artifact(root, base_exe),
            "candidate_exe": _artifact(root, candidate_exe),
            "baseline_map": _artifact(root, base_map),
            "candidate_map": _artifact(root, candidate_map),
            "baseline_stdout": _artifact(root, base_stdout),
            "candidate_stdout": _artifact(root, candidate_stdout),
            "baseline_output": _artifact(root, base_output),
            "candidate_output": _artifact(root, candidate_output),
        }
        receipt["phase"] = "quality"
        receipt["quality"] = _quality(base_map, candidate_map)
        receipt["status"] = "passed"
    except BaseException as error:
        receipt["status"] = "failed"
        receipt["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        _write_receipt(root, receipt)
    return root / RECEIPT_NAME


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="gorillas-gate")
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--timeout", default=300, type=int)
    arguments = parser.parse_args(arguments)
    if arguments.timeout < 1:
        parser.error("--timeout must be positive")
    try:
        print(run(Options(arguments.source, arguments.output, arguments.timeout)))
    except (OSError, ValueError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
