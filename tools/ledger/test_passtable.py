import os
import subprocess
import sys
from pathlib import Path

TOOL = Path(__file__).with_name("passtable.py")


def test_a_pass_row_has_its_costs_and_the_totals_add_up(tmp_path):
    ours = tmp_path / "ours.incl"
    ours.write_text(
        "     6,000,000 (10.0%)  ???:<llrm_codegen::backend::peephole::Peephole as llrm_codegen::model::passes::LIRTransform>::transform_raising [x]\n"
        "     1,000,000 (1.0%)  ???:<llrm_codegen::backend::latecleanup::LateCleanup as llrm_codegen::model::passes::LIRTransform>::transform_raising [x]\n"
    )
    clang = tmp_path / "clang.incl"
    clang.write_text(
        "     2,000,000 (2.0%)  /llvm/P.cpp:(anonymous namespace)::PeepholeOptimizerLegacy::runOnMachineFunction(llvm::MachineFunction&) [x]\n"
        "     500,000 (0.5%)  /llvm/L.cpp:(anonymous namespace)::MachineLateInstrsCleanupLegacy::runOnMachineFunction(llvm::MachineFunction&) [x]\n"
    )
    done = subprocess.run(
        [sys.executable, str(TOOL)],
        env={"PATH": os.environ["PATH"], "LEDGER_OURS": str(ours), "LEDGER_CLANG": str(clang)},
        capture_output=True,
        text=True,
    )
    assert done.returncode == 0, done.stderr
    assert "| peephole::Peephole | 6.0 | 2.0 | 3.0 | w-1 |" in done.stdout
    assert "| latecleanup::LateCleanup | 1.0 | 0.5 | 2.0 | funcattrs-port |" in done.stdout
    assert "| total | 7.0 | 2.5 | 2.80 | |" in done.stdout
