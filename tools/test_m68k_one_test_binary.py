"""llrm-m68k's integration tests link as one binary, `tests/all/main.rs`.

287 test binaries of 96 MB each took the gate past its 900 s cap and 24 GB of every target dir. A file in `tests/all/` that
`main.rs` does not declare is never compiled: the suite would pass without it."""
import re
from pathlib import Path

TESTS = Path(__file__).resolve().parents[1] / "crates/target/llrm-m68k/tests"
# One process each: a binary another harness names, or one whose assertion reads process-wide state.
OWN_PROCESS = {
    "hygiene": "checkland runs `-p llrm-m68k --test hygiene` by binary name",
    "timing_init_allocs": "its #[global_allocator] counts every allocation in the process, and the first price must be the first",
    "unroll_analysis_budget": "compares two wall-clock compiles; other tests running beside it skew the ratio",
    "startup_isa": "asserts on `isa::rows_parsed`, a process-wide counter every other test's parse also moves",
    "lowering_per_module": "asserts on `lowerings_built`, a process-wide counter",
    "timing_prices_once": "asserts on `priced_texts`, a process-wide counter",
}


def test_the_tests_dir_holds_one_binary_and_those_that_need_their_own_process():
    stray = sorted(path.name for path in TESTS.glob("*.rs") if path.stem not in OWN_PROCESS)
    assert not stray, f"{len(stray)} files (first {stray[:5]}): move into tests/all/ and add `mod NAME;` to tests/all/main.rs"


def test_every_file_in_the_one_binary_is_declared_in_its_main():
    declared = set(re.findall(r"^mod (\w+);", (TESTS / "all/main.rs").read_text(), re.M)) - {"support"}
    files = {path.stem for path in (TESTS / "all").glob("*.rs") if path.name != "main.rs"}
    assert files == declared, f"not declared: {sorted(files - declared)}; no file: {sorted(declared - files)}"
