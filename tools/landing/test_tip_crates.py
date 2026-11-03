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
