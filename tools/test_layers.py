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
            "llrm-object", "llrm-dwarf", "llrm-omf", "llrm-elf",
            "llrm-coff", "llrm-codeview", "llrm-macho", "llrm-target", "llrm-iselgen",
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
    ("frontends", ["llrm-c", "llrm-nib", "llrm-qb", "llrm-qbruntime", "llrm-bcobj"]),
    ("tools", ["llrm"]),
]
RANK = {name: at for at, (_, names) in enumerate(LAYERS) for name in names}

ALLOWED: set[tuple[str, str]] = {
    # BC objects and the QB contract point up into the backend (plan 3d: B1 to B5 remove them)
    ("llrm-codegen", "llrm-qbruntime"),
    ("llrm-bcdriver", "llrm-bcobj"),
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


def test_codegen_names_only_the_format_crates_that_are_not_a_targets():
    """Codegen wrote `elf32`, `elf64`, `coff32`, `coff64` and `macho` by name; those are target tables now (llrm-x86, llrm-arm64), reached through ObjectTarget."""
    codegen = next(p for p in workspace() if p["name"] == "llrm-codegen")
    formats = {d["name"] for d in codegen["dependencies"] if d["kind"] is None and d["name"].startswith(("llrm-elf", "llrm-coff", "llrm-codeview", "llrm-macho", "llrm-omf", "llrm-dwarf"))}
    assert formats <= {"llrm-omf", "llrm-dwarf"}, sorted(formats)


def test_the_root_mir_directory_is_not_tracked():
    """`--dump` writes ./mir/ in the cwd; 97 of its files were committed by accident and nothing reads them."""
    tracked = subprocess.check_output(["git", "ls-files", "mir"], cwd=ROOT, text=True)
    assert tracked == "", tracked.splitlines()[:3]


def test_the_default_build_does_not_make_llrm_c():
    """llrm-llc and llrm-cc are the C compiler; llrm-c builds only with feature c-legacy (and wccq, which brings it)."""
    import tomllib

    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    features = manifest["features"]
    wanted, seen = list(features["default"]), set()
    while wanted:
        name = wanted.pop()
        if name not in seen:
            seen.add(name)
            wanted += [f for f in features.get(name, []) if "/" not in f and not f.startswith("dep:")]
    assert "c-legacy" not in seen and "wccq" not in seen, seen
    binary = next(b for b in manifest["bin"] if b["name"] == "llrm-c")
    assert binary["required-features"] == ["c-legacy"]


def test_a_frontend_crate_has_no_binary_of_its_own():
    """qbfront was a second QB binary beside llrm-qb; frontends are libraries the root binaries link."""
    for manifest in sorted((ROOT / "crates" / "frontends").glob("*/Cargo.toml")):
        assert "[[bin]]" not in manifest.read_text(), manifest.relative_to(ROOT)


def test_the_workspace_members_follow_the_layer_glob():
    """Members were `crates/*/llrm-*` plus three frontends and a runtime crate named one by one; a crate in a layer directory that matched none was silently outside the workspace."""
    import tomllib

    members = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["members"]
    assert members == [".", "crates/*/*", "runtime/qb"]
    metadata = json.loads(subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT, capture_output=True, text=True, check=True).stdout)
    inside = {Path(p["manifest_path"]).parent.relative_to(ROOT).as_posix() for p in metadata["packages"]}
    layers = {p.parent.relative_to(ROOT).as_posix() for p in (ROOT / "crates").glob("*/*/Cargo.toml")}
    assert layers <= inside, sorted(layers - inside)


def test_llrm_c_lib_tests_build_without_the_root_enabling_its_feature():
    """llrm-c became optional in the root: `--workspace --all-targets` then compiled its lib tests without `toolchain` and stopped at the compile_error (#5586 refused 00:51)."""
    done = subprocess.run(["cargo", "check", "-p", "llrm-c", "--lib", "--tests"], cwd=ROOT, capture_output=True, text=True)
    assert done.returncode == 0, done.stderr[-600:]


def test_codegen_names_no_object_target():
    """`objbuild` wrote `X86Objects` by name, a second dispatch beside arm64's; codegen is handed the target's `ObjectTarget` (`Target::objects`)."""
    named = sorted(
        str(path.relative_to(ROOT))
        for path in (ROOT / "crates" / "codegen" / "llrm-codegen" / "src").rglob("*.rs")
        if "X86Objects" in path.read_text() or "Arm64Objects" in path.read_text()
    )
    assert not named, named


def test_llrm_target_depends_on_the_object_crate_alone_among_the_mc_crates():
    """`Target::objects` returns an `ObjectTarget`: the one edge from the target interface to the mc layer."""
    target = next(p for p in workspace() if p["name"] == "llrm-target")
    mc = {d["name"] for d in target["dependencies"] if d["kind"] is None and d["name"].startswith(("llrm-elf", "llrm-coff", "llrm-codeview", "llrm-macho", "llrm-omf", "llrm-dwarf", "llrm-object"))}
    assert mc == {"llrm-object"}, sorted(mc)
