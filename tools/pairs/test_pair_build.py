"""pair_build.sh builds each sha in a worktree of its own, keeps one dist target, and leaves only the binaries per sha."""
from __future__ import annotations

import os
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
        for b in llrm-c llrm-qb llrm-nib nibfront; do git rev-parse HEAD > $CARGO_TARGET_DIR/dist/$b; done
        for t in jwasm jwlink dosbox-x; do echo $t > $CARGO_TARGET_DIR/dist/$t; done
        chmod +x $CARGO_TARGET_DIR/dist/*; echo "llrm-c: src.rs" > $CARGO_TARGET_DIR/dist/llrm-c.d
        """))
    stub.chmod(0o755)
    return {**os.environ, "PAIR_CARGO": f"{stub} cargo"}


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
        assert (scratch / f"sbp-{sha}" / "dist" / "llrm-c").read_text().strip() == sha
    assert subprocess.run(["git", "-C", str(repo), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip() == second
    assert not list(scratch.glob("wt-pair-*")), "the build worktrees are removed"


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
        assert left == ["dist/dosbox-x", "dist/jwasm", "dist/jwlink", "dist/llrm-c", "dist/llrm-c.d", "dist/llrm-nib", "dist/llrm-qb", "dist/nibfront", "ok"], (sha, left)
    assert set(log.read_text().split()) == {str(scratch / "tgt-dist")}
    build(scratch, repo, shas[0], env)  # built already: no second build
    assert len(log.read_text().split()) == 2
