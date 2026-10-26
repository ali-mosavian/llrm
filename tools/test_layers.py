"""`uv run --project tools python -m pytest tools/test_layers.py`

The crates' layers run support < ir < analysis < transforms < mc and the target interface < codegen < concrete targets < driver < frontends,
as LLVM layers CodeGen over MC (layout plan, section 1). A normal dependency may name its own layer or one below it.
ALLOWED lists today's upward edges and may only shrink: a new one fails, and so does an entry that is no longer a violation.
"""
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

LAYERS = [
    ("support", ["llrm-env", "llrm-support", "llrm-alloc", "llrm-adt"]),
    ("ir", ["llrm-mir", "llrm-lir", "llrm-hir"]),
    ("analysis", ["llrm-analysis"]),
    ("transforms", ["llrm-transforms", "llrm-sroa-poc"]),
    (
        "mc and target interface",
        [
            "llrm-object", "llrm-debuginfo", "llrm-omf", "llrm-elf", "llrm-elf32", "llrm-elf64", "llrm-elf-arm64",
            "llrm-coff", "llrm-coff32", "llrm-coff64", "llrm-macho", "llrm-target", "llrm-instr", "llrm-iselgen",
            "llrm-peepgen",
        ],
    ),
    ("codegen", ["llrm-codegen", "llrm-dagisel", "llrm-dagisel-gen"]),
    (
        "concrete targets",
        [
            "llrm-x86", "llrm-x86-m16", "llrm-x86-m32", "llrm-x86-m64", "llrm-x86-m16-select", "llrm-x86-m32-select",
            "llrm-x86-m64-select", "llrm-arm64",
        ],
    ),
    ("driver", ["llrm-driver", "llrm-cc", "llrm-bcdriver"]),
    ("frontends", ["llrm-c", "llrm-nib", "llrm-qb", "llrm-qbruntime", "llrm-x86-bc", "llrm-x86-bcmachine"]),
    ("tools", ["llrm"]),
]
RANK = {name: at for at, (_, names) in enumerate(LAYERS) for name in names}

ALLOWED: set[tuple[str, str]] = {
    # BC objects and the QB contract point up into the backend (plan 3d: B1 to B5 remove them)
    ("llrm-codegen", "llrm-x86-bcmachine"),
    ("llrm-codegen", "llrm-qbruntime"),
    ("llrm-bcdriver", "llrm-x86-bc"),
    # target facts read from a concrete target: machine facts should be passed in (one machine description)
    ("llrm-codegen", "llrm-x86"),
    ("llrm-hir", "llrm-target"),
    ("llrm-lir", "llrm-omf"),
}


def violations(packages: list[dict], rank: dict[str, int] = RANK) -> set[tuple[str, str]]:
    """(crate, dependency) for each normal dependency on a workspace crate of a higher layer."""
    members = {p["name"] for p in packages}
    found = set()
    for p in packages:
        for d in p["dependencies"]:
            if d["kind"] is None and d["name"] in members and rank[d["name"]] > rank[p["name"]]:
                found.add((p["name"], d["name"]))
    return found


def workspace() -> list[dict]:
    out = subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=ROOT, text=True)
    return json.loads(out)["packages"]


def test_an_upward_dependency_is_reported():
    packages = [
        {"name": "low", "dependencies": [{"name": "high", "kind": None}, {"name": "same", "kind": None}]},
        {"name": "high", "dependencies": [{"name": "low", "kind": None}]},
        {"name": "same", "dependencies": [{"name": "high", "kind": "dev"}]},
    ]
    assert violations(packages, {"low": 0, "same": 0, "high": 1}) == {("low", "high")}


def test_every_workspace_crate_has_a_layer():
    missing = sorted(p["name"] for p in workspace() if p["name"] not in RANK)
    assert not missing, f"add to LAYERS: {missing}"


def test_no_new_upward_dependency_and_the_allow_list_only_shrinks():
    found = violations(workspace())
    assert not found - ALLOWED, f"new upward dependencies: {sorted(found - ALLOWED)}"
    assert not ALLOWED - found, f"no longer violations, remove from ALLOWED: {sorted(ALLOWED - found)}"
