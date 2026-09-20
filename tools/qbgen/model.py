"""Data model and DOS-safe rendering for generated QB-family source tests."""

from __future__ import annotations

import re
import json
import hashlib
from pathlib import Path
from dataclasses import dataclass

_DOS_FILE = re.compile(r"^[A-Z0-9]{1,8}\.[A-Z0-9]{1,3}$")
_DIALECTS = frozenset({"qb45", "pds71", "vbdos"})
_OUTCOMES = frozenset({"accepted", "syntax-error", "semantic-error"})


@dataclass(frozen=True, slots=True)
class DialectOutcome:
    """The required result for one language profile, not an ambiguous allow-list."""

    dialect: str
    result: str = "accepted"


ACCEPTED_ALL = (
    DialectOutcome("qb45"),
    DialectOutcome("pds71"),
    DialectOutcome("vbdos"),
)


@dataclass(frozen=True, slots=True)
class GeneratedCase:
    """One independent source witness, including facts a bad compiler cannot fake.

    ``ast`` names the syntax nodes which must survive parsing.  The public
    frontend boundary intentionally exposes HIR rather than parser-private
    Rust syntax, so Python checks parser acceptance independently and checks
    the observable semantic/HIR consequences below it.  Rust parser tests can
    use the same manifest when a parser-private assertion is appropriate.
    """

    family: str
    name: str
    source: str
    outcomes: tuple[DialectOutcome, ...] = ACCEPTED_ALL
    ast: tuple[str, ...] = ()
    bindings: tuple[tuple[str, str], ...] = ()
    hir_ops: tuple[str, ...] = ()
    runtime_calls: tuple[str, ...] = ()
    expected_output: tuple[str, ...] = ()
    classification: str = "required"
    origin: str = ""

    def filename(self, ordinal: int) -> str:
        """Return a stable 8.3 filename, independent of a host pathname."""
        code = "".join(part[0] for part in self.family.upper().split("_"))[:2] or "G"
        return f"G{code}{ordinal:05d}.BAS"

    def manifest_entry(self, ordinal: int) -> dict[str, object]:
        filename = self.filename(ordinal)
        payload = render_bas(self.source).encode("ascii")
        return {
            "file": filename,
            "family": self.family,
            "name": self.name,
            "outcomes": tuple({"dialect": one.dialect, "result": one.result} for one in self.outcomes),
            "ast": self.ast,
            "bindings": self.bindings,
            "hir_ops": self.hir_ops,
            "runtime_calls": self.runtime_calls,
            "expected_output": self.expected_output,
            "classification": self.classification,
            "origin": self.origin,
            "sha256": hashlib.sha256(payload).hexdigest(),
        }


def render_bas(source: str) -> str:
    """Render ASCII BASIC as DOS CRLF, ending in exactly one CRLF."""
    normalized = source.replace("\r\n", "\n").replace("\r", "\n").strip("\n")
    if not normalized.isascii():
        raise ValueError("generated BASIC must be ASCII for DOS toolchains")
    return normalized.replace("\n", "\r\n") + "\r\n"


def write_cases(directory: Path, generated: tuple[GeneratedCase, ...]) -> Path:
    """Write reproducible 8.3/CRLF input and a deterministic manifest."""
    directory.mkdir(parents=True, exist_ok=True)
    entries: list[dict[str, object]] = []
    for ordinal, case in enumerate(generated, 1):
        filename = case.filename(ordinal)
        if not _DOS_FILE.fullmatch(filename):
            raise ValueError(f"generated filename is not DOS 8.3: {filename}")
        (directory / filename).write_bytes(render_bas(case.source).encode("ascii"))
        entries.append(case.manifest_entry(ordinal))
    # The manifest travels with DOS fixtures too, so it obeys 8.3 as well.
    manifest = directory / "MANIFEST.JSN"
    manifest.write_text(json.dumps({"schema": 1, "cases": entries}, indent=2, sort_keys=True) + "\n")
    return manifest


def validate(generated: tuple[GeneratedCase, ...]) -> None:
    names: set[str] = set()
    for ordinal, case in enumerate(generated, 1):
        if not case.name or case.name in names:
            raise ValueError(f"duplicate or blank generated case name: {case.name!r}")
        names.add(case.name)
        dialects = [one.dialect for one in case.outcomes]
        if not dialects or not set(dialects) <= _DIALECTS or len(set(dialects)) != len(dialects):
            raise ValueError(f"{case.name}: invalid per-dialect outcomes {case.outcomes!r}")
        if any(one.result not in _OUTCOMES for one in case.outcomes):
            raise ValueError(f"{case.name}: unknown per-dialect result")
        accepted = any(one.result == "accepted" for one in case.outcomes)
        if not accepted and (case.bindings or case.hir_ops or case.runtime_calls):
            raise ValueError(f"{case.name}: fully rejected source cannot assert lowered facts")
        if case.expected_output and not accepted:
            raise ValueError(f"{case.name}: fully rejected source cannot have a DOS verdict")
        if not _DOS_FILE.fullmatch(case.filename(ordinal)):
            raise ValueError(f"{case.name}: generated filename is not 8.3")
        render_bas(case.source)
