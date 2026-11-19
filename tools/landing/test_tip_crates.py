"""The red-tip check skipped llrm-transforms when only a crate it depends on changed: two lib tests were red on the tip it marked green."""
import tip_crates as t

META = {
    "workspace_root": "/w",
    "packages": [
        {"name": "base", "manifest_path": "/w/crates/ir/base/Cargo.toml", "dependencies": []},
        {"name": "mid", "manifest_path": "/w/crates/opt/mid/Cargo.toml", "dependencies": [{"name": "base"}]},
        {"name": "top", "manifest_path": "/w/crates/opt/top/Cargo.toml", "dependencies": [{"name": "mid"}, {"name": "serde"}]},
        {"name": "other", "manifest_path": "/w/crates/opt/other/Cargo.toml", "dependencies": []},
    ],
}


def test_a_change_to_a_dependency_tests_its_dependents():
    assert t.selection(META, ["crates/ir/base/src/lib.rs"]) == "-p base -p mid -p top"


def test_a_leaf_change_tests_only_the_leaf():
    assert t.selection(META, ["crates/opt/top/src/a.rs"]) == "-p top"


def test_a_lockfile_or_unowned_source_tests_the_workspace():
    assert t.selection(META, ["Cargo.lock"]) == "--workspace"
    assert t.selection(META, ["crates/new/src/x.rs"]) == "--workspace"


def test_a_tools_change_selects_nothing():
    assert t.selection(META, ["tools/landing/x.sh"]) == ""


ROOT = {
    "workspace_root": "/w",
    "packages": [
        {"name": "lib", "manifest_path": "/w/crates/lib/Cargo.toml", "dependencies": []},
        {"name": "root", "manifest_path": "/w/Cargo.toml", "dependencies": [{"name": "lib"}]},
    ],
}


def test_a_change_to_a_root_package_binary_selects_the_root_package():
    """src/bin/llrm-mir.rs belongs to the root package, whose directory is "."; it selected nothing, so neither its tests nor a bins check ever ran for it."""
    assert t.selection(ROOT, ["src/bin/llrm-mir.rs"]) == "-p root"
    assert t.selection(ROOT, ["tests/x.rs"]) == "-p root"


def test_a_library_change_still_reaches_the_root_package_that_uses_it():
    assert t.selection(ROOT, ["crates/lib/src/lib.rs"]) == "-p lib -p root"
