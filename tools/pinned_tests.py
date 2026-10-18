"""For a test that runs the pinned toolchain (clang d1106deb71cc, gcc 13.4.0): it skips where the pin is absent, and fails on a job that must have it."""
import pytest

import llvmpin

SKIPPED = "SKIPPED: pinned toolchain absent"  # the pytest summary counts the skips and prints this line with -rs


def require_pin():
    """Call first in the test. A skip says so and is counted; on a self-hosted runner an absent pin is a failure, never a skip."""
    why = llvmpin.missing()
    if why is None:
        return
    if llvmpin.pin_required():
        pytest.fail(f"the pinned toolchain must be present on this job: {why}")
    pytest.skip(f"{SKIPPED}: {why}")
