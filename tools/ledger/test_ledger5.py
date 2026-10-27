import pathlib, subprocess, sys, tempfile

LEDGER = pathlib.Path(__file__).with_name("ledger5.py")


def buckets(lines):
    """Run ledger5 over a callgrind listing; bucket name -> printed row."""
    with tempfile.TemporaryDirectory() as dir:
        path = pathlib.Path(dir) / "x.txt"
        path.write_text("\n".join(lines) + "\n")
        out = subprocess.run([sys.executable, str(LEDGER), str(path)], capture_output=True, text=True, check=True).stdout
    return {row[:56].strip(): row[56:].split() for row in out.splitlines()[1:]}


def test_clangs_own_cost_is_not_the_front_end_of_llrm_c():
    """llrm-cc runs clang for parse and sema: its symbols were 'unattributed', so the board gave
    llrm-c a front end it does not have (and showed clang's cost as nothing)."""
    got = buckets(
        [
            " 600,000,000 ( 60.0%)  /opt/llvm/bin/clang-21:clang::Sema::ActOnFunctionDeclarator",
            " 300,000,000 ( 30.0%)  llvm::StringMapImpl::LookupBucketFor",
            " 100,000,000 ( 10.0%)  llrm_c::compile::function",
            " 1,000,000,000 (100.0%)  PROGRAM TOTALS",
        ]
    )
    assert float(got["clang (llrm-cc front end)"][0]) > float(got["front end"][0]), got
    assert "unattributed" not in got, got
