"""Where the built binaries are: LLRM_BIN, else $CARGO_TARGET_DIR/release, else <repo>/target/release.

The one resolver every Python tool and test uses; a tree built elsewhere (the gate's) has no ./target.
"""
import os
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


def bin_dir(env: dict | None = None, repo: Path = REPO) -> Path:
    env = os.environ if env is None else env
    if env.get("LLRM_BIN"):
        return Path(env["LLRM_BIN"])
    if env.get("CARGO_TARGET_DIR"):
        return Path(env["CARGO_TARGET_DIR"]) / "release"
    return target_dir(env, repo) / "release"


def target_dir(env: dict | None = None, repo: Path = REPO) -> Path:
    """Cargo's target directory, which the tools' scratch work goes under: CARGO_TARGET_DIR, else <repo>/target."""
    env = os.environ if env is None else env
    return Path(env["CARGO_TARGET_DIR"]) if env.get("CARGO_TARGET_DIR") else repo / "target"
