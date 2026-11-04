"""toolchain/clangfetch.sh puts a clang tree where the setup action looks when the cache misses.

Run 40780286803 (the pull request release/next into main) restored neither clang tree: the caches were saved on refs/heads/release/next, which
a pull request into main cannot read. LLRM_PIN_CLANG stayed unset and every llrm-cc row failed on ~/scratch/llvm-build/bin/clang."""

import os
import subprocess
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SETUP = (ROOT / ".github/actions/setup/action.yml").read_text()


def fake_gh(tmp_path: Path, artifact: Path, listed: str) -> dict:
    """A `gh` that lists `listed` for the artifact query and sends `artifact` for the download."""
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    gh = bin_dir / "gh"
    gh.write_text(f'#!/bin/sh\ncase "$2" in\n*artifacts*) echo "{listed}" ;;\n*) cat {artifact} ;;\nesac\n')
    gh.chmod(0o755)
    return {**os.environ, "PATH": f"{bin_dir}:{os.environ['PATH']}", "GITHUB_REPOSITORY": "o/r"}


def artifact(tmp_path: Path) -> Path:
    tree = tmp_path / "src/bin"
    tree.mkdir(parents=True)
    (tree / "clang").write_text("#!/bin/sh\necho clang version 99 d1106deb71cc\n")
    (tree / "clang").chmod(0o755)
    tar = tmp_path / "tree.tar.zst"
    subprocess.run(f"tar -C {tmp_path / 'src'} -cf - . | zstd -q -o {tar}", shell=True, check=True)
    zipped = tmp_path / "a.zip"
    with zipfile.ZipFile(zipped, "w") as z:
        z.write(tar, "tree.tar.zst")
    return zipped


def fetch(tmp_path: Path, listed: str):
    dest = tmp_path / "cache/clang-d1106deb71cc"
    env = fake_gh(tmp_path, artifact(tmp_path), listed)
    done = subprocess.run(["sh", str(ROOT / "toolchain/clangfetch.sh"), "llrm-clang-d1106deb71cc", str(dest)], capture_output=True, text=True, env=env)
    return done, dest


def test_a_cache_miss_is_filled_from_the_artifact_with_clang_executable(tmp_path):
    done, dest = fetch(tmp_path, "https://api.github.com/x/zip")
    assert done.returncode == 0, done.stderr
    assert os.access(dest / "bin/clang", os.X_OK)


def test_no_live_artifact_is_red(tmp_path):
    done, dest = fetch(tmp_path, "null")
    assert done.returncode == 1 and "no live artifact" in done.stderr and not dest.exists()


def test_the_setup_action_falls_back_to_the_artifact_before_it_exports_the_variables():
    assert SETUP.index("sh toolchain/clangfetch.sh") < SETUP.index('echo "LLRM_PIN_CLANG=$pin"')
    assert "GH_TOKEN: ${{ github.token }}" in SETUP


def test_the_toolchain_workflow_uploads_each_tree_as_the_artifact_the_fallback_names():
    action = (ROOT / ".github/actions/clang-tree/action.yml").read_text()
    assert "name: llrm-${{ steps.tree.outputs.name }}" in action and "tree.tar.zst" in action
    assert action.index("actions/cache/save@v4") < action.index("actions/upload-artifact@v4")
    assert 'artifact: "true"' in (ROOT / ".github/workflows/toolchain.yml").read_text()


def test_only_the_toolchain_workflow_uploads_artifacts_not_every_pull_request_run():
    assert "artifact:" not in (ROOT / ".github/workflows/ci.yml").read_text().split("  toolchain:\n")[1].split("\n  # The pin's llc")[0]


def test_the_fallback_runs_before_the_pin_tools_step_that_links_the_pin_clang():
    """The pin tools step links the pin clang into LLVM_BIN: on a cache miss that clang exists only after the fallback."""
    assert SETUP.index("sh toolchain/clangfetch.sh") < SETUP.index("name: The pin's tools tree (built by the pintools job)")
