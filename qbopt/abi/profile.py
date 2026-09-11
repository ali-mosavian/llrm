import json
from dataclasses import dataclass, replace
from hashlib import sha256
from pathlib import Path

from qbopt.abi import runtime
from qbopt.objectfile import omf


@dataclass(frozen=True, slots=True)
class Profile:
    rules: tuple[runtime.Contract, ...]
    fingerprint: str


def _unique(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result = {}
    for name, value in pairs:
        if name in result:
            raise ValueError(f"duplicate contract profile key: {name}")
        result[name] = value
    return result


def _fields(value: object, required: set[str], optional: set[str] | frozenset[str] = frozenset()) -> dict:
    if not isinstance(value, dict) or not required <= value.keys() or value.keys() - required - optional:
        raise ValueError(f"expected profile fields {sorted(required)}, optional {sorted(optional)}")
    return value


def load(path: Path, root: Path | None = None) -> Profile:
    document = _fields(json.loads(path.read_text(), object_pairs_hook=_unique), {"version", "artifacts", "contracts"})
    if type(document["version"]) is not int or document["version"] != 1:
        raise ValueError("unsupported contract profile version")
    artifacts, declarations = document["artifacts"], document["contracts"]
    if not isinstance(artifacts, dict) or not artifacts or not isinstance(declarations, dict) or not declarations:
        raise ValueError("contract profile needs nonempty artifacts and contracts maps")
    directory = (root or path.parent).resolve()
    contents = {}
    for name, expected in artifacts.items():
        relative = Path(name)
        location = (directory / relative).resolve()
        if relative.is_absolute() or ".." in relative.parts or not location.is_relative_to(directory):
            raise ValueError(f"profile artifact must stay inside the contract root: {name}")
        if not isinstance(expected, str) or len(expected) != 64 or any(c not in "0123456789abcdef" for c in expected):
            raise ValueError(f"invalid SHA-256 for profile artifact: {name}")
        data = location.read_bytes()
        if sha256(data).hexdigest() != expected:
            raise ValueError(f"contract profile artifact hash mismatch: {name}")
        contents[name] = data
    symbols = {}
    rules = []
    for name, declaration in sorted(declarations.items()):
        row = _fields(declaration, {"defined_in", "inputs", "evidence"}, {"cleanup"})
        defining = row["defined_in"]
        if not isinstance(defining, str) or defining not in contents:
            raise ValueError(f"{name}: defining object is not a verified artifact")
        if defining not in symbols:
            records = omf.parse(contents[defining])
            symbols[defining] = {
                symbol
                for seg in range(1, len(omf.segments(records)))
                for symbol in omf.pubdef_names(records, seg).values()
            }
        if not name or name not in symbols[defining]:
            raise ValueError(f"{name}: symbol is not defined in {defining}")
        inputs = row["inputs"]
        if not isinstance(inputs, list) or any(not isinstance(item, str) for item in inputs):
            raise ValueError(f"{name}: inputs must be a list of register names")
        registers = frozenset(runtime.Reg(item) for item in inputs)
        if len(registers) != len(inputs):
            raise ValueError(f"{name}: duplicate input register")
        evidence = row["evidence"]
        if not isinstance(evidence, str) or not evidence.strip():
            raise ValueError(f"{name}: audit evidence is required")
        cleanup = row.get("cleanup")
        if cleanup is not None and (type(cleanup) is not int or not 0 <= cleanup <= 65535 or cleanup % 2):
            raise ValueError(f"{name}: cleanup must be an even byte count or null")
        rules.append(replace(runtime.worst(name), inputs=registers, cleanup=cleanup, evidence=evidence))
    fingerprint = sha256(json.dumps(document, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return Profile(tuple(rules), fingerprint)
