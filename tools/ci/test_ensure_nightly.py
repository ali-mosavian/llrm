import os
import stat
import subprocess
from pathlib import Path

SCRIPT = Path(__file__).parent / "ensure_nightly.sh"


def run(tmp_path: Path, toolchains: str, components: str) -> list[str]:
    """The rustup calls the script makes, with a rustup that lists `toolchains` and `components` and logs every call."""
    log = tmp_path / "calls"
    stub = tmp_path / "rustup"
    stub.write_text(
        '#!/bin/bash\necho "$*" >> "%s"\n'
        'case "$1 $2" in "toolchain list") printf "%%s" "%s";; "component list") printf "%%s" "%s";; esac\n' % (log, toolchains, components)
    )
    stub.chmod(stub.stat().st_mode | stat.S_IXUSR)
    subprocess.run(["bash", str(SCRIPT)], check=True, env={**os.environ, "PATH": f"{tmp_path}:{os.environ['PATH']}"})
    return [line for line in log.read_text().splitlines() if line.split()[0] in ("toolchain", "component") and line.split()[1] in ("install", "add")] if log.exists() else []


def test_an_installed_nightly_with_rustfmt_is_left_alone(tmp_path):
    """The runner image's nightly sits in a lower overlay layer: updating it failed every job's set-up with 'Invalid cross-device link' once a newer nightly was out."""
    assert run(tmp_path, "1.99.0-x86_64-unknown-linux-gnu\nnightly-x86_64-unknown-linux-gnu (default)\n", "cargo-x86_64\nrustfmt-x86_64-unknown-linux-gnu\n") == []


def test_a_missing_nightly_is_installed_with_rustfmt(tmp_path):
    assert run(tmp_path, "stable-x86_64-unknown-linux-gnu\n", "") == ["toolchain install nightly --profile minimal --component rustfmt"]


def test_a_nightly_without_rustfmt_gets_the_component_not_a_reinstall(tmp_path):
    assert run(tmp_path, "nightly-x86_64-unknown-linux-gnu\n", "cargo-x86_64\n") == ["component add rustfmt --toolchain nightly"]
