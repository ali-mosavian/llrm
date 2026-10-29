"""pair_build.sh builds each sha in a worktree of its own, keeps one dist target, and leaves only the binaries per sha."""
from __future__ import annotations

import os
import shutil
import subprocess
import textwrap
import threading
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "pair_build.sh"


def repo_with_commits(path: Path, count: int) -> list[str]:
    path.mkdir()
    git = lambda *args: subprocess.run(["git", "-C", str(path), *args], check=True, capture_output=True, text=True).stdout.strip()  # noqa: E731
    git("init", "-q")
    shas = []
    for number in range(count):
        (path / "tree.txt").write_text(f"tree {number}\n")
        git("add", "tree.txt")
        git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", f"c{number}")
        shas.append(git("rev-parse", "HEAD"))
    return shas


def stub(scratch: Path, log: Path) -> dict:
    """A cargo whose binaries say which tree they were built from, as w-3's stamp does."""
    stub = scratch / "cargo-stub"
    stub.write_text(textwrap.dedent(f"""\
        #!/bin/bash
        echo "$CARGO_TARGET_DIR" >> {log}
        sleep 0.3
        mkdir -p $CARGO_TARGET_DIR/dist $CARGO_TARGET_DIR/deps
        # a binary reads a header from the tree it was built in when it runs, as the real one does
        for b in llrm-cc llrm-llc llrm-qb llrm-nib nibfront; do printf '#!/bin/sh\\n# %s\\ncat "%s/tree.txt"\\n' "$(git rev-parse HEAD)" "$(pwd)" > $CARGO_TARGET_DIR/dist/$b; done
        for t in jwasm jwlink dosbox-x; do echo $t > $CARGO_TARGET_DIR/dist/$t; done
        chmod +x $CARGO_TARGET_DIR/dist/*; echo "llrm-llc: $(pwd)/src.rs" > $CARGO_TARGET_DIR/dist/llrm-llc.d
        """))
    stub.chmod(0o755)
    return {**os.environ, "PAIR_CARGO": f"{stub} cargo"}


def built_from(binary: Path) -> str:
    return binary.read_text().splitlines()[1].lstrip("# ")


def build(scratch: Path, repo: Path, sha: str, env: dict) -> None:
    subprocess.run(["bash", str(SCRIPT), str(scratch), sha], cwd=repo, env=env, check=True)


def test_two_concurrent_builds_of_different_shas_each_carry_their_own_tree(tmp_path):
    """The 65eed2ad7 dist was built in a shared worktree another session moved mid-build: a binary of an unknown tree under a sha's name."""
    repo = tmp_path / "wt-head10"
    first, second = repo_with_commits(repo, 2)
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    env = stub(scratch, tmp_path / "targets.log")
    workers = [threading.Thread(target=build, args=(scratch, repo, sha, env)) for sha in (first, second)]
    for worker in workers:
        worker.start()
    for worker in workers:
        worker.join()
    for sha in (first, second):
        assert built_from(scratch / f"sbp-{sha}" / "dist" / "llrm-llc") == sha
    assert subprocess.run(["git", "-C", str(repo), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip() == second
    assert len(list(scratch.glob("wt-pair-*"))) == 2, "the build worktrees are kept for the dist to read"


def test_a_pair_leaves_only_the_binaries_per_sha_and_all_builds_share_one_target(tmp_path):
    """A target dir per sha left a whole build behind for each of 76 pairs (51 GB in a day, on a host falling 50 GB/h toward its alarm)."""
    repo = tmp_path / "wt-head10"
    shas = repo_with_commits(repo, 2)
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    log = tmp_path / "targets.log"
    env = stub(scratch, log)
    for sha in shas:
        build(scratch, repo, sha, env)
    for sha in shas:
        left = sorted(p.relative_to(scratch / f"sbp-{sha}").as_posix() for p in (scratch / f"sbp-{sha}").rglob("*") if p.is_file())
        assert left == ["dist/dosbox-x", "dist/jwasm", "dist/jwlink", "dist/llrm-cc", "dist/llrm-llc", "dist/llrm-llc.d", "dist/llrm-nib", "dist/llrm-qb", "dist/nibfront", "ok"], (sha, left)
    assert set(log.read_text().split()) == {str(scratch / "tgt-dist")}
    build(scratch, repo, shas[0], env)  # built already: no second build
    assert len(log.read_text().split()) == 2


def test_a_built_dist_is_write_once_and_a_rebuild_goes_to_another_dir(tmp_path):
    """Another session copied a different llrm-llc over a built sbp-<sha>/dist (md5 01fdf3625b2a became ecbc4f3de389): `ok` records the sums, the files are read-only, a changed file is refused, a rebuild never overwrites."""
    repo = tmp_path / "wt-head10"
    (sha,) = repo_with_commits(repo, 1)
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    env = stub(scratch, tmp_path / "targets.log")
    build(scratch, repo, sha, env)
    ok = (scratch / f"sbp-{sha}" / "ok").read_text()
    assert f"sha: {sha}" in ok and "cmd: build --profile dist" in ok and "  llrm-llc" in ok, ok
    binary = scratch / f"sbp-{sha}" / "dist" / "llrm-llc"
    assert not os.access(binary, os.W_OK)
    original = binary.read_bytes()
    binary.chmod(0o755)
    binary.write_text("another tree\n")
    refused = subprocess.run(["bash", str(SCRIPT), str(scratch), sha], cwd=repo, env=env, capture_output=True, text=True)
    assert refused.returncode != 0 and "not what its ok recorded" in refused.stderr, refused
    binary.write_bytes(original)
    build(scratch, repo, sha, env)  # restored: kept as it is
    subprocess.run(["bash", str(SCRIPT), str(scratch), sha], cwd=repo, env={**env, "PAIR_REBUILD": "1"}, check=True)
    assert built_from(scratch / f"sbp-{sha}-2" / "dist" / "llrm-llc") == sha
    assert built_from(binary) == sha


def test_the_dist_still_compiles_after_the_pair_and_an_old_worktree_is_pruned(tmp_path):
    """w-1's ex4 was void: the build worktree was removed on exit, and a binary opens a header under the build tree at run time, so every compile failed on both arms."""
    repo = tmp_path / "wt-head10"
    first, second = repo_with_commits(repo, 2)
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    env = stub(scratch, tmp_path / "targets.log")
    build(scratch, repo, first, env)
    ran = subprocess.run([str(scratch / f"sbp-{first}" / "dist" / "llrm-llc")], capture_output=True, text=True)
    assert ran.returncode == 0 and ran.stdout.strip() == "tree 0", ran
    old = scratch / f"wt-pair-{first}"
    os.utime(old, (1, 1))
    build(scratch, repo, second, {**env, "PAIR_KEEP_MIN": "60"})
    assert old.exists(), "the tree of a dist still there is kept: the dist reads its headers from it"
    assert (scratch / f"wt-pair-{second}").exists()
    # With its dist gone the old tree is pruned by age.
    shutil.rmtree(scratch / f"sbp-{first}")
    os.utime(old, (1, 1))
    subprocess.run(["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "c2"], check=True)
    head = subprocess.run(["git", "-C", str(repo), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    build(scratch, repo, head, {**env, "PAIR_KEEP_MIN": "60"})
    assert not old.exists()


def test_a_failed_cargo_build_publishes_no_dist(tmp_path):
    """w5 found by md5 a head dist with the base's binaries: cargo failed to compile the head, the script went on and copied what the shared target held (the base's build) under the head's sha, so a pair measured the base twice."""
    repo = tmp_path / "wt-head10"
    base, head = repo_with_commits(repo, 2)
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    env = stub(scratch, tmp_path / "targets.log")
    build(scratch, repo, base, env)
    failing = scratch / "cargo-fails"
    failing.write_text("#!/bin/bash\necho 'error[E0425]: cannot find value' >&2\nexit 101\n")
    failing.chmod(0o755)
    done = subprocess.run(["bash", str(SCRIPT), str(scratch), head], cwd=repo, env={**env, "PAIR_CARGO": f"{failing} cargo"}, capture_output=True, text=True)
    assert done.returncode != 0
    assert not (scratch / f"sbp-{head}" / "ok").exists() and not (scratch / f"sbp-{head}" / "dist" / "llrm-llc").exists()


def test_a_build_that_left_the_previous_binary_in_place_is_refused_not_stored_under_the_new_sha(tmp_path):
    """Four dists (5e4cac094d, aa8170fbbd, 8cb8ed34df, 3d22dc1faa) were the previous build copied under a new sha: their llrm-llc.d named another commit's worktree."""
    repo = tmp_path / "wt-head10"
    first, second = repo_with_commits(repo, 2)
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    env = stub(scratch, tmp_path / "targets.log")
    build(scratch, repo, first, env)
    idle = scratch / "cargo-idle"
    idle.write_text("#!/bin/bash\nexit 0\n")  # cargo found nothing to do: tgt-dist keeps the first build
    idle.chmod(0o755)
    done = subprocess.run(["bash", str(SCRIPT), str(scratch), second], cwd=repo, env={**env, "PAIR_CARGO": f"{idle} cargo"}, capture_output=True, text=True)
    assert done.returncode != 0 and "not built from" in done.stderr, done.stderr
    assert not (scratch / f"sbp-{second}" / "dist" / "llrm-llc").exists()


def test_the_cargo_of_a_dist_build_asks_buildslot_to_jump_the_queue(tmp_path):
    """A pair_build holding the shared dist lock queued for a build slot behind short tests, 44 min at 0% CPU, with 11 more builds waiting on the lock."""
    repo = tmp_path / "repo"
    (sha,) = repo_with_commits(repo, 1)
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    env = stub(scratch, tmp_path / "targets.log")
    seen = tmp_path / "priority"
    wrapper = scratch / "cargo-priority"
    wrapper.write_text(f'#!/bin/bash\necho "${{BUILDSLOT_PRIORITY:-unset}}" > {seen}\nexec {scratch / "cargo-stub"} "$@"\n')
    wrapper.chmod(0o755)
    build(scratch, repo, sha, {**env, "PAIR_CARGO": f"{wrapper} cargo"})
    assert seen.read_text().strip() == "0"
