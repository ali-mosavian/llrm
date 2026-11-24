"""The llrm-cc row a QB build uses must be a hosted one: a freestanding row (`-ffreestanding`, `__STDC_HOSTED__` 0) makes clang's
limits.h skip glibc's, and os.c answered with 20 clang errors about PATH_MAX.

  python3 tools/qbrow.py DIST TARGET
"""

import os
import subprocess
import sys


def hosted_state(cc, target, env=None):
    """`__STDC_HOSTED__` of the row `target` as `cc` compiles it: 1, 0, or None when it cannot be read."""
    run = subprocess.run(
        [cc, "-x", "c", "-E", "-dM", "-"], input="", capture_output=True, text=True,
        env={**os.environ, **(env or {}), "LLRM_CC_TARGET": target},
    )
    for line in run.stdout.splitlines():
        if line.startswith("#define __STDC_HOSTED__ "):
            return int(line.split()[2])
    return None


def require_hosted(cc, target, env=None):
    """Exit with `wrong row for QB (__STDC_HOSTED__ 0)` for a freestanding row; the row's flags are asked, not its name."""
    if hosted_state(cc, target, env) == 0:
        sys.exit(f"wrong row for QB (__STDC_HOSTED__ 0): {target} is freestanding; use its hosted row (arm64-glibc for arm64-linux)")


if __name__ == "__main__":
    require_hosted(os.path.join(sys.argv[1], "llrm-cc"), sys.argv[2], {"LLRM_LLC": os.path.join(sys.argv[1], "llrm-llc")})
