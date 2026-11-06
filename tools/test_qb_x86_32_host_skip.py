import os
import stat
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def run_host_tests(tmp_path, extra_env):
    """`cargo test --test qb_x86_32_host` with a `cc` that cannot compile -m32 (every other call goes to the real cc, which cargo links with)."""
    stub = tmp_path / "bin"
    stub.mkdir(parents=True)
    cc = stub / "cc"
    cc.write_text('#!/bin/sh\nfor a in "$@"; do [ "$a" = -m32 ] && exit 1; done\nexec /usr/bin/cc "$@"\n')
    cc.chmod(cc.stat().st_mode | stat.S_IEXEC)
    env = {**os.environ, "PATH": f"{stub}:{os.environ['PATH']}", **extra_env}
    if "LLRM_REQUIRE_LIBC32" not in extra_env:
        env.pop("LLRM_REQUIRE_LIBC32", None)
    return subprocess.run(["cargo", "test", "--release", "--test", "qb_x86_32_host", "--", "--nocapture"], cwd=ROOT, env=env, capture_output=True, text=True)


def test_a_host_without_32_bit_libc_skips_with_a_reason_and_fails_where_it_is_required(tmp_path):
    """tests/qb_x86_32_host.rs failed `link: In file included from /usr/include/bits/errno.h` on a machine with no 32-bit libc headers (2 of 14 reds of the full gate, 02:00): it skips with `SKIPPED: why`,
    and fails where LLRM_REQUIRE_LIBC32 says the host must have them."""
    skipped = run_host_tests(tmp_path / "a", {})
    assert skipped.returncode == 0 and "SKIPPED: needs 32-bit libc" in skipped.stderr, skipped.stdout[-800:] + skipped.stderr[-800:]
    required = run_host_tests(tmp_path / "b", {"LLRM_REQUIRE_LIBC32": "1"})
    assert required.returncode != 0 and "LLRM_REQUIRE_LIBC32 is set" in required.stdout + required.stderr
