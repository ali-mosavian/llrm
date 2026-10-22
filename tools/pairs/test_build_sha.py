"""Two builds at once, each of its own sha: each binary's stamp is its own sha (stub cargo, no real build)."""
from __future__ import annotations

import os
import subprocess
import textwrap
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "build_sha.sh"


def git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True, text=True).stdout.strip()


def test_concurrent_builds_of_two_shas_each_carry_their_own_sha(tmp_path):
    """The dist for 65eed2ad7 was another session's checkout: wt-head10 moved during its 12-minute build. Every build has a
    worktree of its own now; the stub stamps the commit of the tree it is run in at the END of the build, as a build.rs
    that reruns late would."""
    repo = tmp_path / "repo"
    repo.mkdir()
    git(repo, "init", "-q")
    shas = []
    for n in (1, 2):
        (repo / "f").write_text(str(n))
        git(repo, "add", "f")
        git(repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", str(n))
        shas.append(git(repo, "rev-parse", "HEAD"))
    # The tools the script reaches from its own path.
    tools = tmp_path / "tools"
    (tools / "pairs").mkdir(parents=True)
    for name in ("build_sha.sh", "pair_build.sh"):
        (tools / "pairs" / name).write_text((HERE / name).read_text())
    (tools / "stamp.py").write_text((HERE.parent / "stamp.py").read_text())
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    stub = scratch / "cargo-stub"
    stub.write_text(textwrap.dedent("""\
        #!/bin/bash
        sleep 2
        mkdir -p $CARGO_TARGET_DIR/dist
        for b in llrm-c llrm-qb llrm-nib nibfront; do
          printf '#!/bin/sh\\necho "%s %s"\\n' $b "$(git rev-parse HEAD)" > $CARGO_TARGET_DIR/dist/$b
        done
        for t in jwasm jwlink dosbox-x; do echo $t > $CARGO_TARGET_DIR/dist/$t; done
        chmod +x $CARGO_TARGET_DIR/dist/*; echo "llrm-c: src.rs" > $CARGO_TARGET_DIR/dist/llrm-c.d
        """))
    stub.chmod(0o755)
    env = {**os.environ, "PAIR_CARGO": f"{stub} cargo"}
    running = [
        subprocess.Popen(["bash", str(tools / "pairs" / "build_sha.sh"), str(repo), str(scratch), sha], env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        for sha in shas
    ]
    for one in running:
        err, _ = one.communicate()
        assert one.returncode == 0, err or "no stderr"
    for sha in shas:
        said = subprocess.run([str(scratch / f"sbp-{sha}" / "dist" / "llrm-c"), "--version"], capture_output=True, text=True).stdout.split()
        assert said == ["llrm-c", sha]
