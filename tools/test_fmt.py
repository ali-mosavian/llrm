"""tools/fmt.sh's exit status: 1 means files would change, anything else wrong is 2 or more."""

import os
import stat
import subprocess
from pathlib import Path

FMT = Path(__file__).resolve().parent / "fmt.sh"


def run(tmp_path, nightly=0, check=0, build=0):
    """`fmt.sh --check` over one file, with a `cargo` answering as given to `+nightly fmt --version`, `+nightly fmt --check` and
    `build`; `rustfmt` upper-cases and rfmt-post copies, so the file always differs and a healthy run exits 1."""
    source = tmp_path / "a.rs"
    source.write_text("fn a() {}\n")
    bin = tmp_path / "bin"
    bin.mkdir()
    scripts = {
        "cargo": f"""#!/bin/sh
case "$*" in
*--version*) exit {nightly};;
*--check*) echo "Formatting {source}"; exit {check};;
build*)
    [ {build} = 0 ] || exit {build}
    while [ $# -gt 0 ]; do [ "$1" = --target-dir ] && dir=$2; shift; done
    mkdir -p "$dir/release"; printf '#!/bin/sh\\ncat\\n' > "$dir/release/rfmt-post"; chmod +x "$dir/release/rfmt-post";;
esac
exit 0
""",
        "rustfmt": "#!/bin/sh\ntr a-z A-Z\n",
    }
    for name, text in scripts.items():
        (bin / name).write_text(text)
        (bin / name).chmod(0o755)
    env = {**os.environ, "PATH": f"{bin}:{os.environ['PATH']}", "CARGO_TARGET_DIR": str(tmp_path / "target")}
    return subprocess.run([str(FMT), "--check"], env=env, capture_output=True, text=True).returncode


def test_files_that_would_change_exit_1(tmp_path):
    """The harness has to get as far as a verdict, or the failure cases below pass for the wrong reason."""
    assert run(tmp_path) == 1


def test_a_missing_nightly_toolchain_is_not_exit_1(tmp_path):
    """`cargo +nightly` failing listed no files and the check passed: an unenforced gate step hid a host without the toolchain."""
    assert run(tmp_path, nightly=1) >= 2


def test_a_rustfmt_crash_is_not_exit_1(tmp_path):
    assert run(tmp_path, check=101) >= 2


def test_a_failed_formatter_build_is_not_exit_1(tmp_path):
    """cargo build exits 1 for an error as well, which is the code for 'files would change'."""
    assert run(tmp_path, build=1) >= 2
