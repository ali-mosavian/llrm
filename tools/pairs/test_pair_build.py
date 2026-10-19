"""pair_build.sh keeps one dist target per worktree and leaves only the binaries per sha."""
from __future__ import annotations

import os
import subprocess
import textwrap
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "pair_build.sh"


def run(scratch: Path, worktree: Path, sha: str, log: Path) -> None:
    stub = scratch / "cargo-stub"
    stub.write_text(textwrap.dedent(f"""\
        #!/bin/bash
        echo "$CARGO_TARGET_DIR" >> {log}
        mkdir -p $CARGO_TARGET_DIR/dist $CARGO_TARGET_DIR/deps
        for b in llrm-c llrm-qb llrm-nib nibfront; do echo $b > $CARGO_TARGET_DIR/dist/$b; done
        chmod +x $CARGO_TARGET_DIR/dist/*; echo "llrm-c: src.rs" > $CARGO_TARGET_DIR/dist/llrm-c.d
        head -c 1000 /dev/zero > $CARGO_TARGET_DIR/deps/big.rlib
        """))
    stub.chmod(0o755)
    env = {**os.environ, "PAIR_CARGO": f"{stub} cargo"}
    subprocess.run(["bash", str(SCRIPT), str(scratch), sha], cwd=worktree, env=env, check=True)


def test_a_pair_leaves_only_the_binaries_per_sha_and_two_pairs_on_one_worktree_share_one_target(tmp_path):
    """A target dir per sha left a whole build behind for each of 76 pairs (51 GB in a day, on a host falling 50 GB/h toward its alarm)."""
    worktree = tmp_path / "wt-head10"
    worktree.mkdir()
    log = tmp_path / "targets.log"
    run(tmp_path, worktree, "aaa111", log)
    run(tmp_path, worktree, "bbb222", log)
    for sha in ("aaa111", "bbb222"):
        left = sorted(p.relative_to(tmp_path / f"sbp-{sha}").as_posix() for p in (tmp_path / f"sbp-{sha}").rglob("*") if p.is_file())
        assert left == ["dist/llrm-c", "dist/llrm-c.d", "dist/llrm-nib", "dist/llrm-qb", "dist/nibfront", "ok"], (sha, left)
    targets = set(log.read_text().split())
    assert targets == {str(tmp_path / "tgt-dist-wt-head10")}, targets
    run(tmp_path, worktree, "aaa111", log)  # built already: no second build
    assert len(log.read_text().split()) == 2
