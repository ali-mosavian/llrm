import pathlib, subprocess, sys

HERE = pathlib.Path(__file__).parent
LEDGER = HERE / "ledger5.py"
# `callgrind --trace-children=yes` of `llrm-cc -O2 -c bench/fib/fib.c` (x86_32-wat), the clang process,
# `callgrind_annotate --auto=no --threshold=100`, top of the listing.
CLANG = HERE / "fixtures" / "llrm-cc-clang.callgrind.txt"


def buckets(path):
    out = subprocess.run([sys.executable, str(LEDGER), str(path)], capture_output=True, text=True, check=True).stdout
    return {row[:56].strip(): float(row[56:].split()[0]) for row in out.splitlines()[1:]}


def test_clangs_own_cost_is_its_own_bucket():
    """llrm-cc runs clang for parse and sema: its symbols (`clang::`, `llvm::`, source paths under
    clang/ and llvm/) were 'unattributed' or the shared leaves, so the board showed clang's cost as
    nothing and the front end as llrm-c's."""
    got = buckets(CLANG)
    assert got["clang (llrm-cc front end)"] > 3.0, got
    assert "front end" not in got and "unattributed" not in got, got
    assert not [k for k in got if k not in ("clang (llrm-cc front end)", "alloc/libc/std (shared leaves)", "(below threshold)", "TOTAL")], got


def test_a_rust_module_named_llvm_is_not_clang():
    """`llrm_codegen::abi::llvm::LlvmAbi` matched `llvm::` and went to the clang bucket."""
    path = HERE / "fixtures" / "llrm-llc.callgrind.txt"
    assert "clang (llrm-cc front end)" not in buckets(path)
