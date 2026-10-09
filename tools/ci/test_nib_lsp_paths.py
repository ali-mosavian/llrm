"""nib-lsp is built on a push to main only when a path in its closure changed."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import nib_lsp_paths as nl  # noqa: E402


def test_the_release_workflow_filters_on_exactly_the_crates_nib_lsp_builds_from():
    """A crate added to nib-lsp's dependencies but not to the filter would change the binary without building it."""
    text = nl.WORKFLOW.read_text()
    assert nl.listed(text) == nl.paths(), "run: python3 tools/ci/nib_lsp_paths.py --write"


def test_the_workflow_builds_only_on_main_and_tags_never_on_a_pull_request():
    """It built on every pull request that touched crates/** or src/**: nearly all of them."""
    text = nl.WORKFLOW.read_text()
    head = text.split("jobs:")[0]
    assert "pull_request" not in head
    assert "tags:" in head and "nib-lsp-v*" in head and "branches: [main]" in head


def test_a_crate_in_the_closure_is_in_the_filter_and_one_outside_it_is_not():
    got = nl.paths()
    assert "crates/ir/llrm-mir/**" in got
    assert not any(p.startswith("tools/") for p in got)


def test_nothing_but_nib_lsps_own_sources_triggers_a_build():
    """The filter once held Cargo.lock, the toolchain file, .cargo and the workflow itself: a change to none of nib-lsp's code built it."""
    got = nl.paths()
    assert not {"Cargo.lock", "rust-toolchain.toml", ".cargo/**"} & set(got)
    assert not any(p.startswith(".github") or p.startswith("tools") for p in got)
    assert got[-3:] == ["src/**", "!src/bin/**", "src/bin/nib-lsp.rs"]
