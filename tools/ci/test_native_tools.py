"""setvars.sh names GCC for every host, and Open Watcom's GCC configuration has no macOS: on a Mac its bootstrap built nothing (`skip_all`) and stopped at
a missing `cpyright.exe`, so no cargo build with the toolchain feature could finish. `native_tools` (toolchain/cache.sh) names clang there."""

import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def owtools(system: str) -> str:
    script = f'uname() {{ echo {system}; }}; . {ROOT}/toolchain/cache.sh; export OWTOOLS=GCC; native_tools; echo "$OWTOOLS"'
    return subprocess.run(["sh", "-c", script], capture_output=True, text=True, check=True).stdout.strip()


def test_a_mac_builds_open_watcom_with_clang():
    assert owtools("Darwin") == "CLANG"


def test_other_hosts_keep_what_setvars_names():
    assert owtools("Linux") == "GCC"


def test_every_bootstrap_script_applies_it_after_setvars():
    for name in ("toolchain/wdump.sh", "toolchain/owshim/build.sh"):
        for line in (ROOT / name).read_text().splitlines():
            if "./setvars.sh" in line and not line.lstrip().startswith("#"):
                assert "native_tools" in line, f"{name}: {line.strip()}"
